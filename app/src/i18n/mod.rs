//! Lightweight runtime internationalization.
//!
//! `assets/i18n/en.json` is the source of truth for string keys; `pl, de, fr,
//! es, it, uk` are fully translated. Every European language is offered in the
//! picker and falls back to English when not yet translated.
//!
//! AGENT RULE: when you add or change a user-facing string, update `en.json` and
//! every seeded locale file so all keys stay in sync.

use std::collections::HashMap;

/// A language offered in the picker.
pub struct Lang {
    /// ISO 639-1 code (also the locale + flag asset key).
    pub code: &'static str,
    /// Native name shown in the picker.
    pub endonym: &'static str,
    /// English name (for reference/search; not yet surfaced in the UI).
    #[allow(dead_code)]
    pub english: &'static str,
}

pub const DEFAULT_LANG: &str = "en";

/// All offered languages (European). Order roughly by speaker count then A–Z.
pub const EUROPEAN_LANGS: &[Lang] = &[
    Lang {
        code: "en",
        endonym: "English",
        english: "English",
    },
    Lang {
        code: "pl",
        endonym: "Polski",
        english: "Polish",
    },
    Lang {
        code: "de",
        endonym: "Deutsch",
        english: "German",
    },
    Lang {
        code: "fr",
        endonym: "Français",
        english: "French",
    },
    Lang {
        code: "es",
        endonym: "Español",
        english: "Spanish",
    },
    Lang {
        code: "it",
        endonym: "Italiano",
        english: "Italian",
    },
    Lang {
        code: "uk",
        endonym: "Українська",
        english: "Ukrainian",
    },
    Lang {
        code: "pt",
        endonym: "Português",
        english: "Portuguese",
    },
    Lang {
        code: "nl",
        endonym: "Nederlands",
        english: "Dutch",
    },
    Lang {
        code: "sv",
        endonym: "Svenska",
        english: "Swedish",
    },
    Lang {
        code: "no",
        endonym: "Norsk",
        english: "Norwegian",
    },
    Lang {
        code: "da",
        endonym: "Dansk",
        english: "Danish",
    },
    Lang {
        code: "fi",
        endonym: "Suomi",
        english: "Finnish",
    },
    Lang {
        code: "is",
        endonym: "Íslenska",
        english: "Icelandic",
    },
    Lang {
        code: "et",
        endonym: "Eesti",
        english: "Estonian",
    },
    Lang {
        code: "lv",
        endonym: "Latviešu",
        english: "Latvian",
    },
    Lang {
        code: "lt",
        endonym: "Lietuvių",
        english: "Lithuanian",
    },
    Lang {
        code: "cs",
        endonym: "Čeština",
        english: "Czech",
    },
    Lang {
        code: "sk",
        endonym: "Slovenčina",
        english: "Slovak",
    },
    Lang {
        code: "sl",
        endonym: "Slovenščina",
        english: "Slovenian",
    },
    Lang {
        code: "hu",
        endonym: "Magyar",
        english: "Hungarian",
    },
    Lang {
        code: "ro",
        endonym: "Română",
        english: "Romanian",
    },
    Lang {
        code: "bg",
        endonym: "Български",
        english: "Bulgarian",
    },
    Lang {
        code: "el",
        endonym: "Ελληνικά",
        english: "Greek",
    },
    Lang {
        code: "hr",
        endonym: "Hrvatski",
        english: "Croatian",
    },
    Lang {
        code: "sr",
        endonym: "Српски",
        english: "Serbian",
    },
    Lang {
        code: "bs",
        endonym: "Bosanski",
        english: "Bosnian",
    },
    Lang {
        code: "mk",
        endonym: "Македонски",
        english: "Macedonian",
    },
    Lang {
        code: "sq",
        endonym: "Shqip",
        english: "Albanian",
    },
    Lang {
        code: "ga",
        endonym: "Gaeilge",
        english: "Irish",
    },
    Lang {
        code: "mt",
        endonym: "Malti",
        english: "Maltese",
    },
    Lang {
        code: "be",
        endonym: "Беларуская",
        english: "Belarusian",
    },
    Lang {
        code: "ru",
        endonym: "Русский",
        english: "Russian",
    },
    Lang {
        code: "tr",
        endonym: "Türkçe",
        english: "Turkish",
    },
    Lang {
        code: "lb",
        endonym: "Lëtzebuergesch",
        english: "Luxembourgish",
    },
    Lang {
        code: "ca",
        endonym: "Català",
        english: "Catalan",
    },
];

