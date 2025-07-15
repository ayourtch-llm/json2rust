#!/bin/bash

# Main test runner script
set -e

echo "🚀 Running json2rust test suite..."
echo "=================================="

# Run regular cargo tests
echo "1. Running unit and integration tests..."
cargo test

echo ""
echo "2. Running struct preservation tests..."
./test_preservation.sh

echo ""
echo "3. Running WASM build test..."
cd json2rust-web
./build.sh > /dev/null 2>&1
cd ..

echo ""
echo "🎉 All tests completed successfully!"
echo "✅ Unit tests: PASSED"
echo "✅ Integration tests: PASSED" 
echo "✅ Struct preservation: PASSED"
echo "✅ WASM build: PASSED"