#!/usr/bin/env bash
# Compila el puente Rust (android/ffi) para Android y genera los bindings Kotlin.
set -e
cd "$(dirname "$0")/ffi"
export PATH="$HOME/.cargo/bin:$PATH"
cargo ndk -t arm64-v8a -t x86_64 -o ../app/src/main/jniLibs build --release
cargo run --bin uniffi-bindgen -- generate \
    --library target/aarch64-linux-android/release/libfocus_ffi.so \
    --language kotlin --out-dir ../app/build/generated/uniffi
