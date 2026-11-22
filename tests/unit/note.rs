use super::*;

#[test]
fn slugifies_query() {
    assert_eq!(slugify("Awk print 3rd column"), "awk-print-3rd-column");
    assert_eq!(slugify("Život notes"), "život-notes");
    assert!(validate_note_title("✨").is_err());
}

#[test]
fn folder_prefix_is_excluded_from_note_filename() {
    let temp = tempfile::tempdir().unwrap();
    let library = Library::new(temp.path().to_path_buf(), Some("notes".to_string()), None);
    fs::create_dir_all(temp.path().join("shell tools")).unwrap();
    fs::create_dir_all(temp.path().join("My Dir")).unwrap();
    assert_eq!(
        infer_note_path_in_library(&library, "bash enable strict mode").unwrap(),
        temp.path().join("bash/enable-strict-mode.md")
    );
    assert_eq!(
        infer_note_path_in_library(&library, "\"shell tools\" safe aliases").unwrap(),
        temp.path().join("shell tools/safe-aliases.md")
    );
    assert_eq!(
        infer_note_path_from_parts(&library, "shell tools", "safe aliases").unwrap(),
        temp.path().join("shell tools/safe-aliases.md")
    );
    assert_eq!(
        infer_note_path_in_library(&library, "\"My Dir/new\" nested idea").unwrap(),
        temp.path().join("My Dir/new/nested-idea.md")
    );
    assert_eq!(
        replace_note_folder("bash enable strict mode", "shell tools"),
        "\"shell tools\" enable strict mode"
    );
    assert!(infer_note_path_in_library(&library, "bash").is_err());
    assert!(infer_note_path_in_library(&library, "../other title").is_err());
}

#[cfg(unix)]
#[test]
fn folder_picker_uses_existing_library_directories() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let library = Library::new(temp.path().to_path_buf(), Some("notes".to_string()), None);
    fs::create_dir_all(temp.path().join("awk/subtopic")).unwrap();
    fs::create_dir_all(temp.path().join(".neith-cache/hidden")).unwrap();
    assert_eq!(
        list_note_folders(&library).unwrap(),
        ["awk", "awk/subtopic"]
    );

    let picker = temp.path().join("picker.sh");
    fs::write(
        &picker,
        "#!/bin/sh\ncat >/dev/null\nprintf 'awk/subtopic\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&picker, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        pick_note_folder_with_program(&library, picker.to_str().unwrap()).unwrap(),
        Some("awk/subtopic".to_string())
    );
}

#[cfg(unix)]
#[test]
fn note_folder_cannot_follow_symlink_outside_library() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let library_path = temp.path().join("library");
    let outside = temp.path().join("outside");
    fs::create_dir(&library_path).unwrap();
    fs::create_dir(&outside).unwrap();
    symlink(&outside, library_path.join("escape")).unwrap();
    symlink(&outside, library_path.join("safe")).unwrap();
    let library = Library::new(library_path, Some("notes".to_string()), None);

    assert!(infer_note_path_in_library(&library, "escape private note").is_err());
    assert!(infer_note_path_in_library(&library, "Safe private note").is_err());
}

#[test]
fn creates_note_from_nearest_template() {
    let temp = tempfile::tempdir().unwrap();
    let category = temp.path().join("awk");
    fs::create_dir(&category).unwrap();
    fs::write(
        temp.path().join(TEMPLATE_FILE),
        "# {{TITLE}}\n\nTask: {{QUERY}}\nSlug: {{SLUG}}\n",
    )
    .unwrap();
    let path = category.join("print-selected-fields.md");

    create_note(&path, "awk print 3rd column").unwrap();

    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "# Print Selected Fields\n\nTask: awk print 3rd column\nSlug: print-selected-fields\n"
    );
}

#[test]
fn unsaved_draft_leaves_no_file_or_directory() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("ideas").join("new-idea.md");
    let draft = prepare_note_edit(&path, "New idea").unwrap();
    let draft_path = draft.editor_path().to_path_buf();
    assert!(draft_path.is_file());
    assert!(!path.exists());
    assert_eq!(draft.finish(true).unwrap(), NoteEditOutcome::Discarded);
    assert!(!draft_path.exists());
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn saved_draft_creates_note_without_overwriting_existing_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("ideas").join("new-idea.md");
    let draft = prepare_note_edit(&path, "New idea").unwrap();
    fs::write(draft.editor_path(), "# New idea\n\nActual content\n").unwrap();
    assert_eq!(
        draft.finish(true).unwrap(),
        NoteEditOutcome::Created(path.clone())
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "# New idea\n\nActual content\n"
    );

    let existing = prepare_note_edit(&path, "New idea").unwrap();
    assert!(matches!(existing, NoteEdit::Existing(_)));
    assert_eq!(existing.editor_path(), path);
    assert_eq!(
        existing.finish(true).unwrap(),
        NoteEditOutcome::OpenedExisting
    );
}

#[test]
fn failed_editor_preserves_saved_draft_outside_library() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("ideas/new-idea.md");
    let draft = prepare_note_edit(&path, "New idea").unwrap();
    let draft_path = draft.editor_path().to_path_buf();
    fs::write(&draft_path, "changed but editor failed\n").unwrap();

    assert!(
        draft
            .finish(false)
            .unwrap_err()
            .to_string()
            .contains("edited draft kept at")
    );
    assert!(!path.exists());
    assert!(draft_path.exists());
    fs::remove_file(draft_path).unwrap();
}

#[test]
fn save_conflict_preserves_draft_and_existing_note() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("new-idea.md");
    let draft = prepare_note_edit(&path, "New idea").unwrap();
    let draft_path = draft.editor_path().to_path_buf();
    fs::write(&draft_path, "# Edited draft\n").unwrap();
    fs::write(&path, "# Existing note\n").unwrap();

    let error = draft.finish(true).unwrap_err();
    assert!(error.to_string().contains("edited draft kept at"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "# Existing note\n");
    assert_eq!(fs::read_to_string(&draft_path).unwrap(), "# Edited draft\n");
    fs::remove_file(draft_path).unwrap();
}
