#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatBucket {
    Vinyl,
    Cd,
    Cassette,
    File,
    Other,
    Unknown,
}

impl FormatBucket {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vinyl => "Vinyl",
            Self::Cd => "CD",
            Self::Cassette => "Cassette",
            Self::File => "File",
            Self::Other => "Other",
            Self::Unknown => "Unknown",
        }
    }
}

pub fn classify_format(format: Option<&str>) -> FormatBucket {
    let Some(raw) = format.filter(|s| !s.is_empty()) else {
        return FormatBucket::Unknown;
    };
    let lower = raw.to_lowercase();

    if lower.contains("vinyl")
        || lower.contains(" lp")
        || lower.contains(" ep")
        || lower.contains(" 7\"")
        || lower.contains(" 10\"")
        || lower.contains(" 12\"")
    {
        FormatBucket::Vinyl
    } else if lower.contains("cd") || lower.contains("compact disc") {
        FormatBucket::Cd
    } else if lower.contains("cass") || lower.contains("cassette") {
        FormatBucket::Cassette
    } else if lower.contains("file") || lower.contains("digital") {
        FormatBucket::File
    } else {
        FormatBucket::Other
    }
}
