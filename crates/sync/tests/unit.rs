use std::collections::HashMap;
use waxdemon_discogs::PriceSuggestion;
use waxdemon_sync::run::pick_suggested_value;

fn sugg(v: f64) -> PriceSuggestion {
    PriceSuggestion {
        currency: "USD".into(),
        value: v,
    }
}

#[test]
fn picks_mint_when_present() {
    let mut m = HashMap::new();
    m.insert("Mint (M)".to_string(), sugg(100.0));
    m.insert("Good (G)".to_string(), sugg(1.0));
    assert_eq!(pick_suggested_value(&m), Some(100.0));
}

#[test]
fn falls_back_down_the_ladder() {
    let mut m = HashMap::new();
    m.insert("Very Good (VG)".to_string(), sugg(10.0));
    m.insert("Good (G)".to_string(), sugg(1.0));
    assert_eq!(pick_suggested_value(&m), Some(10.0));
}

#[test]
fn returns_none_when_no_known_condition() {
    let mut m = HashMap::new();
    m.insert("Brand New".to_string(), sugg(999.0));
    assert_eq!(pick_suggested_value(&m), None);
}

#[test]
fn returns_none_for_empty() {
    let m = HashMap::new();
    assert_eq!(pick_suggested_value(&m), None);
}
