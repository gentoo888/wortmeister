#!/usr/bin/env bash
# This script is created for render services to install wasm on the server and compile wasm with rust

set -e # Stop if error occurs

rustup target add wasm32-unknown-unknown

if ! command -v wasm-bindgen &>/dev/null; then
  echo "installing wasm-bindgen-cli"
  cargo install wasm-bindgen-cli
else
  echo "wasm-bindgen is already installed"
fi

echo "compiling the WASM module"
cd wasm
cargo build --release --target wasm32-unknown-unknown

echo "computing wasm"
wasm-bindgen target/wasm32-unknown-unknown/release/wortmeister_wasm.wasm \
  --out-dir ../static/pkg \
  --target web

cd ..

echo "Compiling the server"
cd server
cargo build --release # Main.rs
cd ..

echo "Copying the static files"
mkdir -p server/static
cp -r static/* server/static/
