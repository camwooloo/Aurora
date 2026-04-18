//! Root component and UI tree. All components live here to keep the signal
//! wiring readable in a single file.

use dioxus::prelude::*;
use serde_json::{json, Value};

use crate::state::*;
use crate::weather::*;

// Bundled assets. Dioxus picks these up at build time and includes them in the bundle.
const MAIN_CSS: Asset = asset!("/assets/main.css");
const MAP_JS: Asset = asset!("/assets/map.js");

/// Run a JS snippet that must end by calling `dioxus.send(value)`. Returns
/// the sent value (or Null on error). This uniform pattern keeps the Rust ↔
/// JS bridge unambiguous across every call site.
async fn js_call(script: String) -> Value {
    let mut e = document::eval(&script);
    e.recv::<Value>().await.unwrap_or(Value::Null)
}

/// Fire-and-forget: no return value expected.
fn js_fire(script: String) {
    spawn(async move {
        let _ = document::eval(&script);
        // Drop Eval; we don't care about the result.
    });
}

// =========================================================================
// Root
// =========================================================================

#[component]
pub fn App() -> Element {
    let state = use_context_provider(|| AppState::new());

    // On mobile, the forecast panel takes up ~half the screen; start with it
    // closed so the map gets the full width. User can toggle via bottom nav.
    use_effect(move || {
        spawn(async move {
            let v = js_call(r#"dioxus.send(window.innerWidth);"#.to_string()).await;
            if let Some(w) = v.as_f64() {
                if w < 768.0 {
                    let mut d = state.detail_open; d.set(false);
                }
            }
        });
    });

    // Boot the Leaflet map + subscribe to events
    use_effect(move || {
        // Initialize the map (fire-and-forget; ends with dioxus.send(null))
        js_fire(r#"
            (async () => {
              const waitFor = (fn) => new Promise(r => {
                const iv = setInterval(() => { if (fn()) { clearInterval(iv); r(); } }, 30);
              });
              await waitFor(() => window.AuroraMap && document.getElementById('map'));
              await window.AuroraMap.init('map');
              dioxus.send(null);
            })();
        "#.to_string());

        // Persistent map-click listener.
        spawn(async move {
            let mut e = document::eval(r#"
                window.addEventListener('aurora:mapclick', (ev) => {
                    dioxus.send({lat: ev.detail.lat, lon: ev.detail.lon});
                });
            "#);
            loop {
                match e.recv::<Value>().await {
                    Ok(v) => {
                        let lat = v.get("lat").and_then(|x| x.as_f64()).unwrap_or(0.0);
                        let lon = v.get("lon").and_then(|x| x.as_f64()).unwrap_or(0.0);
                        handle_map_click(state, lat, lon).await;
                    }
                    Err(_) => break,
                }
            }
        });

        // Timeline drag stream. JS dispatches `aurora:timeline` with a
        // fraction 0..1; we subscribe ONCE (persistent listener) and receive
        // the stream of events so no frames are dropped during fast drag.
        spawn(async move {
            let mut e = document::eval(r#"
                window.addEventListener('aurora:timeline', (ev) => {
                    dioxus.send(ev.detail);
                });
            "#);
            loop {
                match e.recv::<Value>().await {
                    Ok(v) => {
                        if let Some(frac) = v.as_f64() {
                            let h = (frac * (RANGE_HOURS - 1) as f64).round() as i32;
                            let mut th = state.time_hours;
                            th.set(h.clamp(0, RANGE_HOURS - 1));
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Model run ticker (cosmetic — simulates periodic updates)
        spawn(async move {
            let mut model_run_sig = state.model_run;
            loop {
                let now = chrono::Local::now();
                model_run_sig.set(now.format("%b %e, %H:00").to_string());
                gloo_sleep(60_000).await;
            }
        });
    });

    // Push layer changes to the map
    use_effect(move || {
        let layer_key = (state.layer)().key();
        js_fire(format!(
            r#"if (window.AuroraMap) window.AuroraMap.setLayer({}); dioxus.send(null);"#,
            json!(layer_key)
        ));
    });

    // Push timeline changes to the map
    use_effect(move || {
        let h = (state.time_hours)();
        js_fire(format!(
            r#"if (window.AuroraMap) window.AuroraMap.setTime({}, {}); dioxus.send(null);"#,
            h, RANGE_HOURS
        ));
    });

    // Playback: advance timeline every 250ms while playing
    use_effect(move || {
        let playing = (state.playing)();
        if !playing { return; }
        spawn(async move {
            let mut time_hours = state.time_hours;
            while (state.playing)() {
                gloo_sleep(250).await;
                let next = (time_hours() + 1) % RANGE_HOURS;
                time_hours.set(next);
            }
        });
    });

    // Load forecast when selection or model changes
    use_effect(move || {
        let selected = (state.selected)();
        let model_key = (state.model)().key();
        if let Some(loc) = selected {
            spawn(async move {
                let v = js_call(format!(
                    r#"(async () => {{
                        try {{
                            const r = await window.AuroraMap.forecast({}, {}, {});
                            dioxus.send(r);
                        }} catch (e) {{ dioxus.send(null); }}
                    }})();"#,
                    loc.lat, loc.lon, json!(model_key)
                )).await;
                if !v.is_null() {
                    if let Ok(fc) = serde_json::from_value::<Forecast>(v) {
                        let mut forecast_sig = state.forecast;
                        forecast_sig.set(Some(fc));
                    }
                }
            });
        }
    });

    // Initial pan to default selection
    use_effect(move || {
        if let Some(loc) = (state.selected)() {
            js_fire(format!(
                r#"(async () => {{
                    for (let i=0; i<60 && !(window.AuroraMap && window.AuroraMap.panTo); i++)
                        await new Promise(r=>setTimeout(r,50));
                    if (window.AuroraMap && window.AuroraMap.panTo)
                        window.AuroraMap.panTo({}, {}, 5);
                    dioxus.send(null);
                }})();"#,
                loc.lat, loc.lon
            ));
        }
    });

    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Script { src: MAP_JS, defer: true }

        div { class: "app",
            div { id: "map" }
            canvas { id: "windcanvas" }

            TopBar {}
            Rail {}

            if (state.layer_menu_open)() {
                LayerMenu {}
            }

            if (state.detail_open)() {
                DetailPanel {}
            }

            Legend {}
            Timeline {}

            if (state.premium_open)() {
                PremiumModal {}
            }
            if (state.downloads_open)() {
                DownloadsModal {}
            }

            Toast {}
            MobileNav {}
        }
    }
}

// =========================================================================
// Helpers
// =========================================================================

async fn gloo_sleep(ms: i64) {
    let _ = js_call(format!(
        "setTimeout(() => dioxus.send(null), {});", ms
    )).await;
}

async fn handle_map_click(state: AppState, lat: f64, lon: f64) {
    // Reverse geocode for a friendly name
    let v = js_call(format!(
        r#"(async () => {{
            try {{
                const r = await window.AuroraMap.reverseGeocode({}, {});
                dioxus.send(r);
            }} catch (e) {{ dioxus.send(null); }}
        }})();"#, lat, lon
    )).await;
    let name = if !v.is_null() {
        let n = v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let c = v.get("country").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if !n.is_empty() && !c.is_empty() { format!("{}, {}", n, c) }
        else if !n.is_empty() { n }
        else { format!("{:.3}, {:.3}", lat, lon) }
    } else {
        format!("{:.3}, {:.3}", lat, lon)
    };

    let mut selected = state.selected;
    selected.set(Some(GeoPlace { lat, lon, name }));
    let mut detail_open = state.detail_open;
    detail_open.set(true);

    // Quick popup while the panel loads
    let temp_unit = (state.temp_unit)();
    let wind_unit = (state.wind_unit)();
    spawn(async move {
        let v = js_call(format!(
            r#"(async () => {{
                try {{
                    const r = await window.AuroraMap.quickCurrent({}, {});
                    dioxus.send(r);
                }} catch (e) {{ dioxus.send(null); }}
            }})();"#, lat, lon
        )).await;
        if let Some(c) = v.get("current") {
            let t = c.get("temperature_2m").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let ws = c.get("wind_speed_10m").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let wd = c.get("wind_direction_10m").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let code = c.get("weather_code").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
            let html = format!(
                r#"<div class="pp-t">{icon} {temp}{tu} · {cond}</div>
                   <div class="pp-r"><span>Wind</span><b>{ws} {wu} {dir}</b></div>
                   <div class="pp-r"><span>Location</span><b>{lat:.2}, {lon:.2}</b></div>"#,
                icon = wmo_icon(code),
                temp = temp_unit.convert(t).round() as i64,
                tu = temp_unit.label(),
                cond = wmo_text(code),
                ws = wind_unit.convert_from_kmh(ws).round() as i64,
                wu = wind_unit.label(),
                dir = deg_to_cardinal(wd),
                lat = lat, lon = lon,
            );
            js_fire(format!(
                r#"if (window.AuroraMap) window.AuroraMap.showPopup({}, {}, {}); dioxus.send(null);"#,
                lat, lon, json!(html)
            ));
        }
    });
}

