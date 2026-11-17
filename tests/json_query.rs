use std::process::Command;

#[test]
fn json_query_refreshes_changed_notes() {
    let temp = tempfile::tempdir().unwrap();
    let note = temp.path().join("note.md");
    std::fs::write(&note, "# Note\n\noriginaluniqueterm\n").unwrap();

    let query = |text: &str| {
        let output = Command::new(env!("CARGO_BIN_EXE_neith"))
            .arg("--config")
            .arg(temp.path().join("missing-config.toml"))
            .arg("--libs")
            .arg(temp.path())
            .env_remove("NEITH_LIBS")
            .args(["json", "query", text])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };

    assert_eq!(query("originaluniqueterm").as_array().unwrap().len(), 1);
    std::fs::write(&note, "# Note\n\nupdateduniqueterm\n").unwrap();
    let results = query("updateduniqueterm");
    assert_eq!(results.as_array().unwrap().len(), 1);
    assert!(
        results[0]["snippet"]
            .as_str()
            .unwrap()
            .contains("updateduniqueterm")
    );
}
