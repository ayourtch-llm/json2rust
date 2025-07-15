use clap::{Arg, Command};
use json2rust_lib::*;
use std::fs;
use std::io::{self, Read};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("json2rust")
        .version("0.1.0")
        .author("JSON to Rust CLI")
        .about("Convert JSON to Rust structs with serde support")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("FILE")
                .help("Input JSON file (or stdin if not specified)")
                .required(false),
        )
        .arg(
            Arg::new("existing")
                .short('e')
                .long("existing")
                .value_name("FILE")
                .help("Existing Rust source file to extend")
                .required(false),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output file (stdout if not specified)")
                .required(false),
        )
        .arg(
            Arg::new("struct-name")
                .short('n')
                .long("name")
                .value_name("NAME")
                .help("Name for the root struct")
                .default_value("RootStruct"),
        )
        .get_matches();

    let input_json = if let Some(input_file) = matches.get_one::<String>("input") {
        fs::read_to_string(input_file)?
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    };

    let (existing_structs, existing_code) = if let Some(existing_file) = matches.get_one::<String>("existing") {
        let existing_code = fs::read_to_string(existing_file)?;
        let existing_structs = parse_existing_structs(&existing_code)?;
        (existing_structs, Some(existing_code))
    } else {
        (Vec::new(), None)
    };

    let struct_name = matches.get_one::<String>("struct-name").unwrap();
    
    let json_schema = analyze_json(&input_json, struct_name)?;
    let rust_structs = generate_rust_structs(&json_schema, &existing_structs)?;
    let generated_code = generate_code_with_preservation(&rust_structs, existing_code.as_deref())?;

    if let Some(output_file) = matches.get_one::<String>("output") {
        fs::write(output_file, generated_code)?;
    } else {
        println!("{}", generated_code);
    }

    Ok(())
}