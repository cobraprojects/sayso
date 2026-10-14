//! The pseudo-locale: "Save" becomes "[Šåvé~~]". Text in brackets came from a
//! catalog. The padding makes each text about 30% longer, as many
//! translations are, so text that does not fit shows too.

pub(crate) fn transform(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2 + 2);
    out.push('[');
    let mut rest = text;
    let mut letters: usize = 0;
    while !rest.is_empty() {
        // Placeholders stay as they are, so the arguments still go in.
        if rest.starts_with("%{")
            && let Some(end) = rest.find('}')
        {
            out.push_str(&rest[..=end]);
            rest = &rest[end + 1..];
            continue;
        }
        let c = rest.chars().next().expect("rest is not empty");
        if c.is_alphabetic() {
            letters += 1;
        }
        out.push(accent(c));
        rest = &rest[c.len_utf8()..];
    }
    out.extend(std::iter::repeat_n('~', letters.div_ceil(3)));
    out.push(']');
    out
}

fn accent(c: char) -> char {
    match c {
        'a' => 'å',
        'c' => 'ç',
        'e' => 'é',
        'i' => 'î',
        'n' => 'ñ',
        'o' => 'ö',
        's' => 'š',
        'u' => 'ü',
        'y' => 'ý',
        'z' => 'ž',
        'A' => 'Å',
        'C' => 'Ç',
        'E' => 'É',
        'I' => 'Î',
        'N' => 'Ñ',
        'O' => 'Ö',
        'S' => 'Š',
        'U' => 'Ü',
        'Y' => 'Ý',
        'Z' => 'Ž',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::transform;

    #[test]
    fn accents_pads_and_brackets() {
        assert_eq!(transform("Save"), "[Šåvé~~]");
    }

    #[test]
    fn keeps_placeholders() {
        assert_eq!(transform("Hi %{name}"), "[Hî %{name}~]");
    }

    #[test]
    fn keeps_an_unclosed_brace() {
        assert_eq!(transform("%{x"), "[%{x~]");
    }
}
