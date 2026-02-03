//! Expression tree for composing [`Condition`] nodes.
//!
//! An [`Expr`] is a recursive enum that supports AND, OR, and NOT operations
//! over leaf conditions, enabling arbitrarily complex matching logic.
//!
//! `Serialize` and `Deserialize` are implemented manually (via
//! [`serde_json::Value`] as an intermediate) to avoid deep generic
//! monomorphization that the derive macro would produce for this recursive
//! type, which can trigger compiler recursion-limit errors or ICEs.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sf_probe::MediaInfo;

use crate::condition::Condition;

/// A boolean expression tree over media conditions.
///
/// JSON format (internally tagged with `"type"`):
///
/// ```json
/// { "type": "condition", ... }       // leaf condition
/// { "type": "and", "exprs": [...] }
/// { "type": "or",  "exprs": [...] }
/// { "type": "not", "expr": {...} }
/// ```
#[derive(Debug, Clone)]
pub enum Expr {
    /// A leaf condition.
    Condition(Condition),
    /// All sub-expressions must match.
    And(Vec<Expr>),
    /// At least one sub-expression must match.
    Or(Vec<Expr>),
    /// Negates the inner expression.
    Not(Box<Expr>),
}

// ---------------------------------------------------------------------------
// Manual Serialize / Deserialize via serde_json::Value
// ---------------------------------------------------------------------------

/// Convert an `Expr` to a `serde_json::Value` (recursive, but at runtime).
fn expr_to_value(expr: &Expr) -> serde_json::Value {
    match expr {
        Expr::Condition(cond) => {
            // Serialize the Condition (non-recursive, safe to derive).
            let mut val = serde_json::to_value(cond).unwrap_or(serde_json::Value::Null);
            // The Condition already has a `"type"` field from its own serde(tag).
            // Wrap it so we can distinguish it from And/Or/Not at deserialization.
            // We'll use a different approach: re-use the condition's own JSON but
            // ensure our expr "type" key doesn't collide. Since Condition uses
            // "type" for its own tag and the condition types are distinct from
            // "and"/"or"/"not", we can just output the condition's JSON as-is
            // and detect it by the absence of "and"/"or"/"not" type tags.
            //
            // Actually, to keep it clean: wrap in {"type": "condition", "condition": {...}}
            if let serde_json::Value::Object(ref mut map) = val {
                // The condition already has "type" = e.g. "codec", "container", etc.
                // We need to wrap it to distinguish expr-level "type" from condition "type".
                let inner = serde_json::Value::Object(map.clone());
                let mut outer = serde_json::Map::new();
                outer.insert("type".into(), serde_json::Value::String("condition".into()));
                outer.insert("condition".into(), inner);
                return serde_json::Value::Object(outer);
            }
            val
        }
        Expr::And(exprs) => {
            let children: Vec<serde_json::Value> = exprs.iter().map(expr_to_value).collect();
            serde_json::json!({
                "type": "and",
                "exprs": children
            })
        }
        Expr::Or(exprs) => {
            let children: Vec<serde_json::Value> = exprs.iter().map(expr_to_value).collect();
            serde_json::json!({
                "type": "or",
                "exprs": children
            })
        }
        Expr::Not(inner) => {
            serde_json::json!({
                "type": "not",
                "expr": expr_to_value(inner)
            })
        }
    }
}

/// Parse an `Expr` from a `serde_json::Value` (recursive, but at runtime).
fn expr_from_value(val: &serde_json::Value) -> Result<Expr, String> {
    let obj = val.as_object().ok_or("Expr must be a JSON object")?;
    let type_tag = obj
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or("Expr must have a \"type\" field")?;

    match type_tag {
        "condition" => {
            let cond_val = obj
                .get("condition")
                .ok_or("condition expr must have a \"condition\" field")?;
            let cond: Condition = serde_json::from_value(cond_val.clone())
                .map_err(|e| format!("invalid condition: {e}"))?;
            Ok(Expr::Condition(cond))
        }
        "and" => {
            let exprs_val = obj
                .get("exprs")
                .and_then(|v| v.as_array())
                .ok_or("and expr must have an \"exprs\" array")?;
            let exprs: Result<Vec<Expr>, String> = exprs_val.iter().map(expr_from_value).collect();
            Ok(Expr::And(exprs?))
        }
        "or" => {
            let exprs_val = obj
                .get("exprs")
                .and_then(|v| v.as_array())
                .ok_or("or expr must have an \"exprs\" array")?;
            let exprs: Result<Vec<Expr>, String> = exprs_val.iter().map(expr_from_value).collect();
            Ok(Expr::Or(exprs?))
        }
        "not" => {
            let inner_val = obj
                .get("expr")
                .ok_or("not expr must have an \"expr\" field")?;
            let inner = expr_from_value(inner_val)?;
            Ok(Expr::Not(Box::new(inner)))
        }
        other => Err(format!("unknown expr type: {other}")),
    }
}

impl Serialize for Expr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = expr_to_value(self);
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Expr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        expr_from_value(&value).map_err(serde::de::Error::custom)
    }
}

