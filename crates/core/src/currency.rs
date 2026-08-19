pub fn parse_currency(input: Option<&str>) -> Option<f64> {
    let raw = input?;
    if raw.is_empty() {
        return None;
    }

    let mut cleaned: String = raw
        .chars()
        .filter(|c| !matches!(c, '$' | ',' | '€' | '£' | '¥'))
        .collect();

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
