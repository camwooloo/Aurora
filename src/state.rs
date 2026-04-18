//! Global app state — Dioxus signals shared via context.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

pub const RANGE_HOURS: i32 = 240; // 10 days × 24h

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Wind, Temp, Rain, Clouds, Pressure, Waves, Snow, Satellite,
}
impl Layer {
    pub fn key(self) -> &'static str {
        match self {
            Layer::Wind => "wind", Layer::Temp => "temp", Layer::Rain => "rain",
            Layer::Clouds => "clouds", Layer::Pressure => "pressure",
            Layer::Waves => "waves", Layer::Snow => "snow",
            Layer::Satellite => "satellite",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Layer::Wind => "Wind speed", Layer::Temp => "Temperature",
            Layer::Rain => "Precipitation", Layer::Clouds => "Cloud cover",
            Layer::Pressure => "MSL pressure", Layer::Waves => "Wave height",
            Layer::Snow => "Snow depth", Layer::Satellite => "Infrared",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model { Ecmwf, Gfs, Icon, Ukmo }
impl Model {
    pub fn key(self) -> &'static str {
        match self { Model::Ecmwf => "ecmwf", Model::Gfs => "gfs", Model::Icon => "icon", Model::Ukmo => "ukmo" }
    }
    pub fn label(self) -> &'static str {
        match self { Model::Ecmwf => "ECMWF", Model::Gfs => "GFS", Model::Icon => "ICON", Model::Ukmo => "UKMO" }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TempUnit { C, F }
impl TempUnit {
    pub fn label(self) -> &'static str { match self { TempUnit::C => "°C", TempUnit::F => "°F" } }
    pub fn convert(self, c: f64) -> f64 { match self { TempUnit::C => c, TempUnit::F => c * 9.0/5.0 + 32.0 } }
    pub fn next(self) -> Self { match self { TempUnit::C => TempUnit::F, TempUnit::F => TempUnit::C } }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindUnit { Kmh, Mph, Ms, Kt }
impl WindUnit {
    pub fn label(self) -> &'static str {
        match self { WindUnit::Kmh => "km/h", WindUnit::Mph => "mph", WindUnit::Ms => "m/s", WindUnit::Kt => "kt" }
    }
    pub fn convert_from_kmh(self, v: f64) -> f64 {
        match self {
            WindUnit::Kmh => v,
            WindUnit::Mph => v * 0.621_371,
            WindUnit::Ms  => v / 3.6,
            WindUnit::Kt  => v * 0.539_957,
        }
    }
    pub fn next(self) -> Self {
        match self {
            WindUnit::Kmh => WindUnit::Mph,
            WindUnit::Mph => WindUnit::Ms,
            WindUnit::Ms  => WindUnit::Kt,
            WindUnit::Kt  => WindUnit::Kmh,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab { Hourly, Daily }

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct GeoPlace {
    pub lat: f64,
    pub lon: f64,
    pub name: String,
}

#[derive(Clone, PartialEq, Debug, Default, Deserialize)]
pub struct Current {
    pub temperature_2m: Option<f64>,
    pub apparent_temperature: Option<f64>,
    pub weather_code: Option<i32>,
    pub wind_speed_10m: Option<f64>,
    pub wind_direction_10m: Option<f64>,
    pub relative_humidity_2m: Option<f64>,
    pub surface_pressure: Option<f64>,
    pub cloud_cover: Option<f64>,
    pub precipitation: Option<f64>,
}

#[derive(Clone, PartialEq, Debug, Default, Deserialize)]
pub struct Hourly {
    #[serde(default)] pub time: Vec<String>,
    #[serde(default)] pub temperature_2m: Vec<f64>,
    #[serde(default)] pub weather_code: Vec<i32>,
    #[serde(default)] pub wind_speed_10m: Vec<f64>,
    #[serde(default)] pub wind_direction_10m: Vec<f64>,
    #[serde(default)] pub precipitation_probability: Vec<f64>,
    #[serde(default)] pub precipitation: Vec<f64>,
}

#[derive(Clone, PartialEq, Debug, Default, Deserialize)]
pub struct Daily {
    #[serde(default)] pub time: Vec<String>,
    #[serde(default)] pub weather_code: Vec<i32>,
    #[serde(default)] pub temperature_2m_max: Vec<f64>,
    #[serde(default)] pub temperature_2m_min: Vec<f64>,
    #[serde(default)] pub precipitation_sum: Vec<f64>,
    #[serde(default)] pub precipitation_probability_max: Vec<f64>,
    #[serde(default)] pub wind_speed_10m_max: Vec<f64>,
    #[serde(default)] pub sunrise: Vec<String>,
    #[serde(default)] pub sunset: Vec<String>,
}

#[derive(Clone, PartialEq, Debug, Default, Deserialize)]
pub struct Forecast {
    #[serde(default)] pub current: Option<Current>,
    #[serde(default)] pub hourly: Option<Hourly>,
    #[serde(default)] pub daily: Option<Daily>,
}

#[derive(Clone, Copy, Debug)]
pub struct AppState {
    pub layer: Signal<Layer>,
    pub model: Signal<Model>,
    pub altitude: Signal<&'static str>,
    pub temp_unit: Signal<TempUnit>,
    pub wind_unit: Signal<WindUnit>,
    pub time_hours: Signal<i32>,
    pub playing: Signal<bool>,
    pub layer_menu_open: Signal<bool>,
    pub premium_open: Signal<bool>,
    pub detail_open: Signal<bool>,
    pub tab: Signal<Tab>,
    pub selected: Signal<Option<GeoPlace>>,
    pub forecast: Signal<Option<Forecast>>,
    pub toast: Signal<Option<String>>,
    pub model_run: Signal<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            layer: Signal::new(Layer::Wind),
            model: Signal::new(Model::Ecmwf),
            altitude: Signal::new("surface"),
            temp_unit: Signal::new(TempUnit::C),
            wind_unit: Signal::new(WindUnit::Kmh),
            time_hours: Signal::new(0),
            playing: Signal::new(false),
            layer_menu_open: Signal::new(false),
            premium_open: Signal::new(false),
            detail_open: Signal::new(true),
            tab: Signal::new(Tab::Hourly),
            selected: Signal::new(Some(GeoPlace {
                lat: 46.948, lon: 7.447, name: "Bern, Switzerland".into(),
            })),
            forecast: Signal::new(None),
            toast: Signal::new(None),
            model_run: Signal::new(String::new()),
        }
    }
}
