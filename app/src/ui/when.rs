//! Friendly local dates for grown-ups: "Today, 14:05", "Yesterday, 09:12",
//! "26 September, 14:05" (with the year only when it isn't this year). The
//! word order and month names come from the translations (`when.*`), so each
//! language reads naturally ("26. September", "26 de septiembre").

use chrono::{DateTime, Datelike, Local, NaiveDateTime, Timelike};

use crate::i18n::I18n;

const MONTHS: [&str; 12] = [
    "when.month.1",
    "when.month.2",
    "when.month.3",
    "when.month.4",
    "when.month.5",
    "when.month.6",
    "when.month.7",
    "when.month.8",
    "when.month.9",
    "when.month.10",
    "when.month.11",
    "when.month.12",
];

/// A Unix timestamp as a friendly date in the computer's local time zone.
pub fn friendly(i18n: &I18n, secs: u64) -> String {
    let Some(utc) = DateTime::from_timestamp(secs as i64, 0) else {
        return String::new();
    };
    let when = utc.with_timezone(&Local).naive_local();
    friendly_local(i18n, when, Local::now().naive_local())
}

/// [`friendly`] for a local `when`, as seen at local time `now`.
pub fn friendly_local(i18n: &I18n, when: NaiveDateTime, now: NaiveDateTime) -> String {
    let template = match (now.date() - when.date()).num_days() {
        0 => i18n.t("when.today"),
        1 => i18n.t("when.yesterday"),
        _ if when.year() == now.year() => i18n.t("when.date"),
        _ => i18n.t("when.date_year"),
    };
    template
        .replace("{day}", &when.day().to_string())
        .replace("{month}", i18n.t(MONTHS[when.month0() as usize]))
        .replace("{year}", &when.year().to_string())
        .replace(
            "{time}",
            &format!("{:02}:{:02}", when.hour(), when.minute()),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(y: i32, m: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, mi, 0)
            .unwrap()
    }

    #[test]
    fn today_yesterday_and_older_dates() {
        let en = I18n::new("en");
        let now = at(2026, 9, 28, 13, 40);
        assert_eq!(
            friendly_local(&en, at(2026, 9, 28, 9, 5), now),
            "Today, 09:05"
        );
        // Just before midnight yesterday is still "yesterday".
        assert_eq!(
            friendly_local(&en, at(2026, 9, 27, 23, 59), now),
            "Yesterday, 23:59"
        );
        assert_eq!(
            friendly_local(&en, at(2026, 9, 26, 14, 5), now),
            "26 September, 14:05"
        );
        assert_eq!(
            friendly_local(&en, at(2025, 12, 1, 8, 0), now),
            "1 December 2025, 08:00"
        );
    }

    #[test]
    fn languages_use_their_own_order_and_months() {
        let now = at(2026, 9, 28, 13, 40);
        let when = at(2026, 9, 26, 14, 5);
        assert_eq!(
            friendly_local(&I18n::new("pl"), when, now),
            "26 września, 14:05"
        );
        assert_eq!(
            friendly_local(&I18n::new("de"), when, now),
            "26. September, 14:05"
        );
        assert_eq!(
            friendly_local(&I18n::new("es"), at(2025, 3, 2, 10, 0), now),
            "2 de marzo de 2025, 10:00"
        );
        assert_eq!(
            friendly_local(&I18n::new("uk"), at(2026, 9, 28, 7, 30), now),
            "Сьогодні, 07:30"
        );
    }

    #[test]
    fn every_language_has_all_month_names() {
        for lang in ["en", "pl", "de", "fr", "es", "it", "uk"] {
            let i18n = I18n::new(lang);
            for key in MONTHS {
                assert_ne!(i18n.t(key), key, "{lang} is missing {key}");
            }
        }
    }

    #[test]
    fn a_timestamp_formats_in_local_time() {
        // Whatever the zone, a real timestamp gives a non-empty date.
        assert!(!friendly(&I18n::new("en"), 1_790_000_000).is_empty());
    }
}
