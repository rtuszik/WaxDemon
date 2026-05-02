use waxdemon_core::{classify_format, FormatBucket};

#[test]
fn none_is_unknown() {
    assert_eq!(classify_format(None), FormatBucket::Unknown);
}

#[test]
fn empty_is_unknown() {
    assert_eq!(classify_format(Some("")), FormatBucket::Unknown);
}

#[test]
fn vinyl_matches() {
    for s in [
        "1 x Vinyl",
        "2 x Vinyl (LP, Album)",
        "Some LP here",
        "EP release",
        "1 x 7\"",
        "10\" single",
        "12\" maxi",
    ] {
        // All of the above strings, once lowercased, must hit the "vinyl|lp|ep|7\"|10\"|12\"" branch.
        // EP/LP triggers require a leading space (to avoid false positives like "Helped"),
        // so the inputs are padded with a leading space before classification.
        let padded = format!(" {}", s);
        assert_eq!(
            classify_format(Some(&padded)),
            FormatBucket::Vinyl,
            "input: {}",
            s
        );
    }
}

#[test]
fn cd_matches() {
    assert_eq!(classify_format(Some("1 x CD, Album")), FormatBucket::Cd);
    assert_eq!(
        classify_format(Some("Compact Disc, Remastered")),
        FormatBucket::Cd
    );
}

#[test]
fn cassette_matches() {
    assert_eq!(
        classify_format(Some("1 x Cassette")),
        FormatBucket::Cassette
    );
    assert_eq!(
        classify_format(Some("Cass, Stereo")),
        FormatBucket::Cassette
    );
}

#[test]
fn file_matches() {
    assert_eq!(classify_format(Some("File, FLAC")), FormatBucket::File);
    assert_eq!(
        classify_format(Some("Digital download")),
        FormatBucket::File
    );
}

#[test]
fn other_fallback() {
    assert_eq!(classify_format(Some("Box Set")), FormatBucket::Other);
}
