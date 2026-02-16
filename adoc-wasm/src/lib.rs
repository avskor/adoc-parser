use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn to_html(input: &str) -> String {
    adoc_html::to_html(input)
}
