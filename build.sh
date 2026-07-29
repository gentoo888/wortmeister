#!/usr/bin/env bash

# Install wasm-bindgen to make the server ready for wasm compiling
cargo install wasm-bindgen-cli

# Compile the wasm module
cd wasm
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/wortmeister_wasm.wasm \
  --out-dir ../static/pkg --target web
cd ..

# Compile the server
cd server
cargo build --release
cd ..
