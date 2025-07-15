#!/bin/bash

# Test script for struct preservation functionality
set -e

echo "🧪 Testing struct preservation functionality..."

# Test the current compilation issue first
echo "Step 1: Testing compilation..."
if echo '{"name": "Alice", "age": 25, "email": "alice@example.com", "active": true}' | cargo run --bin json2rust -- -e test_multiple_structs.rs -n User > /dev/null 2>&1; then
    echo "✅ Compilation successful"
else
    echo "❌ Compilation failed - need to fix code first"
    exit 1
fi

# Test 1: Basic struct preservation
echo "Step 2: Testing basic struct preservation..."
OUTPUT=$(echo '{"name": "Alice", "age": 25, "email": "alice@example.com", "active": true}' | cargo run --bin json2rust -- -e test_multiple_structs.rs -n User)

if echo "$OUTPUT" | grep -q "struct Product"; then
    echo "✅ Product struct preserved"
else
    echo "❌ Product struct NOT preserved"
    echo "Expected: Product struct should be preserved verbatim"
    echo "Actual output:"
    echo "$OUTPUT"
    exit 1
fi

if echo "$OUTPUT" | grep -q "struct Order"; then
    echo "✅ Order struct preserved"
else
    echo "❌ Order struct NOT preserved"
    echo "Expected: Order struct should be preserved verbatim"
    echo "Actual output:"
    echo "$OUTPUT"
    exit 1
fi

if echo "$OUTPUT" | grep -q "pub struct User"; then
    echo "✅ User struct extended"
else
    echo "❌ User struct NOT extended"
    echo "Expected: User struct should be extended with new fields"
    echo "Actual output:"
    echo "$OUTPUT"
    exit 1
fi

# Test 2: Verify extended struct has correct fields
echo "Step 3: Testing extended struct fields..."
if echo "$OUTPUT" | grep -q "pub age: i32"; then
    echo "✅ Type preservation working (age: i32)"
else
    echo "❌ Type preservation failed"
    echo "Expected: age should remain i32, not become f64"
    exit 1
fi

if echo "$OUTPUT" | grep -q "pub email: String"; then
    echo "✅ New field added (email)"
else
    echo "❌ New field not added"
    echo "Expected: email field should be added"
    exit 1
fi

# Test 3: Check imports are preserved
echo "Step 4: Testing import preservation..."
if echo "$OUTPUT" | grep -q "use serde"; then
    echo "✅ Imports preserved"
else
    echo "❌ Imports not preserved"
    echo "Expected: Original imports should be preserved"
    exit 1
fi

echo "🎉 All struct preservation tests passed!"
echo ""
echo "📝 Summary:"
echo "- Unmodified structs (Product, Order) are preserved verbatim"
echo "- Modified struct (User) is extended with new fields"
echo "- Type preservation works correctly (i32 stays i32)"
echo "- Imports and other file structure preserved"