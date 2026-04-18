# AuroraCast

Cross-platform interactive weather visualization — written in Rust with
[Dioxus](https://dioxuslabs.com). One codebase targets **web (WASM)**,
**desktop (Wry)**, and **mobile (iOS / Android)**.

Features (all unlocked in this build):

- Animated wind particles (Leaflet + leaflet-velocity)
- 8 weather layers: Wind, Temperature, Rain/Radar, Clouds, Pressure, Waves, Snow, Satellite
- 1-hour forecast step for 10 days (hourly + daily tabs)
- 4 forecast models: ECMWF, GFS, ICON, UKMO (via Open-Meteo)
- Multiple altitudes: surface, 850, 700, 500, 300 hPa
- Timeline scrubber with playback
- RainViewer radar loop + infrared satellite
- Geocoded search + reverse geocode on map click
- Unit switching (°C/°F · km/h/mph/m/s/kt)
- Responsive: desktop rail + right panel, mobile bottom sheet + bottom nav

Data sources (all free, no API keys):

- [Open-Meteo](https://open-meteo.com) — forecast + geocoding
- [RainViewer](https://www.rainviewer.com/api.html) — radar + satellite tiles
- [CartoDB](https://carto.com/basemaps/) — dark basemap
- Global wind GRIB-JSON sample — bundled with leaflet-velocity

## Prerequisites

```bash
# Rust (stable)
rustup default stable

# Dioxus CLI
cargo install dioxus-cli        # `dx` binary

# WebAssembly target (for web builds)
rustup target add wasm32-unknown-unknown
```

## Run — web

```bash
dx serve --platform web
# opens http://localhost:8080 with hot reload
```

Production web bundle:

```bash
dx bundle --platform web --release
# output in dist/
```

## Run — desktop (macOS / Linux / Windows)

```bash
dx serve --platform desktop
```

Production desktop bundle (native installer):

```bash
dx bundle --platform desktop --release
```

## Run — mobile

You need the platform SDKs installed. Dioxus wraps the webview natively, so
the same Rust/JS code runs inside a real iOS / Android app shell.

### iOS (macOS only)

```bash
# one-time:
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
# plus Xcode + Xcode command line tools

dx serve --platform ios            # runs in simulator
dx bundle --platform ios --release # produces an .ipa
```

### Android

```bash
# one-time:
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
# plus Android SDK + NDK (ANDROID_HOME and NDK_HOME set)

dx serve --platform android            # runs in emulator or connected device
dx bundle --platform android --release # produces an .apk / .aab
```

## Project layout

```
aurora-cast/
├── Cargo.toml
├── Dioxus.toml
├── index.html            # shell loaded on web/desktop/mobile
├── assets/
│   ├── main.css          # responsive styles
│   └── map.js            # Leaflet controller, API bridge, wind particles
├── src/
│   ├── main.rs           # platform entry (feature-gated)
│   ├── app.rs            # root + all components
│   ├── state.rs          # shared Signals, types, WMO codes helpers
│   └── weather.rs        # formatting helpers
└── README.md
```

### How Rust ↔ map interop works

Dioxus (Rust) owns the full UI shell and state. A tiny `window.AuroraMap`
controller (in `assets/map.js`) owns the Leaflet map, wind particle canvas,
and HTTP fetching (browser `fetch` works natively inside every target's
webview). Rust calls JS with `dioxus::document::eval` one-shot calls; the
map emits `aurora:mapclick` events that Rust listens for with
Promise-resolving evals in a background task.

This keeps the Rust side portable (no `wasm-bindgen` boilerplate) and means
no changes are needed when building for desktop or mobile — the JS runs in
the webview on every target.

## Swapping the wind data for live GRIB

`assets/map.js` points `WIND_JSON_URL` at the bundled sample that ships with
`leaflet-velocity`. For real-time particles, swap it for a live
GRIB → JSON feed (e.g. your own scheduled conversion of NOMADS GFS surface
winds). The JSON shape is `[{header, data[]}]` as documented in the
leaflet-velocity readme.

## License

MIT. Weather icons are emoji. Map data © OpenStreetMap contributors, tiles
by CartoDB (CC-BY). Forecast data © Open-Meteo (CC-BY 4.0). Radar ©
RainViewer.
