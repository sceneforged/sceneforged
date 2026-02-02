#![cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Parse a release name from WebAssembly.
///
/// This function provides a WebAssembly-compatible interface to the parser.
/// It takes a release name string and returns a JavaScript value containing
/// the parsed metadata.
///
/// # Arguments
/// * `input` - The release name string to parse
///
/// # Returns
/// A JsValue containing the serialized ParsedRelease structure, or null on error.
///
/// # Examples (JavaScript)
/// ```javascript
/// import { parse } from 'sceneforged-parser';
///
/// const result = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
/// console.log(result.title);  // "The Matrix"
/// console.log(result.year);   // 1999
/// ```
#[wasm_bindgen]
pub fn parse(input: &str) -> JsValue {
    let result = crate::parse(input);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}
