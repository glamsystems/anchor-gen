#!/bin/bash

CPI_GEN="cargo run  -p glam-cpi-gen glam --config ./config.yaml"

# Define test cases
# Each test case consists of:
# test_names[i]: name of the test
# test_idls[i]: path to the IDL file
# test_expected[i]: path to the expected output file
# test_instructions[i]: space-separated list of instructions to test

test_names=(
    "drift"
)

test_idls=(
    "$(realpath ../../glam/anchor/deps/drift/drift.json)"
)

test_expected=(
    "./drift-expected.rs"
)

test_instructions=(
    "initializeUserStats initializeUser deleteUser placeOrders updateUserCustomMarginRatio updateUserDelegate updateUserMarginTradingEnabled deposit withdraw cancelOrders cancelOrdersByIds modifyOrder"
)

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Test counter
total_tests=0
passed_tests=0

# Run all test cases
for i in "${!test_names[@]}"; do
    ((total_tests++))
    
    test_name="${test_names[$i]}"
    idl_path="${test_idls[$i]}"
    expected_path="${test_expected[$i]}"
    instructions="${test_instructions[$i]}"
    output_file="/tmp/${test_name}.rs"
    
    echo "🔄 Testing ${test_name}..."
    
    # Build instruction arguments
    ix_args=""
    for ix in $instructions; do
        ix_args="$ix_args --ixs $ix"
    done
    
    # Run CPI generator
    $CPI_GEN "$idl_path" $ix_args -o "$output_file"
    
    # Compare output with expected
    diff "$output_file" "$expected_path" > /dev/null
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Test failed: ${test_name}${NC}"
        echo "📊 Diff between generated and expected:"
        diff "$output_file" "$expected_path"
    else
        echo -e "${GREEN}✅ Test passed: ${test_name}${NC}"
        ((passed_tests++))
    fi
    echo "-----------------------------------"
done

# Print summary
echo "📋 Test Summary:"
echo "Total tests: $total_tests"
echo "Passed tests: $passed_tests"

# Exit with failure if any test failed
if [ $passed_tests -ne $total_tests ]; then
    echo -e "${RED}❌ Some tests failed${NC}"
    exit 1
else
    echo -e "${GREEN}✅ All tests passed${NC}"
fi
