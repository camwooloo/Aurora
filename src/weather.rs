//! WMO weather codes and simple formatting helpers.

pub fn wmo_icon(code: i32) -> &'static str {
    match code {
        0 => "☀️", 1 => "🌤️", 2 => "⛅", 3 => "☁️",
        45 | 48 => "🌫️",
        51 | 53 | 55 => "🌦️",
        56 | 57 => "🌧️",
        61 | 63 | 65 => "🌧️",
        66 | 67 => "🌧️",
        71 | 73 | 75 | 77 => "🌨️",
        80 | 81 => "🌦️", 82 => "⛈️",
        85 | 86 => "🌨️",
        95 | 96 | 99 => "⛈️",
        _ => "❓",
    }
}

pub fn wmo_text(code: i32) -> &'static str {
    match code {
        0 => "Clear sky", 1 => "Mostly clear", 2 => "Partly cloudy", 3 => "Overcast",
        45 => "Fog", 48 => "Rime fog",
        51 => "Light drizzle", 53 => "Drizzle", 55 => "Heavy drizzle",
        56 | 57 => "Freezing drizzle",
        61 => "Light rain", 63 => "Rain", 65 => "Heavy rain",
        66 | 67 => "Freezing rain",
        71 => "Light snow", 73 => "Snow", 75 => "Heavy snow", 77 => "Snow grains",
        80 | 81 => "Rain showers", 82 => "Violent showers",
        85 => "Snow showers", 86 => "Heavy snow showers",
        95 => "Thunderstorm", 96 => "Thunderstorm + hail", 99 => "Severe thunderstorm",
        _ => "Unknown",
    }
}

pub fn deg_to_cardinal(d: f64) -> &'static str {
    const DIRS: [&str; 16] = [
        "N","NNE","NE","ENE","E","ESE","SE","SSE",
        "S","SSW","SW","WSW","W","WNW","NW","NNW",
    ];
    let idx = (((d % 360.0) / 22.5).round() as usize) % 16;
    DIRS[idx]
}

/// "Thu, Apr 17, 14:00"
pub fn format_time_short(iso: &str) -> String {
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(iso) {
        return t.format("%H:%M").to_string();
    }
    if let Ok(t) = chrono::NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M") {
        return t.format("%H:%M").to_string();
    }
    iso.split('T').nth(1).unwrap_or(iso).to_string()
}

pub fn format_day_dow(iso: &str, i: usize) -> String {
    if i == 0 { return "Today".into(); }
    if let Ok(t) = chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d") {
        return t.format("%a").to_string();
    }
    iso.into()
}

pub fn format_day_date(iso: &str) -> String {
    if let Ok(t) = chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d") {
        return t.format("%b %e").to_string();
    }
    iso.into()
}
