#!/bin/bash

# Test enum-based schema variant deserialization
set -e

echo "🧪 Testing enum-based schema variant deserialization..."

# Create test project
mkdir -p test_enum_project
cd test_enum_project

# Initialize cargo project
cargo init --name test_enum > /dev/null 2>&1

# Add dependencies
cat > Cargo.toml << 'EOF'
[package]
name = "test_enum"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
EOF

# Copy test file
cp ../test_enum_deserialization.rs src/main.rs

# Run test
echo "Running enum deserialization tests..."
cargo run

echo "🎉 Enum deserialization tests completed!"

# Clean up
cd ..
rm -rf test_enum_project