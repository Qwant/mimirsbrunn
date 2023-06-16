use serde::de;
use serde::de::Deserializer;
use serde::Deserialize;
use std::time::Duration;

pub const DEFAULT_LIMIT_RESULT_ES: i64 = 10;
pub const DEFAULT_LIMIT_RESULT_REVERSE_API: i64 = 1;
pub const DEFAULT_LANG: &str = "fr";

pub fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let ms: u64 = Deserialize::deserialize(deserializer)?;
    Ok(Duration::from_millis(ms))
}

pub fn deserialize_opt_duration<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let ms: u64 = Deserialize::deserialize(deserializer)?;
    Ok(Some(Duration::from_millis(ms)))
}

pub fn usize1000() -> usize {
    1000
}

pub fn default_result_limit() -> i64 {
    DEFAULT_LIMIT_RESULT_ES
}

pub fn default_lat_lon() -> Option<f32> {
    None
}

pub fn default_result_limit_reverse() -> i64 {
    DEFAULT_LIMIT_RESULT_REVERSE_API
}

pub fn default_false() -> bool {
    false
}

pub fn default_lang() -> String {
    DEFAULT_LANG.to_string()
}

pub fn deserialize_f32<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    Ok(Some(s.parse::<f32>().unwrap()))
}

pub fn deserialize_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;
    Ok(s.parse::<i64>().unwrap())
}

pub fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s: &str = de::Deserialize::deserialize(deserializer)?;

    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Ok(false),
    }
}
