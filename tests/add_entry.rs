#[cfg(unix)]
#[test]
fn cli_add_discards_unsaved_draft_and_keeps_saved_note() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let temp = tempfile::tempdir().unwrap();
    let library = temp.path().join("library");
    fs::create_dir(&library).unwrap();
    let editor = temp.path().join("editor.sh");
    let config = temp.path().join("config.toml");
    fs::write(
        &config,
        format!(
            "[[libraries]]\npath = {:?}\nalias = \"notes\"\n\n[editor]\ncommand = {:?}\n",
            library.display().to_string(),
            editor.display().to_string(),
        ),
    )
    .unwrap();

    fs::write(&editor, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o755)).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_neith"))
            .args(["--config", config.to_str().unwrap(), "add", "New", "idea"])
            .output()
            .unwrap()
    };
    let output = run();
    assert!(output.status.success());
    assert!(!library.join("new").exists());

    let empty = Command::new(env!("CARGO_BIN_EXE_neith"))
        .args(["--config", config.to_str().unwrap(), "add"])
        .output()
        .unwrap();
    assert!(!empty.status.success());
    assert!(!library.join("notes/untitled.md").exists());

    fs::write(&editor, "#!/bin/sh\nprintf '# Saved\\n' > \"$1\"\n").unwrap();
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(library.join("new/idea.md")).unwrap(),
        "# Saved\n"
    );

    fs::create_dir(library.join("shell tools")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_neith"))
        .args([
            "--config",
            config.to_str().unwrap(),
            "add",
            "shell tools",
            "Safe",
            "aliases",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(library.join("shell tools/safe-aliases.md").is_file());
}

#[cfg(unix)]
#[test]
fn cli_folder_completion_and_fzf_pick_use_current_directories() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    let temp = tempfile::tempdir().unwrap();
    let library = temp.path().join("library");
    fs::create_dir_all(library.join("awk/subtopic")).unwrap();
    fs::create_dir(library.join("bash")).unwrap();
    let config = temp.path().join("config.toml");
    let editor = temp.path().join("editor.sh");
    let fake_fzf = temp.path().join("fzf");
    fs::write(&editor, "#!/bin/sh\nprintf '# Saved\\n' > \"$1\"\n").unwrap();
    fs::write(
        &fake_fzf,
        "#!/bin/sh\ncat >/dev/null\nprintf 'awk/subtopic\\n'\n",
    )
    .unwrap();
    for path in [&editor, &fake_fzf] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(
        &config,
        format!(
            "[[libraries]]\npath = {:?}\nalias = \"notes\"\n\n[editor]\ncommand = {:?}\n",
            library.display().to_string(),
            editor.display().to_string()
        ),
    )
    .unwrap();

    let binary = env!("CARGO_BIN_EXE_neith");
    let output = Command::new(binary)
        .args([
            "--config",
            config.to_str().unwrap(),
            "__complete-folders",
            "awk",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "awk\nawk/subtopic\n"
    );

    let completions = Command::new(binary)
        .args(["completions", "bash"])
        .output()
        .unwrap();
    assert!(completions.status.success());
    let completion_file = temp.path().join("neith-completion.bash");
    fs::write(&completion_file, completions.stdout).unwrap();
    let shell = Command::new("bash")
        .args([
            "-c",
            "source \"$1\"; COMP_WORDS=(\"$2\" --config \"$3\" add aw); COMP_CWORD=4; _neith \"$2\" aw add; printf '%s\\n' \"${COMPREPLY[@]}\"",
            "bash",
            completion_file.to_str().unwrap(),
            binary,
            config.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        shell.status.success(),
        "{}",
        String::from_utf8_lossy(&shell.stderr)
    );
    assert_eq!(
        String::from_utf8(shell.stdout).unwrap(),
        "awk\nawk/subtopic\n"
    );

    let output = Command::new(binary)
        .args([
            "--config",
            config.to_str().unwrap(),
            "add",
            "--pick-folder",
            "Safe",
            "aliases",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                temp.path().display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(library.join("awk/subtopic/safe-aliases.md")).unwrap(),
        "# Saved\n"
    );

    fs::write(&fake_fzf, "#!/bin/sh\nexit 1\n").unwrap();
    let output = Command::new(binary)
        .args([
            "--config",
            config.to_str().unwrap(),
            "add",
            "--pick-folder",
            "Cancelled",
            "idea",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                temp.path().display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!library.join("awk/subtopic/cancelled-idea.md").exists());
}
