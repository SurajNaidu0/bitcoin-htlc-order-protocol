#!/bin/bash

# Build script for Bitcoin HTLC Order Protocol canister

set -e

# Build the canister in release mode
cargo build --target wasm32-unknown-unknown --release

# Optional: Install ic-wasm for optimization
if ! command -v ic-wasm &> /dev/null; then
    echo "ic-wasm not found. Installing..."
    cargo install ic-wasm
fi

# Optimize the WASM binary
ic-wasm target/wasm32-unknown-unknown/release/bitcoin_limit_order_protocol.wasm -o target/wasm32-unknown-unknown/release/bitcoin_htlc_order_protocol.wasm optimize O3 --keep-name-section
