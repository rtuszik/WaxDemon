use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct Page {
    pub pagination: crate::types::Pagination,
    pub releases: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
pub struct Entry {
    #[serde(flatten)]
    pub release: crate::types::DiscogsReleaseBasic,
    #[serde(default)]
    pub notes: Vec<Note>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Note {
    pub field_id: i64,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct Suggestion {
    pub currency: String,
    #[serde(with = "rust_decimal::serde::arbitrary_precision")]
    pub value: Decimal,
}

pub type Suggestions = BTreeMap<String, Suggestion>;
