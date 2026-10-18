//! Numbers in the separators of the interface language, from CLDR: "1,234.5"
//! in English, "1.234,5" in German, "1 234,5" in French. Only the languages
//! of the catalogs are here. Another language gets the English separators.
//!
//! Use these for numbers that the user reads. A number that the user edits
//! in an input, or that goes into `config.toml`, keeps the decimal point.

/// A no-break space, the French and Russian group separator. CLDR gives
/// French the narrow no-break space (U+202F), but the bundled fonts do not
/// have it, so French text uses this one everywhere.
const NBSP: char = '\u{A0}';

struct Separators {
    decimal: char,
    group: char,
    /// The fewest digits before the first separator: Spanish writes "1234"
    /// but "12 345".
    min_grouping: usize,
}

fn separators(locale: &str) -> Separators {
    let language = locale.split(['-', '_']).next().unwrap_or(locale);
    match language {
        "de" | "it" => Separators { decimal: ',', group: '.', min_grouping: 1 },
        "es" => Separators { decimal: ',', group: '.', min_grouping: 2 },
        "pt" => Separators { decimal: ',', group: '.', min_grouping: 1 },
        "fr" => Separators { decimal: ',', group: NBSP, min_grouping: 1 },
        "ru" => Separators { decimal: ',', group: NBSP, min_grouping: 1 },
        _ => Separators { decimal: '.', group: ',', min_grouping: 1 },
    }
}

/// A whole number with group separators: "1,234", "1.234", "1 234".
pub fn integer(n: u64) -> String {
    integer_in(&crate::locale(), n)
}

/// A number with `places` decimals: "1.5", "1,5".
pub fn decimal(value: f64, places: usize) -> String {
    decimal_in(&crate::locale(), value, places)
}

fn integer_in(locale: &str, n: u64) -> String {
    let s = separators(locale);
    let digits = n.to_string();
    if digits.len() < 4 + s.min_grouping - 1 {
        return digits;
    }
    let mut out = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(s.group);
        }
        out.push(d);
    }
    out
}

fn decimal_in(locale: &str, value: f64, places: usize) -> String {
    let s = separators(locale);
    let text = format!("{value:.places$}");
    let (whole, fraction) = text.split_once('.').map_or((text.as_str(), None), |(w, f)| (w, Some(f)));
    let (sign, whole) = whole.strip_prefix('-').map_or(("", whole), |w| ("-", w));
    let whole = whole.parse::<u64>().map_or_else(|_| whole.to_string(), |n| integer_in(locale, n));
    match fraction {
        Some(f) => format!("{sign}{whole}{}{f}", s.decimal),
        None => format!("{sign}{whole}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_and_cjk() {
        for locale in ["en", "ja", "zh-CN", "ko"] {
            assert_eq!(integer_in(locale, 999), "999");
            assert_eq!(integer_in(locale, 1234), "1,234");
            assert_eq!(integer_in(locale, 1_234_567), "1,234,567");
            assert_eq!(decimal_in(locale, 1.25, 1), "1.2");
        }
    }

    #[test]
    fn decimal_comma_languages() {
        assert_eq!(integer_in("de", 1234), "1.234");
        assert_eq!(integer_in("it", 1_234_567), "1.234.567");
        assert_eq!(integer_in("pt-BR", 1234), "1.234");
        assert_eq!(decimal_in("de", 1.5, 1), "1,5");
        assert_eq!(decimal_in("pt-BR", 12345.0, 1), "12.345,0");
    }

    #[test]
    fn spanish_groups_from_five_digits() {
        assert_eq!(integer_in("es", 1234), "1234");
        assert_eq!(integer_in("es", 12345), "12.345");
        assert_eq!(decimal_in("es", 1.5, 1), "1,5");
    }

    #[test]
    fn french_and_russian_group_with_spaces() {
        assert_eq!(integer_in("fr", 1234), "1\u{A0}234");
        assert_eq!(integer_in("ru", 1_234_567), "1\u{A0}234\u{A0}567");
        assert_eq!(decimal_in("fr", 94.12, 1), "94,1");
    }

    #[test]
    fn negative_and_no_decimals() {
        assert_eq!(decimal_in("de", -1234.5, 1), "-1.234,5");
        assert_eq!(decimal_in("de", 45.4, 0), "45");
    }
}
