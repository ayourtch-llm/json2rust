# JSON2Rust Project Context

## Overview
JSON2Rust is a command-line utility that converts JSON data (single records or arrays) into Rust struct definitions with serde serialization/deserialization support. The project uses a workspace structure with separate CLI and library crates.

## Project Structure
```
json2rust/
├── Cargo.toml                 # Workspace configuration
├── claude-context.md          # This file
├── json2rust-cli/            # CLI binary crate
│   ├── Cargo.toml            # CLI dependencies
│   ├── src/main.rs           # CLI entry point
│   └── tests/integration_tests.rs # Integration tests
└── json2rust-lib/            # Core library crate
    ├── Cargo.toml            # Library dependencies
    └── src/
        ├── lib.rs            # Library exports
        ├── types.rs          # Core data structures
        ├── analyzer.rs       # JSON analysis logic
        ├── parser.rs         # Rust AST parsing
        └── codegen.rs        # Code generation
```

## Core Features
1. **JSON Analysis**: Parse JSON input and extract data structure patterns
2. **Existing Code Integration**: Parse existing Rust structs and extend them compatibly
3. **Smart Merging**: Use similarity heuristics to decide when to extend vs create new structs
4. **Code Generation**: Generate clean Rust structs with serde derives
5. **Backward Compatibility**: Ensure JSON from any iteration remains deserializable

## Key Components

### Types (`types.rs`)
- `JsonSchema`: Internal representation of JSON structure
- `RustStruct` & `RustField`: Generated Rust structure representation
- `ExistingStruct`: Parsed existing struct information
- `Json2RustError`: Error types for the library

### Analyzer (`analyzer.rs`)
- `analyze_json()`: Main entry point for JSON analysis
- Schema merging logic for arrays and objects
- Utility functions for case conversion (snake_case, PascalCase)

### Parser (`parser.rs`)
- `parse_existing_structs()`: Parse Rust source files using syn
- `calculate_struct_similarity()`: Heuristic for compatibility checking
- Type compatibility checking for backward compatibility

### Code Generator (`codegen.rs`)
- `generate_rust_structs()`: Main code generation orchestrator
- `generate_code()`: Output final Rust source code
- Similarity threshold management (70% for extension vs new struct)

## CLI Interface
```bash
json2rust [OPTIONS]
  -i, --input <FILE>        Input JSON file (or stdin)
  -e, --existing <FILE>     Existing Rust source to extend
  -o, --output <FILE>       Output file (or stdout)
  -n, --name <NAME>         Root struct name (default: "RootStruct")
```

## Key Design Decisions
1. **Similarity Threshold**: 70% similarity required to extend existing structs
2. **Array Handling**: Root-level arrays create wrapper structs with `items` field
3. **Type Compatibility**: String/number types are interchangeable for compatibility
4. **Optional Fields**: Fields that may be missing are wrapped in `Option<T>`
5. **Serde Integration**: All structs include Debug, Clone, Serialize, Deserialize derives

## Test Coverage
- **Unit Tests**: Core functionality in each module
- **Integration Tests**: End-to-end CLI testing with various JSON inputs
- **Compatibility Tests**: Existing struct extension scenarios

## Current State
The project is fully functional with:
- ✅ Complete CLI interface
- ✅ JSON parsing and analysis
- ✅ Existing struct parsing and extension
- ✅ Code generation with serde support
- ✅ Comprehensive test suite
- ✅ Error handling and edge cases

## Future Enhancements
- Support for more complex type inference
- Configuration file support
- Custom derive attribute selection
- Performance optimizations for large JSON files
- Additional output formats (trait implementations, etc.)

## Development Notes
- Uses `syn` for Rust AST parsing
- Uses `quote` for code generation
- Uses `clap` for CLI argument parsing
- Follows Rust best practices for error handling with `anyhow` and `thiserror`
- Integration tests use `tempfile` for safe file operations

## Usage Examples
```bash
# Simple JSON object
echo '{"name": "John", "age": 30}' | json2rust -n Person

# JSON array
json2rust -i users.json -n Users

# Extend existing struct
json2rust -i new_data.json -e existing.rs -n UpdatedStruct
```

This project provides a solid foundation for JSON-to-Rust conversion with extensibility and backward compatibility as core principles.