use super::*;
use crate::query::{LibraryScope, MatchMode, SearchRequest, SourceFilter};
use crate::search::SearchEngine;

fn temp_library(root: &std::path::Path) -> Library {
    Library::new(root.to_path_buf(), Some("test".to_string()), None)
}

fn search_query(library: &Library, query: &str) -> Vec<crate::search::SearchResult> {
    let manager = IndexManager::open(std::slice::from_ref(library)).unwrap();
    let engine = SearchEngine::new(manager);
    engine.search(&SearchRequest {
        query: query.to_string(),
        filter: SourceFilter::All,
        mode: MatchMode::Fuzzy,
        library: LibraryScope::All,
        limit: 10,
    })
}

#[test]
fn missing_manifest_rebuilds_index_so_deleted_docs_disappear() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("ghost.md"), "# Ghost\n\nuniqueghostterm\n").unwrap();
    let library = temp_library(temp.path());
    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();
    assert_eq!(search_query(&library, "uniqueghostterm").len(), 1);

    std::fs::remove_file(temp.path().join("ghost.md")).unwrap();
    std::fs::remove_file(manifest_path(&library)).unwrap();
    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();

    assert!(search_query(&library, "uniqueghostterm").is_empty());
}

#[test]
fn missing_catalog_makes_index_unusable_and_regenerates() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("note.md"), "# Note\n").unwrap();
    let library = temp_library(temp.path());
    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();
    assert!(IndexManager::has_usable_indexes(std::slice::from_ref(
        &library
    )));

    std::fs::remove_file(catalog_path(&library)).unwrap();
    assert!(!IndexManager::has_usable_indexes(std::slice::from_ref(
        &library
    )));

    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();
    assert!(IndexManager::has_usable_indexes(std::slice::from_ref(
        &library
    )));
}

#[test]
fn catalog_order_follows_discovery_order() {
    let temp = tempfile::tempdir().unwrap();
    for name in ["gamma", "alpha", "beta"] {
        std::fs::write(
            temp.path().join(format!("{name}.md")),
            format!("# {name}\n"),
        )
        .unwrap();
    }
    let library = temp_library(temp.path());

    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();
    let first_order = load_catalog(&library)
        .unwrap()
        .into_iter()
        .map(|entry| entry.rel_path)
        .collect::<Vec<_>>();

    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();
    let second_order = load_catalog(&library)
        .unwrap()
        .into_iter()
        .map(|entry| entry.rel_path)
        .collect::<Vec<_>>();

    assert_eq!(first_order, vec!["alpha.md", "beta.md", "gamma.md"]);
    assert_eq!(second_order, first_order);
}
