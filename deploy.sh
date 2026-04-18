#!/usr/bin/env bash
# Build the web bundle locally and ship it to Vercel as static files.
# Requires: dioxus-cli (`cargo install dioxus-cli`) and vercel CLI (`npm i -g vercel`).
# Also bundles the Windows desktop app as /downloads/AuroraCast-windows.zip.
set -euo pipefail

ROOT="$(pwd)"

# 1. Desktop build → zip (Windows binary + assets)
dx bundle --platform desktop --release --target x86_64-pc-windows-msvc
DESKTOP_APP="target/dx/aurora-cast/release/windows/app"
DESKTOP_ZIP="target/dx/aurora-cast/release/windows/AuroraCast-windows.zip"
# cygpath converts /c/Users/... to C:\Users\... so PowerShell understands it
APP_WIN="$(cygpath -w "$ROOT/$DESKTOP_APP")"
ZIP_WIN="$(cygpath -w "$ROOT/$DESKTOP_ZIP")"
powershell -NoProfile -Command "Compress-Archive -Path '$APP_WIN\\*' -DestinationPath '$ZIP_WIN' -Force"

# 2. Web build
dx bundle --platform web --release

# 3. Stage downloads next to the web bundle
WEB_PUBLIC="$ROOT/target/dx/aurora-cast/release/web/public"
mkdir -p "$WEB_PUBLIC/downloads"
cp "$ROOT/$DESKTOP_ZIP" "$WEB_PUBLIC/downloads/"

# 4. Deploy
cd "$WEB_PUBLIC"
vercel link --yes --project aurora
vercel deploy --prod --yes
