use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn to_html(input: &str) -> String {
    adoc_html::to_html(input)
}

#[wasm_bindgen]
pub fn to_html_standalone(input: &str) -> String {
    adoc_html::to_html_with_options(input, adoc_html::HtmlOptions {
        standalone: true,
        ..Default::default()
    })
}