fn show_toast(state: AppState, msg: impl Into<String>) {
    let msg = msg.into();
    let mut toast_sig = state.toast;
    toast_sig.set(Some(msg));
    spawn(async move {
        gloo_sleep(1800).await;
        let mut t = state.toast;
        t.set(None);
    });
}

// =========================================================================
// TopBar
// =========================================================================

#[component]
fn TopBar() -> Element {
    let state = use_context::<AppState>();
    let mut query = use_signal(String::new);
    let mut results = use_signal(Vec::<Value>::new);

    let on_input = move |evt: FormEvent| {
        let q = evt.value();
        query.set(q.clone());
        if q.trim().is_empty() {
            results.set(Vec::new());
            return;
        }
        spawn(async move {
            gloo_sleep(220).await;
            if query() != q { return; } // debounced
            let v = js_call(format!(
                r#"(async () => {{
                    try {{
                        const r = await window.AuroraMap.geocode({});
                        dioxus.send(r || []);
                    }} catch (e) {{ dioxus.send([]); }}
                }})();"#,
                json!(q)
            )).await;
            let arr = v.as_array().cloned().unwrap_or_default();
            results.set(arr);
        });
    };

    rsx! {
        div { class: "topbar",
            div { class: "brand",
                div { class: "logo", "A" }
                div { class: "name", "AuroraCast ", small { "Weather" } }
                span { class: "premium-pill", title: "All premium features unlocked", "Premium" }
            }
            div { class: "searchbar",
                svg {
                    width: "16", height: "16", view_box: "0 0 24 24",
                    fill: "none", stroke: "currentColor", stroke_width: "2",
                    stroke_linecap: "round", stroke_linejoin: "round",
                    circle { cx: "11", cy: "11", r: "7" }
                    path { d: "m20 20-3.5-3.5" }
                }
                input {
                    r#type: "text",
                    placeholder: "Search city or place…",
                    autocomplete: "off",
                    value: "{query}",
                    oninput: on_input,
                }
                span { class: "kbd", "/" }

                if !results().is_empty() {
                    div { class: "search-results",
                        for it in results().into_iter() {
                            {
                                let lat = it.get("latitude").and_then(|x| x.as_f64()).unwrap_or(0.0);
                                let lon = it.get("longitude").and_then(|x| x.as_f64()).unwrap_or(0.0);
                                let name = it.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let admin = it.get("admin1").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let country = it.get("country").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let display = if !country.is_empty() { format!("{}, {}", name, country) } else { name.clone() };
                                let sub = format!("{} · {:.2}, {:.2}", admin, lat, lon);
                                let display_clone = display.clone();
                                rsx! {
                                    div { class: "item",
                                        onclick: move |_| {
                                            let mut sel = state.selected;
                                            sel.set(Some(GeoPlace{ lat, lon, name: display_clone.clone() }));
                                            let mut det = state.detail_open; det.set(true);
                                            query.set(display_clone.clone());
                                            results.set(Vec::new());
                                            js_fire(format!(
                                                "if (window.AuroraMap) window.AuroraMap.panTo({}, {}, 8); dioxus.send(null);", lat, lon
                                            ));
                                        },
                                        div {
                                            div { "{display}" }
                                            div { class: "muted", "{sub}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// =========================================================================
// Rail (desktop) — icon buttons on the left
// =========================================================================

#[component]
fn Rail() -> Element {
    let state = use_context::<AppState>();

    let icon_home = rsx! {
        svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
              stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "M3 10.5 12 3l9 7.5" }
            path { d: "M5 9v11h14V9" }
        }
    };
    let icon_layers = rsx! {
        svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
              stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "m12 3 10 6-10 6L2 9z" }
            path { d: "m2 15 10 6 10-6" }
        }
    };
    let icon_locate = rsx! {
        svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
              stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "3" }
            path { d: "M12 2v3M12 19v3M2 12h3M19 12h3" }
        }
    };
    let icon_units = rsx! {
        svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
              stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "M4 4h16v16H4z" }
            path { d: "M8 8h8M8 12h8M8 16h5" }
        }
    };
    let icon_full = rsx! {
        svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
              stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "M3 9V3h6M21 9V3h-6M3 15v6h6M21 15v6h-6" }
        }
    };
    let icon_crown = rsx! {
        svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
              stroke: "#ffb347", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "M3 8l4 9 5-6 5 6 4-9-4 3-5-5-5 5z" }
        }
    };
    let icon_download = rsx! {
        svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
              stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
            path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
            path { d: "M7 10l5 5 5-5" }
            path { d: "M12 15V3" }
        }
    };

    rsx! {
        nav { class: "rail",
            button {
                class: "active", title: "Home",
                onclick: move |_| {
                    js_fire("if (window.AuroraMap) window.AuroraMap.panTo(46.5, 8, 4); dioxus.send(null);".to_string());
                },
                {icon_home}
                span { class: "tip", "Home" }
            }
            button {
                title: "Weather layers",
                onclick: move |_| {
                    let mut o = state.layer_menu_open;
                    o.set(!o());
                },
                {icon_layers}
                span { class: "tip", "Layers & Models" }
            }
            button {
                title: "My location",
                onclick: move |_| {
                    spawn(async move {
                        let v = js_call(r#"
                            if (!navigator.geolocation) {
                                dioxus.send(null);
                            } else {
                                navigator.geolocation.getCurrentPosition(
                                    p => dioxus.send({lat:p.coords.latitude, lon:p.coords.longitude}),
                                    () => dioxus.send(null)
                                );
                            }
                        "#.to_string()).await;
                        if let (Some(lat), Some(lon)) = (
                            v.get("lat").and_then(|x| x.as_f64()),
                            v.get("lon").and_then(|x| x.as_f64())
                        ) {
                            let mut sel = state.selected;
                            sel.set(Some(GeoPlace{ lat, lon, name: "Your location".into() }));
                            let mut det = state.detail_open; det.set(true);
                            js_fire(format!(
                                "if (window.AuroraMap) window.AuroraMap.panTo({}, {}, 8); dioxus.send(null);", lat, lon
                            ));
                        } else {
                            show_toast(state, "Location unavailable");
                        }
                    });
                },
                {icon_locate}
                span { class: "tip", "My location" }
            }
            div { class: "sep" }
            button {
                title: "Units",
                onclick: move |_| {
                    let mut tu = state.temp_unit; tu.set(tu().next());
                    let mut wu = state.wind_unit; wu.set(wu().next());
                    let msg = format!("Units: {} · {}", tu().label(), wu().label());
                    show_toast(state, msg);
                },
                {icon_units}
                span { class: "tip", "Units" }
            }
            button {
                title: "Fullscreen",
                onclick: move |_| {
                    js_fire(r#"
                        try {
                            if (!document.fullscreenElement) document.documentElement.requestFullscreen();
                            else document.exitFullscreen();
                        } catch (e) {}
                        dioxus.send(null);
                    "#.to_string());
                },
                {icon_full}
                span { class: "tip", "Fullscreen" }
            }
            div { class: "sep" }
            button {
                title: "Download",
                onclick: move |_| { let mut o = state.downloads_open; o.set(true); },
                {icon_download}
                span { class: "tip", "Download apps" }
            }
            button {
                title: "Premium",
                onclick: move |_| { let mut o = state.premium_open; o.set(true); },
                {icon_crown}
                span { class: "tip", "Premium features" }
            }
        }
    }
}

// =========================================================================
// LayerMenu
// =========================================================================

#[component]
fn LayerMenu() -> Element {
    let state = use_context::<AppState>();
    let cur_layer = (state.layer)();
    let cur_model = (state.model)();
    let cur_alt = (state.altitude)();

    let layers: &[(Layer, &str, &str)] = &[
        (Layer::Wind,      "🌬️", "Wind"),
        (Layer::Temp,      "🌡️", "Temperature"),
        (Layer::Rain,      "🌧️", "Rain & Radar"),
        (Layer::Clouds,    "☁️", "Clouds"),
        (Layer::Pressure,  "📊", "Pressure"),
        (Layer::Waves,     "🌊", "Waves"),
        (Layer::Snow,      "❄️", "Snow"),
        (Layer::Satellite, "🛰️", "Satellite"),
    ];
    let models = [Model::Ecmwf, Model::Gfs, Model::Icon, Model::Ukmo];
    let altitudes = ["surface", "850", "700", "500", "300"];

    rsx! {
        div { class: "layer-menu",
            h4 { "Weather Layers" }
            div { class: "layer-grid",
                for (l, ico, name) in layers.iter().copied() {
                    button {
                        class: if cur_layer == l { "active" } else { "" },
                        onclick: move |_| {
                            let mut sig = state.layer; sig.set(l);
                            show_toast(state, format!("Layer: {}", l.label()));
                        },
                        span { class: "ico", "{ico}" }
                        "{name}"
                    }
                }
            }
            h4 { "Forecast Model" }
            div { class: "model-row",
                for m in models.iter().copied() {
                    {
                        let m_label = m.label();
                        rsx! {
                            button {
                                class: if cur_model == m { "active" } else { "" },
                                onclick: move |_| {
                                    let mut sig = state.model; sig.set(m);
                                    show_toast(state, format!("Model: {}", m.label()));
                                },
                                "{m_label}"
                            }
                        }
                    }
                }
            }
            h4 { "Altitude" }
            div { class: "model-row",
                for a in altitudes.iter().copied() {
                    {
                        let alt_label: String = if a == "surface" { "Surface".into() } else { format!("{} hPa", a) };
                        let alt_label_click = alt_label.clone();
                        rsx! {
                            button {
                                class: if cur_alt == a { "active" } else { "" },
                                onclick: move |_| {
                                    let mut sig = state.altitude; sig.set(a);
                                    show_toast(state, format!("Altitude: {}", alt_label_click));
                                },
                                "{alt_label}"
                            }
                        }
                    }
                }
            }
        }
    }
}

// =========================================================================
// DetailPanel — right-side forecast
// =========================================================================

#[component]
fn DetailPanel() -> Element {
    let state = use_context::<AppState>();
    let sel = (state.selected)();
    let fc = (state.forecast)();
    let tu = (state.temp_unit)();
    let wu = (state.wind_unit)();
    let tab = (state.tab)();

    let (place, coord) = match &sel {
        Some(l) => (l.name.clone(), format!("{:.3}°, {:.3}°", l.lat, l.lon)),
        None => ("Select a location".into(), "—".into()),
    };

    let nowcard = render_nowcard(fc.as_ref(), tu, wu);
    let scroll_body = match fc.as_ref() {
        None => rsx!{ div { class: "empty", "Fetching forecast…" } },
        Some(f) => match tab {
            Tab::Hourly => render_hourly(f, tu, wu),
            Tab::Daily  => render_daily(f, tu, state),
        }
    };

    rsx! {
        aside { class: "detail",
            div { class: "hdr",
                div {
                    div { class: "place", "{place}" }
                    div { class: "coord", "{coord}" }
                }
                button {
                    title: "Close",
                    onclick: move |_| { let mut o = state.detail_open; o.set(false); },
                    "✕"
                }
            }
            {nowcard}
            div { class: "tabs",
                button {
                    class: if tab == Tab::Hourly { "active" } else { "" },
                    onclick: move |_| { let mut s = state.tab; s.set(Tab::Hourly); },
                    "Hourly (1h)"
                }
                button {
                    class: if tab == Tab::Daily { "active" } else { "" },
                    onclick: move |_| { let mut s = state.tab; s.set(Tab::Daily); },
                    "10-day"
                }
            }
            div { class: "scroll", {scroll_body} }
        }
    }
}

fn render_nowcard(fc: Option<&Forecast>, tu: TempUnit, wu: WindUnit) -> Element {
    let c = match fc.and_then(|f| f.current.as_ref()) {
        Some(c) => c,
        None => return rsx! { div {} },
    };
    let t = c.temperature_2m.unwrap_or(0.0);
    let feels = c.apparent_temperature.unwrap_or(t);
    let code = c.weather_code.unwrap_or(0);
    let ws = c.wind_speed_10m.unwrap_or(0.0);
    let wd = c.wind_direction_10m.unwrap_or(0.0);
    let hum = c.relative_humidity_2m.unwrap_or(0.0);
    let pr = c.surface_pressure.unwrap_or(0.0);

    let temp_disp = tu.convert(t).round() as i64;
    let feels_disp = tu.convert(feels).round() as i64;
    let unit_label = tu.label();
    let cond = format!("{} · feels {}{}", wmo_text(code), feels_disp, unit_label);
    let icon = wmo_icon(code);
    let wind_disp = format!("{} {} {}",
        wu.convert_from_kmh(ws).round() as i64,
        wu.label(),
        deg_to_cardinal(wd),
    );
    let hum_disp = format!("{}%", hum.round() as i64);
    let pr_disp = format!("{} hPa", pr.round() as i64);

    rsx! {
        div { class: "nowcard",
            div {
                div { class: "temp",
                    span { "{temp_disp}" }
                    small { "{unit_label}" }
                }
                div { class: "cond", "{cond}" }
            }
            div { class: "ico", "{icon}" }
        }
        div { class: "quickstats",
            div { class: "s",
                div { class: "l", "Wind" }
                div { class: "v", "{wind_disp}" }
            }
            div { class: "s",
                div { class: "l", "Humidity" }
                div { class: "v", "{hum_disp}" }
            }
            div { class: "s",
                div { class: "l", "Pressure" }
                div { class: "v", "{pr_disp}" }
            }
        }
    }
}

fn render_hourly(f: &Forecast, tu: TempUnit, wu: WindUnit) -> Element {
    let h = match &f.hourly { Some(h) => h, None => return rsx!{ div { class: "empty", "No data." } } };
    let now_hour = chrono::Local::now().format("%Y-%m-%dT%H:00").to_string();
    let mut rows: Vec<Element> = Vec::new();
    let mut count = 0;
    for i in 0..h.time.len() {
        if count >= 48 { break; }
        if h.time[i] < now_hour { continue; }
        let time_label = format_time_short(&h.time[i]);
        let icon = wmo_icon(*h.weather_code.get(i).unwrap_or(&0));
        let temp_str = format!("{}°", tu.convert(*h.temperature_2m.get(i).unwrap_or(&0.0)).round() as i64);
        let wind_str = format!("{} {}",
            wu.convert_from_kmh(*h.wind_speed_10m.get(i).unwrap_or(&0.0)).round() as i64,
            wu.label(),
        );
        let wd = *h.wind_direction_10m.get(i).unwrap_or(&0.0);
        let pp_str = format!("{}%", h.precipitation_probability.get(i).copied().unwrap_or(0.0).round() as i64);
        let arrow_style = format!("transform: rotate({}deg); display:inline-block;", wd);
        rows.push(rsx! {
            div { class: "hour",
                div { class: "t", "{time_label}" }
                div { class: "ico", "{icon}" }
                div { class: "temp", "{temp_str}" }
                div { class: "wind",
                    span { class: "arr", style: "{arrow_style}", "↓" }
                    " {wind_str}"
                }
                div { class: "rain", "{pp_str}" }
            }
        });
        count += 1;
    }
    rsx! { {rows.into_iter()} }
}

fn render_daily(f: &Forecast, tu: TempUnit, state: AppState) -> Element {
    let d = match &f.daily { Some(d) => d, None => return rsx!{ div { class: "empty", "No data." } } };
    if d.time.is_empty() { return rsx!{ div { class: "empty", "No data." } }; }

    let min_t = d.temperature_2m_min.iter().copied().fold(f64::INFINITY, f64::min);
    let max_t = d.temperature_2m_max.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = (max_t - min_t).max(1.0);

    let mut rows: Vec<Element> = Vec::new();
    for (i, t) in d.time.iter().enumerate() {
        let code = *d.weather_code.get(i).unwrap_or(&0);
        let hi = *d.temperature_2m_max.get(i).unwrap_or(&0.0);
        let lo = *d.temperature_2m_min.get(i).unwrap_or(&0.0);
        let fill_left = ((lo - min_t) / range) * 100.0;
        let fill_width = (((hi - lo) / range) * 100.0).max(6.0);
        let dow = format_day_dow(t, i);
        let date = format_day_date(t);
        let icon = wmo_icon(code);
        let hi_str = format!("{}°", tu.convert(hi).round() as i64);
        let lo_str = format!("{}°", tu.convert(lo).round() as i64);
        let bar_style = format!("left:{}%;width:{}%", fill_left, fill_width);
        let idx = i as i32;
        rows.push(rsx! {
            div { class: "day",
                onclick: move |_| {
                    let mut th = state.time_hours;
                    th.set((idx * 24 + 12).min(RANGE_HOURS - 1));
                },
                div {
                    div { class: "dow", "{dow}" }
                    div { class: "date", "{date}" }
                }
                div { class: "ico", "{icon}" }
                div { class: "bar", div { class: "fill", style: "{bar_style}" } }
                div { class: "hi", "{hi_str}" }
                div { class: "lo", "{lo_str}" }
            }
        });
    }
    rsx! { {rows.into_iter()} }
}

// =========================================================================
// Timeline
// =========================================================================

#[component]
fn Timeline() -> Element {
    let state = use_context::<AppState>();
    let hours = (state.time_hours)();
    let playing = (state.playing)();
    let pct = (hours as f64 / (RANGE_HOURS - 1) as f64) * 100.0;
    let date_label = {
        let t = chrono::Local::now() + chrono::Duration::hours(hours as i64);
        t.format("%a, %b %e · %H:%M").to_string()
    };

    let model_label = (state.model)().label();
    let updated = (state.model_run)();

    // Tick marks for the track: one every 6h, labeled at day boundaries
    let ticks_per_range = RANGE_HOURS / 6;
    let mut ticks = Vec::new();
    for i in 0..ticks_per_range {
        let dt = chrono::Local::now() + chrono::Duration::hours((i * 6) as i64);
        let is_day = dt.format("%H").to_string() == "00";
        let class = if is_day { "tick day" } else { "tick" };
        let lbl = if is_day { Some(dt.format("%a %e").to_string()) } else { None };
        ticks.push((class, lbl));
    }

    rsx! {
        div { class: "timeline",
            div { class: "tl-top",
                button {
                    class: "play", title: "Play/Pause",
                    onclick: move |_| {
                        let mut p = state.playing; p.set(!p());
                    },
                    if playing { "❚❚" } else { "▶" }
                }
                div { class: "tl-date", "{date_label}" }
                div { class: "tl-spacer" }
                div { class: "tl-meta",
                    span { "Model: ", b { "{model_label}" } }
                    span { "Step: ", b { "1 h" } }
                    span { "Range: ", b { "10 days" } }
                    span { "Updated: ", b { "{updated}" } }
                }
            }
            div {
                class: "tl-track",
                onclick: move |evt| {
                    let client = evt.client_coordinates();
                    let cx = client.x;
                    spawn(async move {
                        let v = js_call(format!(r#"
                            const el = document.querySelector('.tl-track');
                            if (!el) {{ dioxus.send(0); }}
                            else {{
                                const r = el.getBoundingClientRect();
                                dioxus.send(Math.max(0, Math.min(1, ({} - r.left) / r.width)));
                            }}
                        "#, cx)).await;
                        if let Some(frac) = v.as_f64() {
                            let h = (frac * (RANGE_HOURS - 1) as f64).round() as i32;
                            let mut th = state.time_hours; th.set(h.clamp(0, RANGE_HOURS - 1));
                        }
                    });
                },
                div { class: "tl-ticks",
                    for (cls, lbl) in ticks {
                        div { class: "{cls}",
                            if let Some(l) = lbl { div { class: "lbl", "{l}" } }
                        }
                    }
                }
                div { class: "tl-playhead", style: "left: {pct}%" }
            }
        }
    }
}

// =========================================================================
// Legend
// =========================================================================

#[component]
fn Legend() -> Element {
    let state = use_context::<AppState>();
    let layer = (state.layer)();
    let (min, max) = match layer {
        Layer::Wind => ("0", "80+ km/h"),
        Layer::Temp => ("-40°", "+45°"),
        Layer::Rain => ("drizzle", "storm"),
        Layer::Clouds => ("clear", "overcast"),
        Layer::Pressure => ("970", "1040 hPa"),
        Layer::Waves => ("calm", "10m+"),
        Layer::Snow => ("0", "1m+"),
        Layer::Satellite => ("warm", "cold cloud tops"),
    };
    let bar_class = format!("bar {}", layer.key());
    let layer_name = layer.label();

    rsx! {
        div { class: "legend",
            span { "{layer_name}" }
            div { class: "{bar_class}" }
            span { "{min}" }
            span { "{max}" }
        }
    }
}

// =========================================================================
// Premium modal
// =========================================================================

#[component]
fn PremiumModal() -> Element {
    let state = use_context::<AppState>();
    let close = move |_| { let mut o = state.premium_open; o.set(false); };

    rsx! {
        div { class: "modal-bg", onclick: close.clone(),
            div {
                class: "modal",
                onclick: move |evt| evt.stop_propagation(),
                button { class: "close", onclick: close.clone(), "✕" }
                div { class: "hero",
                    div { class: "logo", "A" }
                    div {
                        h2 { "AuroraCast Premium" }
                        p { "The most detailed forecast available." }
                    }
                }
                div { class: "body",
                    ul {
                        li { b { "1-hour forecast step " } "— hourly detail for the next 10 days" }
                        li { b { "10-day outlook " } "— extended daily range" }
                        li { b { "Updates 4× per day " } "— fresh model runs at 00/06/12/18 UTC" }
                        li { b { "12-hour radar & satellite loop " } "— animated playback" }
                        li { b { "Archive going back 1 year " } "— replay past storms" }
                        li { b { "Detailed forecast maps " } "— ECMWF, GFS, ICON, UKMO" }
                        li { b { "Multiple altitudes " } "— surface, 850, 700, 500, 300 hPa" }
                    }
                    div { class: "tier active",
                        div {
                            div { class: "t", "Annual subscription" }
                            div { class: "sub", "Included free in this build." }
                        }
                        div { class: "price", "Unlocked" }
                    }
                    button {
                        class: "cta",
                        onclick: move |_| {
                            let mut o = state.premium_open; o.set(false);
                            show_toast(state, "All premium features are already active ♛");
                        },
                        "All features unlocked — close"
                    }
                }
                div { class: "foot",
                    "No payment required. This is a self-hosted weather app."
                }
            }
        }
    }
}

// =========================================================================

#[component]
fn DownloadsModal() -> Element {
    let state = use_context::<AppState>();
    let close = move |_| { let mut o = state.downloads_open; o.set(false); };

    rsx! {
        div { class: "modal-bg", onclick: close.clone(),
            div {
                class: "modal",
                onclick: move |evt| evt.stop_propagation(),
                button { class: "close", onclick: close.clone(), "✕" }
                div { class: "hero",
                    div { class: "logo", "A" }
                    div {
                        h2 { "Download AuroraCast" }
                        p { "Run locally, offline — same weather maps." }
                    }
                }
                div { class: "body",
                    div { class: "tier active",
                        div {
                            div { class: "t", "Windows (x64)" }
                            div { class: "sub", "~1 MB zip · extract and run aurora-cast.exe · needs WebView2 (bundled on Windows 11)" }
                        }
                        a {
                            class: "price",
                            href: "/downloads/AuroraCast-windows.zip",
                            download: "AuroraCast-windows.zip",
                            "Download"
                        }
                    }
                    div { class: "tier",
                        div {
                            div { class: "t", "Android (.apk)" }
                            div { class: "sub", "Unsigned debug build · install via adb or enable \"Unknown sources\" · rolling release from CI" }
                        }
                        a {
                            class: "price",
                            href: "https://github.com/camwooloo/Aurora/releases/download/android-latest/AuroraCast.apk",
                            target: "_blank",
                            rel: "noopener",
                            "Download"
                        }
                    }
                }
                div { class: "foot",
                    "Web version at aurora.you. Source on GitHub."
                }
            }
        }
    }
}

// =========================================================================
// Toast
// =========================================================================

#[component]
fn Toast() -> Element {
    let state = use_context::<AppState>();
    let msg = (state.toast)();
    let show_class = if msg.is_some() { "toast show" } else { "toast" };
    let text = msg.unwrap_or_default();
    rsx! {
        div { class: "{show_class}", "{text}" }
    }
}

// =========================================================================
// Mobile bottom nav
// =========================================================================

#[component]
fn MobileNav() -> Element {
    let state = use_context::<AppState>();
    let detail_open = (state.detail_open)();
    let layer_open = (state.layer_menu_open)();

    rsx! {
        nav { class: "mobnav",
            button {
                class: if !detail_open && !layer_open { "active" } else { "" },
                onclick: move |_| {
                    let mut d = state.detail_open; d.set(false);
                    let mut l = state.layer_menu_open; l.set(false);
                    js_fire("if (window.AuroraMap) window.AuroraMap.invalidate(); dioxus.send(null);".to_string());
                },
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                      stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                    path { d: "m12 3 10 6-10 6L2 9z" }
                }
                "Map"
            }
            button {
                class: if layer_open { "active" } else { "" },
                onclick: move |_| {
                    let mut l = state.layer_menu_open; l.set(!layer_open);
                    if !layer_open {
                        let mut d = state.detail_open; d.set(false);
                    }
                },
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                      stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                    rect { x: "3", y: "3", width: "7", height: "7", rx: "1" }
                    rect { x: "14", y: "3", width: "7", height: "7", rx: "1" }
                    rect { x: "3", y: "14", width: "7", height: "7", rx: "1" }
                    rect { x: "14", y: "14", width: "7", height: "7", rx: "1" }
                }
                "Layers"
            }
            button {
                class: if detail_open { "active" } else { "" },
                onclick: move |_| {
                    let mut d = state.detail_open; d.set(!detail_open);
                    if !detail_open {
                        let mut l = state.layer_menu_open; l.set(false);
                    }
                },
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                      stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                    path { d: "M4 14a8 8 0 1116 0" }
                    path { d: "M8 14a4 4 0 118 0" }
                }
                "Forecast"
            }
            button {
                onclick: move |_| { let mut d = state.downloads_open; d.set(true); },
                svg { width: "20", height: "20", view_box: "0 0 24 24", fill: "none",
                      stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                    path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
                    path { d: "M7 10l5 5 5-5" }
                    path { d: "M12 15V3" }
                }
                "Get app"
            }
        }
    }
}
