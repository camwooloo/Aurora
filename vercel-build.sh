#!/usr/bin/env bash
set -euo pipefail

# Install Rust (stable) if not present
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
fi
. "$HOME/.cargo/env"

rustup target add wasm32-unknown-unknown

# Fast dioxus-cli install via prebuilt binary (cargo-binstall), fall back to source build
if ! command -v dx >/dev/null 2>&1; then
  curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
  cargo binstall -y dioxus-cli --locked || cargo install dioxus-cli --locked
fi

dx bundle --platform web --release
