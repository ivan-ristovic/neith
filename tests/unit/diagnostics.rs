use super::*;
use crate::config::{AppConfig, RuntimeConfig};
use crate::indexer::ensure_indexes;
use crate::library::Library;

#[test]
fn renders_index_table_without_color() {
    let library = Library::new(
        PathBuf::from("/tmp/neith-lib"),
        Some("lib".to_string()),
        None,
    );
    let stats = IndexStats {
        indexed: 3,
        removed: 0,
        unchanged: 7,
    };

    assert_eq!(
        render_index_table([(&library, &stats)], false),
        "alias  indexed  removed  unchanged\nlib    3        0        7"
    );
}

#[test]
fn reports_missing_index_status() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("note.md"), "# Test\n").unwrap();
    let library = Library::new(temp.path().to_path_buf(), Some("test".to_string()), None);

    let rows = collect_status_rows(&[library]).unwrap();

    assert_eq!(rows[0].files, 1);
    assert_eq!(rows[0].indexed, 0);
    assert_eq!(rows[0].stale, 1);
    assert_eq!(rows[0].cache, "missing");
    assert_eq!(rows[0].index, "missing");
}

#[test]
fn reports_current_index_status() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("note.md"), "# Test\n").unwrap();
    let library = Library::new(temp.path().to_path_buf(), Some("test".to_string()), None);
    ensure_indexes(std::slice::from_ref(&library), false, |_library, _stage| {}).unwrap();

    let rows = collect_status_rows(&[library]).unwrap();

    assert_eq!(rows[0].files, 1);
    assert_eq!(rows[0].indexed, 1);
    assert_eq!(rows[0].stale, 0);
    assert_eq!(rows[0].index, "ok");
}

#[test]
fn health_report_exit_code_classifies_levels() {
    assert_eq!(HealthReport { checks: vec![] }.exit_code(), 0);
    assert_eq!(
        HealthReport {
            checks: vec![warning("x", "y")]
        }
        .exit_code(),
        2
    );
    assert_eq!(
        HealthReport {
            checks: vec![failure("x", "y")]
        }
        .exit_code(),
        1
    );
}

#[test]
fn tool_checks_report_missing_custom_clipboard_command() {
    let mut runtime = RuntimeConfig {
        path: None,
        app: AppConfig::default(),
        libraries: Vec::new(),
        dropped_libraries: Vec::new(),
    };
    runtime.app.editor.command = "true".to_string();
    runtime.app.clipboard.command = "definitely-missing-neith-copy --flag".to_string();

    let checks = tool_checks(&runtime);
    let clipboard = checks
        .iter()
        .find(|check| check.name == "clipboard")
        .unwrap();

    assert_eq!(clipboard.level, CheckLevel::Warning);
    assert_eq!(
        clipboard.detail,
        "custom command not found in PATH: definitely-missing-neith-copy"
    );
}

#[test]
fn healthcheck_reports_missing_configured_libraries() {
    let temp = tempfile::tempdir().unwrap();
    let valid = temp.path().join("valid");
    std::fs::create_dir(&valid).unwrap();
    let missing = temp.path().join("missing");
    let config = temp.path().join("config.toml");
    std::fs::write(
        &config,
        format!(
            "[editor]\ncommand = \"true\"\n\n[[libraries]]\npath = \"{}\"\nalias = \"valid\"\n\n[[libraries]]\npath = \"{}\"\nalias = \"missing\"\n",
            valid.display(),
            missing.display()
        ),
    )
    .unwrap();

    let report = healthcheck(Some(config), None);

    assert!(report.checks.iter().any(|check| {
        check.level == CheckLevel::Failure
            && check.name == "library missing"
            && check.detail.contains("missing or not a directory")
    }));
}
