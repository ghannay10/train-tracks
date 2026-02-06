use chrono::{DateTime, Utc};

pub fn parse_time_input(input: &str) -> String {
    // Try HHMM or HH:MM format
    let clean = input.replace(":", "");
    if clean.len() == 4 && clean.chars().all(|c| c.is_ascii_digit()) {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        return format!("{}T{}:{}:00Z", today, &clean[0..2], &clean[2..4]);
    }
    // Otherwise assume it's already ISO format
    input.to_string()
}

pub fn format_iso_time(iso: &str) -> String {
    if let Ok(dt) = DateTime::parse_from_rfc3339(iso) {
        dt.format("%H:%M").to_string()
    } else {
        iso.to_string()
    }
}

pub fn calculate_change_time(arrival: &str, departure: &str) -> Option<i32> {
    let arr = DateTime::parse_from_rfc3339(arrival).ok()?;
    let dep = DateTime::parse_from_rfc3339(departure).ok()?;
    let diff = dep.signed_duration_since(arr);
    Some(diff.num_minutes() as i32)
}

pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
