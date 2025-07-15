# JSON2Rust Web Interface

A WebAssembly-powered web interface for the json2rust library that allows converting JSON to Rust structs directly in the browser.

## Features

- **Three-panel interface**: Existing Rust code, JSON input, and generated Rust output
- **Real-time conversion**: Auto-converts as you type (with debouncing)
- **Example data**: Quick-load buttons for common JSON patterns
- **Validation**: Client-side JSON validation with error reporting
- **Self-contained**: Single HTML file with embedded JavaScript
- **Direct library testing**: Serves as a comprehensive test for json2rust-lib in WASM environment

## Building

### Prerequisites

- Rust toolchain
- `wasm-pack` (will be installed automatically by build script if missing)

### Build Steps

1. Run the build script:
   ```bash
   ./build.sh
   ```

2. Serve the files over HTTP (required for WASM loading):
   ```bash
   python3 -m http.server 8000
   ```

3. Open `http://localhost:8000` in your browser

## Usage

1. **JSON Input**: Paste your JSON data in the middle textarea
2. **Existing Rust Code** (optional): Add existing struct definitions to extend them
3. **Struct Name**: Set the name for your root struct
4. **Convert**: Click convert or wait for auto-conversion
5. **Generated Code**: Copy the generated Rust structs from the output panel

## Example Workflows

### Simple Object
```json
{"name": "John", "age": 30, "active": true}
```
→ Generates a `User` struct with appropriate fields

### JSON Array
```json
[{"id": 1, "title": "Hello"}, {"id": 2, "title": "World"}]
```
→ Generates a root struct with `Vec<ItemType>` field

### Extending Existing Structs
Add existing struct definitions in the left panel, then provide JSON with additional fields. The tool will intelligently extend compatible structs or create new ones based on similarity.

## Architecture

- **Frontend**: Vanilla HTML/CSS/JavaScript for maximum compatibility
- **WASM Module**: Direct bindings to json2rust-lib core functions
- **Build**: Uses wasm-pack for optimized WebAssembly generation

## Files

- `index.html`: Self-contained web interface
- `src/lib.rs`: WASM bindings and exports
- `build.sh`: Build script for WASM module
- `Cargo.toml`: Dependencies and WASM configuration

## Testing

This web interface serves as a comprehensive integration test for the json2rust-lib library, validating that:
- JSON parsing works correctly in WASM environment
- Existing struct parsing functions properly
- Code generation produces valid output
- Error handling works across the WASM boundary

The interface provides immediate feedback on library functionality across different JSON patterns and edge cases.