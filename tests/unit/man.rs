use super::*;

#[test]
fn parses_name_section_query() {
    assert_eq!(
        split_man_query("printf(3) format"),
        Some(ManQuery {
            title: "printf".to_string(),
            section: Some("3".to_string()),
            remainder: "format".to_string()
        })
    );
}

#[test]
fn parses_section_name_query() {
    assert_eq!(
        split_man_query("3 printf format").unwrap().section,
        Some("3".to_string())
    );
}

#[test]
fn man_lookup_cache_reuses_page_resolution() {
    let mut cache = ManLookupCache::default();
    let mut calls = 0;
    let section = Some("3".to_string());
    cache.get_or_insert_with("printf", &section, || {
        calls += 1;
        Vec::new()
    });
    cache.get_or_insert_with("printf", &section, || {
        calls += 1;
        Vec::new()
    });
    assert_eq!(calls, 1);
}
