#!/usr/bin/env bash
# Build the web bundle locally and ship it to Vercel as static files.
# Requires: dioxus-cli (`cargo install dioxus-cli`) and vercel CLI (`npm i -g vercel`).
set -euo pipefail

dx bundle --platform web --release
vercel deploy --prod target/dx/aurora-cast/release/web/public
