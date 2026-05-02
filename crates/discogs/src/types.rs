use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaginationUrls {
    #[serde(default)]
    pub last: Option<String>,
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub page: i64,
    pub pages: i64,
    pub per_page: i64,
    pub items: i64,
    #[serde(default)]
    pub urls: PaginationUrls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub name: String,
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Format {
    pub name: String,
    pub qty: String,
    #[serde(default)]
    pub descriptions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub catno: String,
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicInformation {
    pub id: i64,
    pub title: String,
    pub year: i32,
    pub resource_url: String,
    pub thumb: String,
    pub cover_image: String,
    pub formats: Vec<Format>,
    pub labels: Vec<Label>,
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub genres: Option<Vec<String>>,
    #[serde(default)]
    pub styles: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscogsReleaseBasic {
    pub id: i64,
    pub instance_id: i64,
    pub folder_id: i64,
    pub rating: i64,
    pub date_added: String,
    pub basic_information: BasicInformation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionPage {
    pub pagination: Pagination,
    pub releases: Vec<DiscogsReleaseBasic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionValue {
    pub minimum: String,
    pub median: String,
    pub maximum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSuggestion {
    pub currency: String,
    pub value: f64,
}

pub type PriceSuggestionsResponse = HashMap<String, PriceSuggestion>;
