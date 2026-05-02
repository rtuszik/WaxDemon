pub mod client;
pub mod error;
pub mod types;

pub use client::{fetch_collection_page, fetch_collection_value, fetch_price_suggestions, Client};
pub use error::DiscogsError;
pub use types::{
    CollectionPage, CollectionValue, DiscogsReleaseBasic, PaginationUrls, PriceSuggestion,
    PriceSuggestionsResponse,
};
