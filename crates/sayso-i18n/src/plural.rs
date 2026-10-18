//! The plural category of a count, from the CLDR rules for whole numbers.
//! Only the languages of the catalogs are here. Another language gets
//! "one" for 1 and "other" for the rest, which is the English rule.

pub(crate) fn category(locale: &str, n: u64) -> &'static str {
    let language = locale.split(['-', '_']).next().unwrap_or(locale);
    match language {
        // No plural forms.
        "ja" | "zh" | "ko" => "other",
        // "one" for 0 and 1.
        "pt" if !locale.eq_ignore_ascii_case("pt-PT") => match n {
            0 | 1 => "one",
            _ if is_million(n) => "many",
            _ => "other",
        },
        "fr" => match n {
            0 | 1 => "one",
            _ if is_million(n) => "many",
            _ => "other",
        },
        "es" | "pt" | "it" => match n {
            1 => "one",
            _ if is_million(n) => "many",
            _ => "other",
        },
        // "other" is only for fractions, which counts never are.
        "ru" => match (n % 10, n % 100) {
            (1, 11) => "many",
            (1, _) => "one",
            (2..=4, 12..=14) => "many",
            (2..=4, _) => "few",
            _ => "many",
        },
        _ => match n {
            1 => "one",
            _ => "other",
        },
    }
}

/// The categories that [`category`] gives for the counts of a locale. Each
/// plural of its catalog needs a text for each one, and for "other".
pub(crate) fn categories(locale: &str) -> &'static [&'static str] {
    let language = locale.split(['-', '_']).next().unwrap_or(locale);
    match language {
        "ja" | "zh" | "ko" => &["other"],
        "es" | "pt" | "fr" | "it" => &["one", "many", "other"],
        "ru" => &["one", "few", "many", "other"],
        _ => &["one", "other"],
    }
}

/// CLDR "many" in the Romance languages: a whole number of millions, as in
/// "1 millón de palabras".
fn is_million(n: u64) -> bool {
    n != 0 && n.is_multiple_of(1_000_000)
}

#[cfg(test)]
mod tests {
    use super::category;

    #[test]
    fn english() {
        assert_eq!(category("en", 0), "other");
        assert_eq!(category("en", 1), "one");
        assert_eq!(category("en", 2), "other");
        assert_eq!(category("en", 1_000_000), "other");
    }

    #[test]
    fn spanish() {
        assert_eq!(category("es", 0), "other");
        assert_eq!(category("es", 1), "one");
        assert_eq!(category("es", 21), "other");
        assert_eq!(category("es", 2_000_000), "many");
    }

    #[test]
    fn brazilian_portuguese_counts_zero_as_one() {
        assert_eq!(category("pt-BR", 0), "one");
        assert_eq!(category("pt-BR", 1), "one");
        assert_eq!(category("pt-BR", 2), "other");
        assert_eq!(category("pt-BR", 1_000_000), "many");
        assert_eq!(category("pt-PT", 0), "other");
    }

    #[test]
    fn chinese_japanese_and_korean_have_no_plural() {
        for n in [0, 1, 2, 1_000_000] {
            assert_eq!(category("zh-CN", n), "other");
            assert_eq!(category("ja", n), "other");
            assert_eq!(category("ko", n), "other");
        }
    }

    #[test]
    fn french_counts_zero_as_one() {
        assert_eq!(category("fr", 0), "one");
        assert_eq!(category("fr", 1), "one");
        assert_eq!(category("fr", 2), "other");
        assert_eq!(category("fr", 1_000_000), "many");
        assert_eq!(category("fr-CA", 0), "one");
    }

    #[test]
    fn german_and_italian() {
        assert_eq!(category("de", 0), "other");
        assert_eq!(category("de", 1), "one");
        assert_eq!(category("de", 1_000_000), "other");
        assert_eq!(category("it", 0), "other");
        assert_eq!(category("it", 1), "one");
        assert_eq!(category("it", 1_000_000), "many");
    }

    #[test]
    fn russian() {
        for (n, expected) in [
            (0, "many"),
            (1, "one"),
            (2, "few"),
            (4, "few"),
            (5, "many"),
            (11, "many"),
            (12, "many"),
            (14, "many"),
            (21, "one"),
            (22, "few"),
            (25, "many"),
            (101, "one"),
            (111, "many"),
            (112, "many"),
            (1_000_000, "many"),
        ] {
            assert_eq!(category("ru", n), expected, "{n}");
        }
    }

    #[test]
    fn categories_cover_every_count() {
        for locale in ["en", "es", "pt-BR", "pt-PT", "fr", "it", "de", "ru", "ko", "ja", "zh-CN"] {
            for n in (0..2_000).chain([1_000_000, 2_000_000, 21_000_000]) {
                assert!(super::categories(locale).contains(&category(locale, n)), "{locale} {n}");
            }
        }
    }
}
