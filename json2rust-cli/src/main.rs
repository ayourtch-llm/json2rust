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
                .help("Input JSON file(s) - can be specified multiple times for sequential processing")
                .action(clap::ArgAction::Append)
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
        .arg(
            Arg::new("merge-strategy")
                .short('s')
                .long("merge-strategy")
                .value_name("STRATEGY")
                .help("Strategy for merging incompatible schemas")
                .value_parser(["optional", "enum", "hybrid"])
                .default_value("optional"),
        )
        .arg(
            Arg::new("show-intermediate")
                .long("show-intermediate")
                .help("Show intermediate results between multi-step processing")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let input_files: Vec<String> = if let Some(input_files) = matches.get_many::<String>("input") {
        input_files.cloned().collect()
    } else {
        // Read from stdin
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        // Write to temp file for processing
        let temp_file = "tmp/stdin_input.json";
        fs::write(temp_file, buffer)?;
        vec![temp_file.to_string()]
    };

    let (mut existing_structs, mut current_code) = if let Some(existing_file) = matches.get_one::<String>("existing") {
        let existing_code = fs::read_to_string(existing_file)?;
        let existing_structs = parse_existing_structs(&existing_code)?;
        (existing_structs, Some(existing_code))
    } else {
        (Vec::new(), None)
    };

    let struct_name = matches.get_one::<String>("struct-name").unwrap();
    let merge_strategy = matches.get_one::<String>("merge-strategy").unwrap().as_str().into();
    let show_intermediate = matches.get_flag("show-intermediate");
    
    // Process each input file sequentially
    for (step, input_file) in input_files.iter().enumerate() {
        eprintln!("📝 Step {}: Processing {}", step + 1, input_file);
        
        let input_json = fs::read_to_string(input_file)?;
        let json_schema = analyze_json(&input_json, struct_name)?;
        let generated_types = generate_rust_types_with_strategy(&json_schema, &existing_structs, &merge_strategy)?;
        let generated_code = generate_code_with_types_and_preservation_and_schema(&generated_types, current_code.as_deref(), &merge_strategy, Some(&json_schema))?;
        
        if show_intermediate {
            eprintln!("🔄 Intermediate result after step {}:", step + 1);
            eprintln!("----------------------------------------");
            eprintln!("{}", generated_code);
            eprintln!("----------------------------------------");
        }
        
        // Update for next iteration
        current_code = Some(generated_code.clone());
        existing_structs = parse_existing_structs(&generated_code)?;
        
        // Final output
        if step == input_files.len() - 1 {
            if let Some(output_file) = matches.get_one::<String>("output") {
                fs::write(output_file, generated_code)?;
            } else {
                println!("{}", generated_code);
            }
        }
    }

    Ok(())
}