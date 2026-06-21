use super::*;

#[test]
fn slugifies_query() {
    assert_eq!(slugify("Awk print 3rd column"), "awk-print-3rd-column");
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