/// Evaluate an expression tree against the given media info.
pub fn evaluate(expr: &Expr, info: &MediaInfo) -> bool {
    match expr {
        Expr::Condition(cond) => cond.evaluate(info),
        Expr::And(exprs) => exprs.iter().all(|e| evaluate(e, info)),
        Expr::Or(exprs) => exprs.iter().any(|e| evaluate(e, info)),
        Expr::Not(inner) => !evaluate(inner, info),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::{AudioCodec, Container, HdrFormat, VideoCodec};
    use sf_probe::{AudioTrack, DvInfo, VideoTrack};
    use std::path::PathBuf;

    fn make_test_info() -> MediaInfo {
        MediaInfo {
            file_path: PathBuf::from("/test/movie.mkv"),
            file_size: 1024 * 1024 * 1024,
            container: Container::Mkv,
            duration: None,
            video_tracks: vec![VideoTrack {
                codec: VideoCodec::H265,
                width: 3840,
                height: 2160,
                frame_rate: Some(23.976),
                bit_depth: Some(10),
                hdr_format: HdrFormat::DolbyVision,
                dolby_vision: Some(DvInfo {
                    profile: 7,
                    rpu_present: true,
                    el_present: true,
                    bl_present: true,
                }),
                default: true,
                language: Some("eng".to_string()),
            }],
            audio_tracks: vec![AudioTrack {
                codec: AudioCodec::TrueHd,
                channels: 8,
                sample_rate: Some(48000),
                language: Some("eng".to_string()),
                atmos: true,
                default: true,
            }],
            subtitle_tracks: vec![],
        }
    }

    #[test]
    fn evaluate_leaf_condition() {
        let info = make_test_info();
        let expr = Expr::Condition(Condition::Codec(vec![VideoCodec::H265]));
        assert!(evaluate(&expr, &info));
    }

    #[test]
    fn evaluate_and_all_true() {
        let info = make_test_info();
        let expr = Expr::And(vec![
            Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
            Expr::Condition(Condition::Container(vec![Container::Mkv])),
        ]);
        assert!(evaluate(&expr, &info));
    }

    #[test]
    fn evaluate_and_one_false() {
        let info = make_test_info();
        let expr = Expr::And(vec![
            Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
            Expr::Condition(Condition::Container(vec![Container::Mp4])),
        ]);
        assert!(!evaluate(&expr, &info));
    }

    #[test]
    fn evaluate_or_one_true() {
        let info = make_test_info();
        let expr = Expr::Or(vec![
            Expr::Condition(Condition::Codec(vec![VideoCodec::H264])),
            Expr::Condition(Condition::Container(vec![Container::Mkv])),
        ]);
        assert!(evaluate(&expr, &info));
    }

    #[test]
    fn evaluate_or_all_false() {
        let info = make_test_info();
        let expr = Expr::Or(vec![
            Expr::Condition(Condition::Codec(vec![VideoCodec::H264])),
            Expr::Condition(Condition::Container(vec![Container::Mp4])),
        ]);
        assert!(!evaluate(&expr, &info));
    }

    #[test]
    fn evaluate_not() {
        let info = make_test_info();
        let expr = Expr::Not(Box::new(Expr::Condition(Condition::Codec(vec![
            VideoCodec::H264,
        ]))));
        assert!(evaluate(&expr, &info));

        let expr = Expr::Not(Box::new(Expr::Condition(Condition::Codec(vec![
            VideoCodec::H265,
        ]))));
        assert!(!evaluate(&expr, &info));
    }

    #[test]
    fn evaluate_nested_and_or_not() {
        let info = make_test_info();
        // (H265 AND MKV) OR (NOT H264) => true OR true => true
        let expr = Expr::Or(vec![
            Expr::And(vec![
                Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
                Expr::Condition(Condition::Container(vec![Container::Mkv])),
            ]),
            Expr::Not(Box::new(Expr::Condition(Condition::Codec(vec![
                VideoCodec::H264,
            ])))),
        ]);
        assert!(evaluate(&expr, &info));
    }

    #[test]
    fn evaluate_complex_dv_rule() {
        let info = make_test_info();
        // DV Profile 7 AND HDR DolbyVision AND MinResolution(3840, 2160)
        let expr = Expr::And(vec![
            Expr::Condition(Condition::DolbyVisionProfile(vec![7])),
            Expr::Condition(Condition::HdrFormat(vec![HdrFormat::DolbyVision])),
            Expr::Condition(Condition::MinResolution {
                width: 3840,
                height: 2160,
            }),
        ]);
        assert!(evaluate(&expr, &info));
    }

    #[test]
    fn evaluate_empty_and_is_true() {
        let info = make_test_info();
        let expr = Expr::And(vec![]);
        assert!(evaluate(&expr, &info));
    }

    #[test]
    fn evaluate_empty_or_is_false() {
        let info = make_test_info();
        let expr = Expr::Or(vec![]);
        assert!(!evaluate(&expr, &info));
    }

    #[test]
    fn serde_roundtrip_condition() {
        let expr = Expr::Condition(Condition::Codec(vec![VideoCodec::H265]));
        let json = serde_json::to_string(&expr).unwrap();
        let back: Expr = serde_json::from_str(&json).unwrap();
        let info = make_test_info();
        assert!(evaluate(&back, &info));
    }

    #[test]
    fn serde_roundtrip_nested() {
        let expr = Expr::And(vec![
            Expr::Condition(Condition::Codec(vec![VideoCodec::H265])),
            Expr::Or(vec![
                Expr::Condition(Condition::Container(vec![Container::Mkv])),
                Expr::Not(Box::new(Expr::Condition(Condition::HasAtmos(false)))),
            ]),
        ]);
        let json = serde_json::to_string_pretty(&expr).unwrap();
        let back: Expr = serde_json::from_str(&json).unwrap();
        let info = make_test_info();
        assert!(evaluate(&back, &info));
    }
}
