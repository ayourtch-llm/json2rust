use wasm_bindgen::prelude::*;
use json2rust_lib::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub fn convert_json_to_rust(json_input: &str, existing_rust_code: &str, struct_name: &str, merge_strategy: &str) -> String {
    match convert_json_to_rust_internal(json_input, existing_rust_code, struct_name, merge_strategy) {
        Ok(result) => result,
        Err(e) => format!("Error: {}", e),
    }
}

fn convert_json_to_rust_internal(
    json_input: &str,
    existing_rust_code: &str,
    struct_name: &str,
    strategy: &str
) -> Result<String, Box<dyn std::error::Error>> {
    let json_schema = analyze_json(json_input, struct_name)?;
    
    let existing_structs = if existing_rust_code.trim().is_empty() {
        Vec::new()
    } else {
        parse_existing_structs(existing_rust_code)?
    };
    let merge_strategy: MergeStrategy = strategy.into();
    let generated_types = generate_rust_types_with_strategy(&json_schema, &existing_structs, &merge_strategy)?;

    
    let rust_structs = generate_rust_structs(&json_schema, &existing_structs)?;
    let generated_code = generate_code_with_types_and_preservation_and_schema(&generated_types, Some(&existing_rust_code), &merge_strategy, Some(&json_schema))?;

    
    Ok(generated_code)
}

#[wasm_bindgen]
pub fn validate_json(json_input: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(json_input) {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[wasm_bindgen]
pub fn get_error_message(json_input: &str, existing_rust_code: &str, struct_name: &str, merge_strategy: &str) -> String {
    match convert_json_to_rust_internal(json_input, existing_rust_code, struct_name, merge_strategy) {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    console_log!("json2rust WASM module loaded");
}
