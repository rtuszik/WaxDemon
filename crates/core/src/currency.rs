/// Parse a Discogs-style money string into a float.
///
/// Strips currency symbols (`$`, `,`, `€`, `£`, `¥`), then keeps only the last `.`
/// so European-formatted thousands separators collapse correctly.
///
/// Returns `None` for null/empty input or unparseable values.
pub fn parse_currency(input: Option<&str>) -> Option<f64> {
    let raw = input?;
    if raw.is_empty() {
        return None;
    }

    // Step 1: drop currency symbols and commas.
    let mut cleaned: String = raw
        .chars()
        .filter(|c| !matches!(c, '$' | ',' | '€' | '£' | '¥'))
        .collect();

    // Step 2: if multiple `.`, keep only the last one.
    if let Some(last_dot) = cleaned.rfind('.') {
        let mut result = String::with_capacity(cleaned.len());
        for (i, ch) in cleaned.char_indices() {
            if ch == '.' && i != last_dot {
                continue;
            }
            result.push(ch);
        }
        cleaned = result;
    }

    cleaned.parse::<f64>().ok()
}
