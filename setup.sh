#!/bin/bash

# Informative message about setting Rust flags
echo "Installing packages with code coverage flags..."

# Install cli package with default config
echo "Installing cli package..."
cargo install --path ./cli

#Set Rust flags
export RUSTFLAGS="-Clink-dead-code \
-Cdebug-assertions \
-Ccodegen-units=1 \
--cfg fuzzing"

# Set Address Sanitizer option
export ASAN_OPTIONS="detect_odr_violation=0"

# Install move-fuzzer with flags
echo "Installing move-fuzzer with code coverage..."
cargo +nightly install --path ./move-fuzzer
