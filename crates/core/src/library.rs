use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LibraryQuery {
    #[serde(default, deserialize_with = "empty_number")]
    pub page: Option<u32>,
    #[serde(default, deserialize_with = "empty_number")]
    pub page_size: Option<u32>,
    pub q: Option<String>,
    pub sort: Option<String>,
    #[serde(default, deserialize_with = "empty_number")]
    pub year: Option<i32>,
    #[serde(default, deserialize_with = "empty_number")]
    pub folder_id: Option<i64>,
    pub genre: Option<String>,
    pub format: Option<String>,
    pub condition: Option<String>,
    pub currency: Option<String>,
}

fn empty_number<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let value = String::deserialize(deserializer)?;
    if value.is_empty() {
        Ok(None)
    } else {
        value.parse().map(Some).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct LibraryItem {
    pub instance_id: i64,
    pub release_id: i64,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub format: Option<String>,
    pub genres: Vec<String>,
    pub styles: Vec<String>,
    pub cover_image_url: Option<String>,
    pub added_date: String,
    pub folder_id: Option<i64>,
    pub rating: Option<i32>,
    pub notes: Option<String>,
    pub condition: Option<String>,
    pub suggested_value: Option<String>,
    pub estimate_condition: Option<String>,
    pub currency: Option<String>,
    pub last_value_check: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct LibraryPage {
    pub items: Vec<LibraryItem>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Preferences {
    pub sync_interval_hours: i32,
    pub price_refresh_hours: i32,
    pub display_currency: Option<String>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            sync_interval_hours: 24,
            price_refresh_hours: 24,
            display_currency: None,
        }
    }
}
