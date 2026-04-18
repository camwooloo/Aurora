#!/usr/bin/env bash
set -euo pipefail

# Vercel's image ships Rust preinstalled at /rust. Use it rather than reinstalling.
if [ -f /rust/env ]; then
  . /rust/env
elif [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
fi
export PATH="/rust/bin:$HOME/.cargo/bin:$PATH"

rustup target add wasm32-unknown-unknown

# Build dioxus-cli from source. Prebuilt binaries (cargo-binstall) require
# glibc 2.35+, which Vercel's build image doesn't have.
if ! command -v dx >/dev/null 2>&1; then
  cargo install dioxus-cli --locked
fi

dx bundle --platform web --release
