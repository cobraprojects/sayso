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
        "es" | "pt" | "fr" | "it" => match n {
            1 => "one",
            _ if is_million(n) => "many",
            _ => "other",
        },
        _ => match n {
            1 => "one",
            _ => "other",
        },
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
    fn chinese_and_japanese_have_no_plural() {
        for n in [0, 1, 2, 1_000_000] {
            assert_eq!(category("zh-CN", n), "other");
            assert_eq!(category("ja", n), "other");
        }
    }
}
