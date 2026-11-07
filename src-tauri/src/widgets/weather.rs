//! Weather: where this PC is (Windows location, rounded to about a kilometre, kept in memory
//! only, never logged) or a town you typed, and the forecast from Open-Meteo. Neither needs an
//! account or key. Refreshed every half hour while the widget is on.

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::settings::TemperatureUnit;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WeatherStatus {
    Ready,
    /// Windows location is off or not allowed for desktop apps.
    NoAccess,
    /// The typed town wasn't found.
    PlaceNotFound,
    /// Offline, or the weather service didn't answer.
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DayForecast {
    /// "2026-11-05"
    pub date: String,
    pub high: i32,
    pub low: i32,
    pub icon: String,
}

/// One hour ahead.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HourForecast {
    /// "2026-11-05T14:00"
    pub time: String,
    pub temperature: i32,
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Weather {
    pub status: WeatherStatus,
    /// In the chosen unit.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<i32>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// sun, moon, cloud-sun, cloud-moon, cloud, fog, drizzle, rain, snow, storm.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Town or city, e.g. "Ahmedabad".
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// Today's sunrise and sunset, local time ("2026-11-05T06:51").
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sunrise: Option<String>,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sunset: Option<String>,
    /// Today first.
    pub days: Vec<DayForecast>,
    /// The next six hours, from the coming one.
    pub hours: Vec<HourForecast>,
    /// Wind now, km/h.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wind_kmh: Option<i32>,
    /// Today's highest chance of rain, percent.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rain_chance: Option<i32>,
    pub unit: TemperatureUnit,
}

impl Weather {
    pub fn with_status(status: WeatherStatus, unit: TemperatureUnit) -> Self {
        Self {
            status,
            temperature: None,
            condition: None,
            icon: None,
            place: None,
            sunrise: None,
            sunset: None,
            days: Vec::new(),
            hours: Vec::new(),
            wind_kmh: None,
            rain_chance: None,
            unit,
        }
    }
}

/// WMO weather code -> (text, icon key) as Open-Meteo reports it.
pub fn describe(code: i64, is_day: bool) -> (&'static str, &'static str) {
    match code {
        0 => ("Clear", if is_day { "sun" } else { "moon" }),
        1 => ("Mostly clear", if is_day { "sun" } else { "moon" }),
        2 => ("Partly cloudy", if is_day { "cloud-sun" } else { "cloud-moon" }),
        3 => ("Cloudy", "cloud"),
        45 | 48 => ("Fog", "fog"),
        51 | 53 | 55 | 56 | 57 => ("Drizzle", "drizzle"),
        61 | 63 | 66 => ("Rain", "rain"),
        65 | 67 => ("Heavy rain", "rain"),
        71 | 73 | 75 | 77 => ("Snow", "snow"),
        80..=82 => ("Showers", "rain"),
        85 | 86 => ("Snow showers", "snow"),
        95..=99 => ("Thunderstorm", "storm"),
        _ => ("—", "cloud"),
    }
}

/// Round a coordinate to two decimals (~1 km): enough for weather, and less precise to send.
pub fn coarse(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// The forecast in Open-Meteo's answer: current conditions, today's sun times, the days ahead.
pub fn parse_forecast(json: &Value, unit: TemperatureUnit) -> Option<Weather> {
    let current = json.get("current")?;
    let temp = current.get("temperature_2m")?.as_f64()?;
    let code = current.get("weather_code")?.as_i64()?;
    let is_day = current.get("is_day").and_then(Value::as_i64).unwrap_or(1) == 1;
    if !temp.is_finite() {
        return None;
    }
    let (condition, icon) = describe(code, is_day);
    let daily = json.get("daily");
    let column = |name: &str| -> Vec<Value> {
        daily
            .and_then(|d| d.get(name))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let (dates, highs, lows, codes) = (
        column("time"),
        column("temperature_2m_max"),
        column("temperature_2m_min"),
        column("weather_code"),
    );
    let days = (0..dates.len())
        .filter_map(|i| {
            Some(DayForecast {
                date: dates.get(i)?.as_str()?.to_string(),
                high: highs.get(i)?.as_f64()?.round() as i32,
                low: lows.get(i)?.as_f64()?.round() as i32,
                icon: describe(codes.get(i)?.as_i64()?, true).1.to_string(),
            })
        })
        .collect();
    let first = |name: &str| column(name).first().and_then(Value::as_str).map(str::to_string);
    // The next six hours from now ("now" is the current reading's hour).
    let hourly = json.get("hourly");
    let hour_column = |name: &str| -> Vec<Value> {
        hourly
            .and_then(|h| h.get(name))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let now = current.get("time").and_then(Value::as_str).unwrap_or("");
    let (times, temps, hour_codes, day_flags) = (
        hour_column("time"),
        hour_column("temperature_2m"),
        hour_column("weather_code"),
        hour_column("is_day"),
    );
    let hours = (0..times.len())
        .filter(|&i| times[i].as_str().is_some_and(|t| t > now))
        .take(6)
        .filter_map(|i| {
            let is_day = day_flags.get(i).and_then(Value::as_i64).unwrap_or(1) == 1;
            Some(HourForecast {
                time: times[i].as_str()?.to_string(),
                temperature: temps.get(i)?.as_f64()?.round() as i32,
                icon: describe(hour_codes.get(i)?.as_i64()?, is_day).1.to_string(),
            })
        })
        .collect();
    let wind_kmh = current
        .get("wind_speed_10m")
        .and_then(Value::as_f64)
        .map(|w| w.round() as i32);
    let rain_chance = column("precipitation_probability_max")
        .first()
        .and_then(Value::as_f64)
        .map(|p| p.round() as i32);
    Some(Weather {
        status: WeatherStatus::Ready,
        temperature: Some(temp.round() as i32),
        condition: Some(condition.into()),
        icon: Some(icon.into()),
        place: None,
        sunrise: first("sunrise"),
        sunset: first("sunset"),
        days,
        hours,
        wind_kmh,
        rain_chance,
        unit,
    })
}

/// A town's name and position from Open-Meteo's geocoder.
pub fn parse_geocode(json: &Value) -> Option<(String, f64, f64)> {
    let hit = json.get("results")?.as_array()?.first()?;
    let name = hit
        .get("name")?
        .as_str()?
        .trim()
        .chars()
        .take(40)
        .collect::<String>();
    let lat = hit.get("latitude")?.as_f64()?;
    let lon = hit.get("longitude")?.as_f64()?;
    (!name.is_empty()).then(|| (name, coarse(lat), coarse(lon)))
}

pub fn parse_place(json: &Value) -> Option<String> {
    ["city", "locality", "principalSubdivision", "countryName"]
        .iter()
        .filter_map(|k| json.get(*k).and_then(Value::as_str))
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(|s| s.chars().take(40).collect())
}

#[derive(Debug)]
pub enum Located {
    At(f64, f64),
    /// Location is off, or desktop apps aren't allowed to use it.
    NoAccess,
    Failed,
}

/// Ask Windows where this PC is (blocks; call off the async runtime).
#[cfg(windows)]
pub fn locate() -> Located {
    use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator, PositionAccuracy};
    let access = Geolocator::RequestAccessAsync().and_then(|op| op.get());
    match access {
        Ok(GeolocationAccessStatus::Allowed) => {}
        Ok(_) => return Located::NoAccess,
        Err(_) => return Located::Failed,
    }
    let position = Geolocator::new()
        .and_then(|g| {
            g.SetDesiredAccuracy(PositionAccuracy::Default)?;
            g.GetGeopositionAsync()?.get()
        })
        .and_then(|p| p.Coordinate()?.Point()?.Position());
    match position {
        Ok(p) => Located::At(coarse(p.Latitude), coarse(p.Longitude)),
        // Access was allowed but location services can't place us right now.
        Err(_) => Located::NoAccess,
    }
}

#[cfg(not(windows))]
pub fn locate() -> Located {
    Located::Failed
}

async fn get_json(http: &reqwest::Client, url: &str, query: &[(&str, String)]) -> Option<Value> {
    http.get(url)
        .query(query)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()
}

/// A typed town's name and position.
pub async fn geocode(http: &reqwest::Client, place: &str) -> Option<(String, f64, f64)> {
    let json = get_json(
        http,
        "https://geocoding-api.open-meteo.com/v1/search",
        &[
            ("name", place.to_string()),
            ("count", "1".into()),
            ("language", "en".into()),
        ],
    )
    .await?;
    parse_geocode(&json)
}

/// The name of the town at a position.
pub async fn place_name(http: &reqwest::Client, lat: f64, lon: f64) -> Option<String> {
    let json = get_json(
        http,
        "https://api.bigdatacloud.net/data/reverse-geocode-client",
        &[
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            ("localityLanguage", "en".into()),
        ],
    )
    .await?;
    parse_place(&json)
}

pub async fn forecast(http: &reqwest::Client, lat: f64, lon: f64, unit: TemperatureUnit) -> Option<Weather> {
    let unit_name = match unit {
        TemperatureUnit::Celsius => "celsius",
        TemperatureUnit::Fahrenheit => "fahrenheit",
    };
    let json = get_json(
        http,
        "https://api.open-meteo.com/v1/forecast",
        &[
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            ("current", "temperature_2m,weather_code,is_day,wind_speed_10m".into()),
            ("hourly", "temperature_2m,weather_code,is_day".into()),
            (
                "daily",
                "weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,precipitation_probability_max"
                    .into(),
            ),
            ("forecast_days", "6".into()),
            ("temperature_unit", unit_name.into()),
            ("timezone", "auto".into()),
        ],
    )
    .await?;
    parse_forecast(&json, unit)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn describes_weather_codes() {
        assert_eq!(describe(2, true), ("Partly cloudy", "cloud-sun"));
        assert_eq!(describe(0, false), ("Clear", "moon"));
        assert_eq!(describe(81, true).1, "rain");
        assert_eq!(describe(96, true).0, "Thunderstorm");
    }

    #[test]
    fn rounds_position_to_about_a_kilometre() {
        assert_eq!(coarse(23.022_505), 23.02);
        assert_eq!(coarse(72.571_365), 72.57);
    }

    #[test]
    fn parses_current_conditions_sun_times_and_days() {
        let f = json!({
            "current": { "temperature_2m": 18.4, "weather_code": 2, "is_day": 1 },
            "daily": {
                "time": ["2026-11-05", "2026-11-06"],
                "weather_code": [61, 0],
                "temperature_2m_max": [24.6, 26.1],
                "temperature_2m_min": [15.2, 16.0],
                "sunrise": ["2026-11-05T06:51", "2026-11-06T06:52"],
                "sunset": ["2026-11-05T17:57", "2026-11-06T17:56"]
            }
        });
        let weather = parse_forecast(&f, TemperatureUnit::Celsius).unwrap();
        assert_eq!(weather.temperature, Some(18));
        assert_eq!(weather.icon.as_deref(), Some("cloud-sun"));
        assert_eq!(weather.sunset.as_deref(), Some("2026-11-05T17:57"));
        assert_eq!(weather.days.len(), 2);
        assert_eq!(
            weather.days[0],
            DayForecast {
                date: "2026-11-05".into(),
                high: 25,
                low: 15,
                icon: "rain".into()
            }
        );
        assert_eq!(
            parse_forecast(&json!({ "error": true }), TemperatureUnit::Celsius),
            None
        );
    }

    #[test]
    fn a_forecast_without_days_still_has_current_conditions() {
        let f = json!({ "current": { "temperature_2m": -3.6, "weather_code": 71 } });
        let weather = parse_forecast(&f, TemperatureUnit::Fahrenheit).unwrap();
        assert_eq!(weather.temperature, Some(-4));
        assert!(weather.days.is_empty() && weather.sunrise.is_none());
    }

    #[test]
    fn parses_geocoder_and_place_names() {
        let g = json!({ "results": [{ "name": "Pune", "latitude": 18.51957, "longitude": 73.85535 }] });
        assert_eq!(parse_geocode(&g), Some(("Pune".into(), 18.52, 73.86)));
        assert_eq!(parse_geocode(&json!({})), None);
        assert_eq!(
            parse_place(&json!({ "city": "", "locality": "Ahmedabad" })).as_deref(),
            Some("Ahmedabad")
        );
        assert_eq!(parse_place(&json!({})), None);
    }
}
