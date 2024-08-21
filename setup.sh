#!/bin/bash

# Function to log messages
log() {
    echo "[INFO] $1"
}

# Function to log errors
error() {
    echo "[ERROR] $1"
    exit 1
}

# Check for the --clean option
CLEAN=false
if [[ "$1" == "--clean" ]]; then
    CLEAN=true
fi

# Get current platform
CURRENT_PLATFORM=$((rustc -vV | grep "host:" | sed 's/host: //') || error "Failed to get target triple")

if [ -z "$CURRENT_PLATFORM" ]; then
    error "Failed to determine current platform"
fi

log "Detected current platform: $CURRENT_PLATFORM"

if cargo install --list | grep -q "move-cli"; then
    log "'move-cli' is already installed."
else
    log "'move-cli' is not installed. Installing from ./move-sui..."
    cargo install --path ./move-sui/crates/move-cli || error "Failed to install 'move-cli' package"
fi

# Optionally clean the projects
if $CLEAN; then
    log "Cleaning CLI target directory..."
    cargo clean --manifest-path "./cli/Cargo.toml" --target $CURRENT_PLATFORM || error "Failed to clean shared target directory"
    log "Cleaning move-fuzzer target directory..."
    cargo +nightly clean --manifest-path "./move-fuzzer/Cargo.toml" --target $CURRENT_PLATFORM || error "Failed to clean shared target directory"
fi

# Install cli package with default config
log "Installing cli package..."
cargo install --path ./cli || error "Failed to install cli package"

# Set Rust flags
export RUSTFLAGS="
-Cpasses=sancov-module \
-Cllvm-args=-sanitizer-coverage-level=4 \
-Cllvm-args=-sanitizer-coverage-inline-8bit-counters \
-Cllvm-args=-sanitizer-coverage-pc-table \
-Cllvm-args=-sanitizer-coverage-trace-compares \
-Cdebug-assertions \
-Ccodegen-units=1 \
--cfg fuzzing \
-Zsanitizer=address"
log "RUSTFLAGS set: $RUSTFLAGS"

# Set Address Sanitizer option
export ASAN_OPTIONS="detect_odr_violation=0"
log "ASAN_OPTIONS set: $ASAN_OPTIONS"

# Install move-fuzzer with code coverage
log "Installing move-fuzzer with code coverage..."
cargo +nightly install --path ./move-fuzzer --target $CURRENT_PLATFORM --config profile.release.debug=true || error "Failed to install move-fuzzer package"
