use super::*;

#[test]
fn extracts_markdown_title() {
    assert_eq!(extract_title("# Hello\nbody"), Some("Hello".to_string()));
}

#[test]
fn classifies_man_doc() {
    assert_eq!(
        classify_source(Path::new("/tmp/lib"), "man/awk.md", "# man: awk"),
        SourceKind::Man
    );
}

#[test]
fn infers_known_aliases() {
    assert_eq!(
        infer_alias(Path::new("/home/ivan/neith/neith-devdocs/generated")),
        "devdocs"
    );
    assert_eq!(
        infer_alias(Path::new("/home/ivan/neith/neith-lib")),
        "neith-lib"
    );
}

#[test]
fn library_ignore_excludes_exact_files_and_directory_prefixes() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(".neithignore"),
        "AGENTS.md\n.hidden/\nskip/\n",
    )
    .unwrap();
    fs::write(temp.path().join("AGENTS.md"), "# Instructions\n").unwrap();
    fs::write(temp.path().join("keep.md"), "# Keep\n").unwrap();
    fs::create_dir(temp.path().join("skip")).unwrap();
    fs::write(temp.path().join("skip").join("note.md"), "# Skip\n").unwrap();
    fs::create_dir(temp.path().join(".hidden")).unwrap();
    fs::write(temp.path().join(".hidden").join("note.md"), "# Hidden\n").unwrap();

    let library = Library::new(temp.path().to_path_buf(), Some("test".to_string()), None);
    let rel_paths = discover_markdown_entries(&library)
        .unwrap()
        .into_iter()
        .map(|entry| entry.rel_path)
        .collect::<Vec<_>>();

    assert_eq!(rel_paths, vec!["keep.md"]);
}

#[test]
fn library_ignore_comments_and_blank_lines_are_ignored() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(".neithignore"),
        "\n# comment\n./ignored.md\n",
    )
    .unwrap();
    fs::write(temp.path().join("ignored.md"), "# Ignored\n").unwrap();
    fs::write(temp.path().join("included.md"), "# Included\n").unwrap();

    let library = Library::new(temp.path().to_path_buf(), Some("test".to_string()), None);
    let rel_paths = discover_markdown_entries(&library)
        .unwrap()
        .into_iter()
        .map(|entry| entry.rel_path)
        .collect::<Vec<_>>();

    assert_eq!(rel_paths, vec!["included.md"]);
}

#[test]
fn excerpt_truncates_on_utf8_boundary() {
    let body = format!("# {}\n", "é".repeat(600));
    let excerpt = build_excerpt(&body, 900);
    assert!(excerpt.ends_with("..."));
    assert!(excerpt.is_char_boundary(excerpt.len()));
}
