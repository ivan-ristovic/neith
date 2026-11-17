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

#[cfg(unix)]
#[test]
fn unchanged_index_keeps_manifest_and_catalog_files() {
    use std::os::unix::fs::MetadataExt;

    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("note.md"), "# Note\n\nBody\n").unwrap();
    let library = temp_library(temp.path());
    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();
    let manifest_inode = std::fs::metadata(manifest_path(&library)).unwrap().ino();
    let catalog_inode = std::fs::metadata(catalog_path(&library)).unwrap().ino();

    let stats =
        ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();
    assert_eq!(stats[0].unchanged, 1);
    assert_eq!(
        std::fs::metadata(manifest_path(&library)).unwrap().ino(),
        manifest_inode
    );
    assert_eq!(
        std::fs::metadata(catalog_path(&library)).unwrap().ino(),
        catalog_inode
    );
}

#[test]
fn exact_regex_matches_every_alternative_and_anchors() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("alpha.md"), "# Alpha\n").unwrap();
    std::fs::write(temp.path().join("beta.md"), "# Beta\n\nnumber 42\n").unwrap();
    let library = temp_library(temp.path());
    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();
    let engine = SearchEngine::new(IndexManager::open(&[library]).unwrap());

    let search = |query: &str| {
        engine.search(&SearchRequest {
            query: query.to_string(),
            filter: SourceFilter::All,
            mode: MatchMode::Exact,
            library: LibraryScope::All,
            limit: 10,
        })
    };
    assert_eq!(search("^Alpha$|^Beta$").len(), 2);
    assert_eq!(search(r"\d+").len(), 1);
}

#[test]
fn empty_query_filters_before_limiting_catalog() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("a-note.md"), "# A note\n").unwrap();
    std::fs::write(temp.path().join("z-man.md"), "# man: sample(1)\n").unwrap();
    let library = temp_library(temp.path());
    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();
    let engine = SearchEngine::new(IndexManager::open(&[library]).unwrap());

    let results = engine.search(&SearchRequest {
        query: String::new(),
        filter: SourceFilter::Man,
        mode: MatchMode::Fuzzy,
        library: LibraryScope::All,
        limit: 1,
    });
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].rel_path, "z-man.md");
}

#[test]
fn sectioned_man_query_keeps_catalog_lookup_in_exact_mode() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("man")).unwrap();
    std::fs::write(
        temp.path().join("man/printf-3-example.md"),
        "# man: printf (3)\n",
    )
    .unwrap();
    let library = temp_library(temp.path());
    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();
    let engine = SearchEngine::new(IndexManager::open(&[library]).unwrap());

    let results = engine.search(&SearchRequest {
        query: "printf(3)".to_string(),
        filter: SourceFilter::Man,
        mode: MatchMode::Exact,
        library: LibraryScope::Alias("test".to_string()),
        limit: 10,
    });
    assert!(
        results
            .iter()
            .any(|result| result.rel_path == "man/printf-3-example.md")
    );
}
