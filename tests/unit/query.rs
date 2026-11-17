use super::*;

#[test]
fn normalizes_ordinal_query_terms() {
    assert_eq!(
        normalize_query("awk print 3rd column"),
        "awk print selected column"
    );
    assert_eq!(normalize_query("print nth column"), "print selected column");
    assert_eq!(
        normalize_query("awk '{ print $3 }'"),
        "awk '{ print selected }'"
    );
}

#[test]
fn detects_regex_syntax_without_treating_awk_field_refs_as_regex() {
    assert!(has_regex_meta("^alpha$|^beta$"));
    assert!(has_regex_meta(r"\d+"));
    assert!(!has_regex_meta("awk $3"));
}
