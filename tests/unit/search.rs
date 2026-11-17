use super::*;

#[test]
fn snippet_prefers_matching_line() {
    let (line, snippet) = best_snippet("alpha\nbeta gamma\n", "gamma");
    assert_eq!(line, 2);
    assert_eq!(snippet, "beta gamma");
}

#[test]
fn source_filter_matches_man() {
    assert!(source_matches(SourceFilter::Man, "man"));
    assert!(!source_matches(SourceFilter::Man, "note"));
}

#[test]
fn ordinal_column_query_boosts_selected_field_titles() {
    let mut selected = SearchResult {
        title: "Print Selected Fields With awk".to_string(),
        path: PathBuf::from("/tmp/awk/print-selected-fields.md"),
        rel_path: "awk/print-selected-fields.md".to_string(),
        library_alias: "test".to_string(),
        source_kind: "note".to_string(),
        line: 1,
        snippet: String::new(),
        score: 100.0,
        rank_reason: String::new(),
        body: String::new(),
        is_live_man: false,
    };
    let mut aligned = SearchResult {
        title: "Print Aligned Table With awk".to_string(),
        path: PathBuf::from("/tmp/awk/print-aligned-table.md"),
        rel_path: "awk/print-aligned-table.md".to_string(),
        library_alias: "test".to_string(),
        source_kind: "note".to_string(),
        line: 1,
        snippet: String::new(),
        score: 120.0,
        rank_reason: String::new(),
        body: String::new(),
        is_live_man: false,
    };

    apply_boosts(
        &mut selected,
        "awk print selected column",
        SourceFilter::All,
        "fuzzy",
    );
    apply_boosts(
        &mut aligned,
        "awk print selected column",
        SourceFilter::All,
        "fuzzy",
    );

    assert!(selected.score > aligned.score);
}

#[test]
fn sectioned_man_query_boosts_exact_page() {
    let mut exact = SearchResult {
        title: "man: printf (3)".to_string(),
        path: PathBuf::from("/tmp/man/printf-3.md"),
        rel_path: "man/printf-3-a88efd3cb696.md".to_string(),
        library_alias: "devdocs".to_string(),
        source_kind: "man".to_string(),
        line: 1,
        snippet: String::new(),
        score: 100.0,
        rank_reason: String::new(),
        body: String::new(),
        is_live_man: false,
    };
    let mut distractor = SearchResult {
        title: "man: gnutls_psk_format_imported_identity (3)".to_string(),
        path: PathBuf::from("/tmp/man/gnutls.md"),
        rel_path: "man/gnutls-psk-format-imported-identity-3.md".to_string(),
        library_alias: "devdocs".to_string(),
        source_kind: "man".to_string(),
        line: 1,
        snippet: String::new(),
        score: 5_000.0,
        rank_reason: String::new(),
        body: String::new(),
        is_live_man: false,
    };

    apply_boosts(&mut exact, "printf(3) format", SourceFilter::All, "fuzzy");
    apply_boosts(
        &mut distractor,
        "printf(3) format",
        SourceFilter::All,
        "fuzzy",
    );

    assert!(exact.score > distractor.score);
}

#[test]
fn live_man_boosts_above_indexed_devdocs_man() {
    let mut live = SearchResult {
        title: "man: printf(3)".to_string(),
        path: PathBuf::from("/tmp/live/printf.3.txt"),
        rel_path: "man:printf(3)".to_string(),
        library_alias: "live-man".to_string(),
        source_kind: "man".to_string(),
        line: 1,
        snippet: String::new(),
        score: 100.0,
        rank_reason: String::new(),
        body: String::new(),
        is_live_man: true,
    };
    let mut indexed = SearchResult {
        title: "man: printf (3)".to_string(),
        path: PathBuf::from("/tmp/devdocs/man/printf-3.md"),
        rel_path: "man/printf-3-a88efd3cb696.md".to_string(),
        library_alias: "devdocs".to_string(),
        source_kind: "man".to_string(),
        line: 1,
        snippet: String::new(),
        score: 5_000.0,
        rank_reason: String::new(),
        body: String::new(),
        is_live_man: false,
    };

    apply_boosts(&mut live, "printf(3) format", SourceFilter::All, "live-man");
    apply_boosts(&mut indexed, "printf(3) format", SourceFilter::All, "index");

    assert!(live.score > indexed.score);
}

