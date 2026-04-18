#!/usr/bin/env bash
set -euo pipefail

# Rustup install is idempotent — ensures rustup + wasm target are available
# even when the image has a partial cargo install without ~/.cargo/env.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
export PATH="$HOME/.cargo/bin:$PATH"

rustup target add wasm32-unknown-unknown

# Fast dioxus-cli install via prebuilt binary, fall back to source build
if ! command -v dx >/dev/null 2>&1; then
  curl -L --proto '=https' --tlsv1.2 -sSf \
    https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
  cargo binstall -y dioxus-cli --locked || cargo install dioxus-cli --locked
fi

dx bundle --platform web --release
