#!/bin/bash

# Remove the code coverage reports
echo "Remove the code coverage reports"
rm -rf target/coverage*

# Run tests with coverage
echo "Run tests with coverage"
CARGO_INCREMENTAL=0 RUSTFLAGS="-Cinstrument-coverage" LLVM_PROFILE_FILE="target/coverage-raw/cargo-test-%p-%m.profraw" cargo test

# Generate coverage reports
echo "Generate coverage reports"
grcov target/coverage-raw \
    --binary-path ./target/debug/deps/ \
    -s . \
    -t html \
    --branch \
    --ignore-not-existing \
    --ignore "../*" \
    --ignore "/*" \
    -o target/coverage/html \
    --llvm-path /usr/local/opt/llvm/bin