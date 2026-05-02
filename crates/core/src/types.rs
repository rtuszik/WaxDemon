use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Ordered distribution map — preserves insertion order in the emitted JSON
/// so the sort-by-count-desc ordering produced by the aggregator survives serialization.
///
/// Backed by a `Vec<(String, i64)>` serialized as a JSON object.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OrderedDist(pub Vec<(String, i64)>);

impl OrderedDist {
    pub fn from_sorted(entries: Vec<(String, i64)>) -> Self {
        Self(entries)
    }
    pub fn iter(&self) -> std::slice::Iter<'_, (String, i64)> {
        self.0.iter()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for OrderedDist {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = ser.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for OrderedDist {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        // Deserialize as BTreeMap for stability when round-tripping tests.
        let m = BTreeMap::<String, i64>::deserialize(de)?;
        Ok(Self(m.into_iter().collect()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValuableItem {
    pub id: i64,
    pub release_id: i64,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub cover_image_url: Option<String>,
    pub condition: Option<String>,
    pub suggested_value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatestAddition {
    pub id: i64,
    pub release_id: i64,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub cover_image_url: Option<String>,
    pub condition: Option<String>,
    pub suggested_value: Option<f64>,
    pub added_date: String,
    pub format: Option<String>,
    pub year: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemCountPoint {
    pub timestamp: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValuePoint {
    pub timestamp: String,
    pub min: Option<f64>,
    pub mean: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardStats {
    #[serde(rename = "totalItems")]
    pub total_items: i64,
    #[serde(rename = "latestValueMin")]
    pub latest_value_min: Option<f64>,
    #[serde(rename = "latestValueMean")]
    pub latest_value_mean: Option<f64>,
    #[serde(rename = "latestValueMax")]
    pub latest_value_max: Option<f64>,
    #[serde(rename = "averageValuePerItem")]
    pub average_value_per_item: Option<f64>,
    #[serde(rename = "itemCountHistory")]
    pub item_count_history: Vec<ItemCountPoint>,
    #[serde(rename = "valueHistory")]
    pub value_history: Vec<ValuePoint>,
    #[serde(rename = "genreDistribution")]
    pub genre_distribution: OrderedDist,
    #[serde(rename = "yearDistribution")]
    pub year_distribution: OrderedDist,
    #[serde(rename = "formatDistribution")]
    pub format_distribution: OrderedDist,
    #[serde(rename = "topValuableItems")]
    pub top_valuable_items: Vec<ValuableItem>,
    #[serde(rename = "leastValuableItems")]
    pub least_valuable_items: Vec<ValuableItem>,
    #[serde(rename = "latestAdditions")]
    pub latest_additions: Vec<LatestAddition>,
}
