//! .NET parsing semantics the game relies on (`docs/spec/ir.md` §3.3): failure yields
//! `false` / `0`, never an error.

/// `Boolean.TryParse`: `"true"` / `"false"`, case-insensitive, surrounding whitespace ignored.
pub fn bool_try_parse(s: &str) -> bool {
    s.trim().eq_ignore_ascii_case("true")
}

/// `Single.TryParse` (invariant culture): decimal or exponent notation, surrounding
/// whitespace ignored; anything else yields 0.
pub fn single_try_parse(s: &str) -> f32 {
    let t = s.trim();
    let ok = !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E'));
    if ok { t.parse().unwrap_or(0.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_strings_fail_like_dotnet() {
        assert!(!bool_try_parse(""));
        assert!(bool_try_parse(" True "));
        assert_eq!(single_try_parse(""), 0.0);
        assert_eq!(single_try_parse("1.2"), 1.2);
        assert_eq!(single_try_parse("x"), 0.0);
    }
}
