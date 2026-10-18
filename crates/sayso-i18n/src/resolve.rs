//! Which locale to use, from the setting and the languages of the system.

use crate::{HAS_PSEUDO, PSEUDO, SYSTEM, available};

/// The locale for the setting `general.language`. "system", and a value that
/// is not an available locale, take the first system language that has one.
/// The last choice is English.
pub fn resolve(setting: &str, system: &[String]) -> &'static str {
    let codes: Vec<&'static str> = available().map(|l| l.code).collect();
    resolve_in(setting, system, &codes)
}

/// The languages of the system, in the order of preference.
pub fn system_locales() -> Vec<String> {
    sys_locale::get_locales().collect()
}

fn resolve_in(setting: &str, system: &[String], codes: &[&'static str]) -> &'static str {
    if setting == PSEUDO && HAS_PSEUDO {
        return PSEUDO;
    }
    if setting != SYSTEM
        && let Some(code) = codes.iter().find(|c| c.eq_ignore_ascii_case(setting))
    {
        return code;
    }
    for tag in system {
        let tag = Tag::parse(tag);
        if let Some(code) = codes.iter().find(|c| Tag::parse(c) == tag) {
            return code;
        }
        if let Some(code) = codes.iter().find(|c| tag.same_language(&Tag::parse(c))) {
            return code;
        }
    }
    "en"
}

/// The parts of a language tag that matter here.
#[derive(Debug, PartialEq, Eq)]
struct Tag {
    language: String,
    /// "Hans" or "Hant" for Chinese, from the script or the region.
    script: Option<String>,
    region: Option<String>,
}

impl Tag {
    /// Parse "zh-Hans-CN", "pt_BR", or the POSIX "zh_TW.UTF-8".
    fn parse(tag: &str) -> Self {
        let tag = tag.split(['.', '@']).next().unwrap_or_default();
        let mut parts = tag.split(['-', '_']);
        let language = parts.next().unwrap_or_default().to_ascii_lowercase();
        let (mut script, mut region) = (None, None);
        for part in parts {
            if part.len() == 4 && part.chars().all(|c| c.is_ascii_alphabetic()) {
                script = Some(title_case(part));
            } else if part.len() == 2 || (part.len() == 3 && part.chars().all(|c| c.is_ascii_digit())) {
                region = Some(part.to_ascii_uppercase());
            }
        }
        if language == "zh" && script.is_none() {
            script = Some(match region.as_deref() {
                Some("TW" | "HK" | "MO") => "Hant".into(),
                _ => "Hans".into(),
            });
        }
        Self { language, script, region }
    }

    /// The same language, and for Chinese also the same script.
    fn same_language(&self, other: &Tag) -> bool {
        self.language == other.language && (self.language != "zh" || self.script == other.script)
    }
}

fn title_case(s: &str) -> String {
    let mut out = s.to_ascii_lowercase();
    if let Some(first) = out.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODES: &[&str] = &["en", "es", "pt-BR", "zh-CN", "ja", "de", "fr", "it", "ko", "ru"];

    fn r(setting: &str, system: &[&str]) -> &'static str {
        let system: Vec<String> = system.iter().map(|s| s.to_string()).collect();
        resolve_in(setting, &system, CODES)
    }

    #[test]
    fn a_known_setting_wins() {
        assert_eq!(r("ja", &["es-ES"]), "ja");
        assert_eq!(r("pt-br", &["en"]), "pt-BR");
    }

    #[test]
    fn system_takes_the_first_language_with_a_catalog() {
        assert_eq!(r("system", &["nl-NL", "es-MX", "en-US"]), "es");
        assert_eq!(r("system", &["en-GB"]), "en");
        assert_eq!(r("system", &["ja-JP"]), "ja");
    }

    #[test]
    fn an_unknown_setting_follows_the_system() {
        assert_eq!(r("xx", &["ja-JP"]), "ja");
    }

    #[test]
    fn regional_variants_get_their_language() {
        assert_eq!(r("system", &["de-AT"]), "de");
        assert_eq!(r("system", &["de_CH.UTF-8"]), "de");
        assert_eq!(r("system", &["fr-CA"]), "fr");
        assert_eq!(r("system", &["it-CH"]), "it");
        assert_eq!(r("system", &["ko-KR"]), "ko");
        assert_eq!(r("system", &["ru-UA"]), "ru");
    }

    #[test]
    fn portuguese_variants_get_brazilian_portuguese() {
        assert_eq!(r("system", &["pt-PT"]), "pt-BR");
        assert_eq!(r("system", &["pt_BR.UTF-8"]), "pt-BR");
    }

    #[test]
    fn simplified_chinese_matches_by_script_or_region() {
        assert_eq!(r("system", &["zh-Hans-CN"]), "zh-CN");
        assert_eq!(r("system", &["zh-Hans-SG"]), "zh-CN");
        assert_eq!(r("system", &["zh_CN.UTF-8"]), "zh-CN");
        assert_eq!(r("system", &["zh"]), "zh-CN");
    }

    #[test]
    fn traditional_chinese_does_not_get_simplified() {
        assert_eq!(r("system", &["zh-Hant-TW"]), "en");
        assert_eq!(r("system", &["zh-TW"]), "en");
        assert_eq!(r("system", &["zh-HK", "ja"]), "ja");
        // An explicit script wins over the region.
        assert_eq!(r("system", &["zh-Hant-CN"]), "en");
    }

    #[test]
    fn nothing_known_gives_english() {
        assert_eq!(r("system", &[]), "en");
        assert_eq!(r("system", &["C", "POSIX", "nl"]), "en");
    }

    #[test]
    fn pseudo_locale() {
        assert_eq!(r(PSEUDO, &["ja"]), if HAS_PSEUDO { PSEUDO } else { "ja" });
    }
}