#[test]
fn trims_snippet_on_utf8_boundary() {
    let snippet = trim_snippet(&"é".repeat(200));
    assert!(snippet.ends_with("..."));
    assert!(snippet.is_char_boundary(snippet.len()));
}

#[test]
fn regex_matches_full_body() {
    let result = SearchResult {
        title: "title".to_string(),
        path: PathBuf::from("/tmp/doc.md"),
        rel_path: "doc.md".to_string(),
        library_alias: "test".to_string(),
        source_kind: "note".to_string(),
        line: 1,
        snippet: "front matter".to_string(),
        score: 1.0,
        rank_reason: String::new(),
        body: "intro\nalpha then beta\n".to_string(),
        is_live_man: false,
    };
    let regex = regex::RegexBuilder::new("alpha.*beta")
        .case_insensitive(true)
        .build()
        .unwrap();

    assert_eq!(
        regex_match_snippet(&regex, &result, SourceFilter::All),
        Some((2, "alpha then beta".to_string()))
    );
}

#[test]
fn regex_matches_across_body_lines() {
    let result = SearchResult {
        title: "title".to_string(),
        path: PathBuf::from("/tmp/doc.md"),
        rel_path: "doc.md".to_string(),
        library_alias: "test".to_string(),
        source_kind: "note".to_string(),
        line: 1,
        snippet: String::new(),
        score: 1.0,
        rank_reason: String::new(),
        body: "intro\nalpha\nbeta\n".to_string(),
        is_live_man: false,
    };
    let regex = regex::RegexBuilder::new("(?s)alpha.*beta")
        .case_insensitive(true)
        .build()
        .unwrap();

    assert_eq!(
        regex_match_snippet(&regex, &result, SourceFilter::All),
        Some((2, "alpha".to_string()))
    );
}

#[test]
fn regex_prefilter_never_rejects_matching_text() {
    for pattern in [
        "foo|bar",
        "^Foo$",
        "(?i)foo",
        "f.o",
        "(?s)alpha.*beta",
        r"\d+",
        "[a-z]+",
    ] {
        let regex = regex::RegexBuilder::new(pattern)
            .case_insensitive(true)
            .build()
            .unwrap();
        let prefilter = RegexPrefilter::new(pattern);
        for text in ["Foo", "bar", "alpha\nbeta", "number 42", "unrelated"] {
            if regex.is_match(text) {
                let bloom = SearchBloom::for_entry("title", "path.md", text);
                assert!(
                    prefilter
                        .as_ref()
                        .is_none_or(|prefilter| prefilter.might_match(Some(bloom))),
                    "pattern {pattern:?} rejected matching text {text:?}"
                );
            }
        }
    }
}

#[test]
fn top_result_collection_is_bounded_and_ranked() {
    let mut results = BinaryHeap::new();
    for score in [1.0, 3.0, 2.0] {
        let result = SearchResult {
            title: score.to_string(),
            path: PathBuf::from(format!("/tmp/{score}.md")),
            rel_path: format!("{score}.md"),
            library_alias: "test".to_string(),
            source_kind: "note".to_string(),
            line: 1,
            snippet: String::new(),
            score,
            rank_reason: "regex".to_string(),
            body: String::new(),
            is_live_man: false,
        };
        retain_top_result(&mut results, result, 2);
    }
    let results = results
        .into_sorted_vec()
        .into_iter()
        .map(|entry| entry.result)
        .collect::<Vec<_>>();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].score, 3.0);
    assert_eq!(results[1].score, 2.0);
}
