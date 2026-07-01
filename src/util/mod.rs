use std::time::{SystemTime, UNIX_EPOCH};

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[allow(dead_code)]
pub fn map_range(x: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    let t = (x - in_min) / (in_max - in_min);
    out_min + t * (out_max - out_min)
}

/// Seconds since the Unix epoch (0 on the impossible clock-before-epoch error).
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A fresh hyphenated UUID v4, used as a stable id for profiles and sessions.
pub fn new_uid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// First 8 alphanumeric characters of a uid — for human-readable folder names.
pub fn short_uid(uid: &str) -> String {
    uid.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect()
}

/// Format a Unix timestamp (UTC) as `YYYY-MM-DD_HH-MM-SS` — sortable and
/// filename-safe. UTC keeps it dependency-free and unambiguous; the precise
/// epoch is also stored in manifests.
pub fn format_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}_{h:02}-{mi:02}-{s:02}")
}

/// Convert days-since-epoch to a (year, month, day) civil date.
/// Howard Hinnant's algorithm (public domain).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_epoch_is_unix_zero() {
        assert_eq!(format_timestamp(0), "1970-01-01_00-00-00");
    }

    #[test]
    fn timestamp_known_date() {
        // 2021-01-01 00:00:00 UTC = 1609459200
        assert_eq!(format_timestamp(1_609_459_200), "2021-01-01_00-00-00");
        // 2021-01-01 13:37:42 UTC
        assert_eq!(
            format_timestamp(1_609_459_200 + 13 * 3600 + 37 * 60 + 42),
            "2021-01-01_13-37-42"
        );
    }

    #[test]
    fn short_uid_is_eight_alnum() {
        let s = short_uid("12345678-90ab-cdef-1234-567890abcdef");
        assert_eq!(s.len(), 8);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
