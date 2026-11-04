//! Filtering for the searchable dialogs (keyboard shortcuts and settings).

/// Whether a row matches `query`.
///
/// The query is split on whitespace and every word must appear, case
/// insensitively, in at least one of `fields` (a row's label, its current value
/// and so on). An empty query matches everything.
#[must_use]
pub fn matches<S: AsRef<str>>(query: &str, fields: &[S]) -> bool {
    let fields: Vec<String> = fields.iter().map(|f| f.as_ref().to_lowercase()).collect();
    query
        .split_whitespace()
        .map(str::to_lowercase)
        .all(|word| fields.iter().any(|field| field.contains(&word)))
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn empty_query_matches_everything() {
        assert!(matches("", &["Timer"]));
        assert!(matches("   ", &["Timer"]));
        assert!(matches::<&str>("", &[]));
    }

    #[test]
    fn matching_ignores_case() {
        assert!(matches("SPACE", &["Start timer", "space"]));
        assert!(matches("ctrl", &["Ctrl + ,"]));
    }

    #[test]
    fn every_word_must_match_some_field() {
        assert!(matches("timer ctrl", &["Start timer", "Ctrl + T"]));
        assert!(!matches("timer shift", &["Start timer", "Ctrl + T"]));
    }

    #[test]
    fn matches_translated_text_with_accents() {
        assert!(matches("sin asignar", &["Siguiente sesión", "Sin asignar"]));
        assert!(matches("SESIÓN", &["Siguiente sesión"]));
    }

    #[test]
    fn no_fields_match_only_an_empty_query() {
        assert!(!matches::<&str>("x", &[]));
    }
}
