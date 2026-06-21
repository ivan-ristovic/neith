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
fn extracts_regex_seed() {
    assert_eq!(
        longest_literal_seed("awk.*print"),
        Some("print".to_string())
    );
    assert_eq!(longest_literal_seed("a.*b"), None);
}