/// The endonym for a code, or the code itself if unknown.
pub fn endonym(code: &str) -> &str {
    EUROPEAN_LANGS
        .iter()
        .find(|l| l.code == code)
        .map(|l| l.endonym)
        .unwrap_or(code)
}

/// Embedded translation JSON for fully-translated locales.
fn embedded(code: &str) -> Option<&'static str> {
    match code {
        "en" => Some(include_str!("../../../assets/i18n/en.json")),
        "pl" => Some(include_str!("../../../assets/i18n/pl.json")),
        "de" => Some(include_str!("../../../assets/i18n/de.json")),
        "fr" => Some(include_str!("../../../assets/i18n/fr.json")),
        "es" => Some(include_str!("../../../assets/i18n/es.json")),
        "it" => Some(include_str!("../../../assets/i18n/it.json")),
        "uk" => Some(include_str!("../../../assets/i18n/uk.json")),
        _ => None,
    }
}

/// PNG bytes for a language's flag (keyed by language code).
pub fn flag_png(code: &str) -> Option<&'static [u8]> {
    macro_rules! flag {
        ($c:literal) => {
            include_bytes!(concat!("../../../assets/flags/", $c, ".png")).as_slice()
        };
    }
    Some(match code {
        "en" => flag!("en"),
        "pl" => flag!("pl"),
        "de" => flag!("de"),
        "fr" => flag!("fr"),
        "es" => flag!("es"),
        "it" => flag!("it"),
        "uk" => flag!("uk"),
        "pt" => flag!("pt"),
        "nl" => flag!("nl"),
        "sv" => flag!("sv"),
        "no" => flag!("no"),
        "da" => flag!("da"),
        "fi" => flag!("fi"),
        "is" => flag!("is"),
        "et" => flag!("et"),
        "lv" => flag!("lv"),
        "lt" => flag!("lt"),
        "cs" => flag!("cs"),
        "sk" => flag!("sk"),
        "sl" => flag!("sl"),
        "hu" => flag!("hu"),
        "ro" => flag!("ro"),
        "bg" => flag!("bg"),
        "el" => flag!("el"),
        "hr" => flag!("hr"),
        "sr" => flag!("sr"),
        "bs" => flag!("bs"),
        "mk" => flag!("mk"),
        "sq" => flag!("sq"),
        "ga" => flag!("ga"),
        "mt" => flag!("mt"),
        "be" => flag!("be"),
        "ru" => flag!("ru"),
        "tr" => flag!("tr"),
        "lb" => flag!("lb"),
        "ca" => flag!("ca"),
        _ => return None,
    })
}

fn parse(json: &str) -> HashMap<String, String> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Holds the active translation map plus an English fallback.
pub struct I18n {
    lang: String,
    map: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

impl I18n {
    pub fn new(lang: &str) -> Self {
        let fallback = parse(embedded(DEFAULT_LANG).unwrap_or("{}"));
        let mut i18n = Self {
            lang: DEFAULT_LANG.to_string(),
            map: HashMap::new(),
            fallback,
        };
        i18n.set_lang(lang);
        i18n
    }

    pub fn set_lang(&mut self, lang: &str) {
        self.lang = lang.to_string();
        self.map = embedded(lang).map(parse).unwrap_or_default();
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }

    /// Translate a key: active locale → English fallback → the key itself.
    /// Keys are compile-time literals, so the fallback can be returned directly.
    pub fn t<'a>(&'a self, key: &'static str) -> &'a str {
        self.map
            .get(key)
            .or_else(|| self.fallback.get(key))
            .map(String::as_str)
            .unwrap_or(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_has_all_keys_others_match() {
        let en = parse(embedded("en").unwrap());
        assert!(!en.is_empty());
        for code in ["pl", "de", "fr", "es", "it", "uk"] {
            let m = parse(embedded(code).unwrap());
            for key in en.keys() {
                assert!(m.contains_key(key), "locale {code} missing key {key}");
            }
            assert_eq!(m.len(), en.len(), "locale {code} has extra/old keys");
        }
    }

    #[test]
    fn fallback_to_english_for_untranslated() {
        let i = I18n::new("cs"); // not seeded
        assert_eq!(i.t("form.create"), "Create");
    }

    #[test]
    fn translated_locale_overrides() {
        let i = I18n::new("pl");
        assert_eq!(i.t("form.create"), "Utwórz");
        assert_eq!(i.t("nonexistent.key"), "nonexistent.key");
    }

    #[test]
    fn every_offered_language_has_a_flag() {
        for l in EUROPEAN_LANGS {
            assert!(flag_png(l.code).is_some(), "no flag for {}", l.code);
        }
    }
}
