pub mod client;
pub mod error;
pub mod types;

pub use client::{Client, fetch_collection_page, fetch_collection_value, fetch_price_suggestions};
pub use error::DiscogsError;
pub use types::{
    CollectionPage, CollectionValue, DiscogsReleaseBasic, PaginationUrls, PriceSuggestion,
    PriceSuggestionsResponse,
};
