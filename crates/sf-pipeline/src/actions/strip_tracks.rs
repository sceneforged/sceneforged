//! Strip (remove) tracks action.

use async_trait::async_trait;

use crate::action::{Action, ActionResult};
use crate::context::ActionContext;

/// Strip specified track types (optionally filtered by language) from the file.
#[derive(Debug)]
pub struct StripTracksAction {
    track_types: Vec<sf_core::StreamType>,
    languages: Option<Vec<String>>,
}

impl StripTracksAction {
    /// Create a new strip-tracks action.
    ///
    /// If `languages` is `Some`, only tracks of the given types whose language
    /// matches one of the listed values are removed.  If `None`, all tracks of
    /// the given types are removed.
    pub fn new(
        track_types: Vec<sf_core::StreamType>,
        languages: Option<Vec<String>>,
    ) -> Self {
        Self {
            track_types,
            languages,
        }
    }
}

#[async_trait]
impl Action for StripTracksAction {
    fn name(&self) -> &'static str {
        "Strip Tracks"
    }

    async fn validate(&self, ctx: &ActionContext) -> sf_core::Result<()> {
        ctx.tools.require("mkvmerge")?;
        Ok(())
    }

    async fn execute(&self, ctx: &ActionContext) -> sf_core::Result<ActionResult> {
        if ctx.dry_run {
            tracing::info!(
                "[DRY RUN] Would strip {:?} (languages: {:?})",
                self.track_types,
                self.languages,
            );
            return Ok(ActionResult {
                output: None,
                summary: format!(
                    "Would strip {:?} (languages: {:?})",
                    self.track_types, self.languages,
                ),
            });
        }

        // Build the list of track indices to strip based on types and languages.
        let indices = self.resolve_track_indices(ctx);

        sf_av::strip_tracks(&ctx.workspace, &ctx.tools, &indices).await?;

        Ok(ActionResult {
            output: Some(ctx.workspace.output()),
            summary: format!("Stripped {} track(s)", indices.len()),
        })
    }

    fn parallelizable(&self) -> bool {
        false
    }
}

impl StripTracksAction {
    /// Resolve the concrete mkvmerge track IDs to strip.
    ///
    /// Video tracks come first (indices 0..), then audio, then subtitles.
    /// This mirrors the standard mkvmerge track ordering.
    fn resolve_track_indices(&self, ctx: &ActionContext) -> Vec<usize> {
        let mut indices = Vec::new();
        let video_count = ctx.media_info.video_tracks.len();
        let audio_count = ctx.media_info.audio_tracks.len();

        for track_type in &self.track_types {
            match track_type {
                sf_core::StreamType::Audio => {
                    for (i, track) in ctx.media_info.audio_tracks.iter().enumerate() {
                        if self.language_matches(track.language.as_deref()) {
                            indices.push(video_count + i);
                        }
                    }
                }
                sf_core::StreamType::Subtitle => {
                    for (i, track) in ctx.media_info.subtitle_tracks.iter().enumerate() {
                        if self.language_matches(track.language.as_deref()) {
                            indices.push(video_count + audio_count + i);
                        }
                    }
                }
                sf_core::StreamType::Video => {
                    for i in 0..video_count {
                        indices.push(i);
                    }
                }
            }
        }

        indices
    }

    /// Check whether a track's language matches the filter (or if there is no filter).
    fn language_matches(&self, language: Option<&str>) -> bool {
        match &self.languages {
            None => true,
            Some(langs) if langs.is_empty() => true,
            Some(langs) => match language {
                Some(lang) => langs.iter().any(|l| l.eq_ignore_ascii_case(lang)),
                None => false,
            },
        }
    }
}
