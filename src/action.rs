use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

use anyhow::{Context, Result};

pub fn copy_text(text: &str) -> Result<()> {
    let mut copied = false;
    if command_exists("xsel") {
        let mut child = Command::new("xsel")
            .arg("--clipboard")
            .arg("--input")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .context("failed to start xsel")?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(text.as_bytes())?;
        }
        if child.wait()?.success() {
            copied = true;
        }
    }
    if std::env::var_os("TMUX").is_some() && command_exists("tmux") {
        let status = Command::new("tmux")
            .arg("set-buffer")
            .arg("--")
            .arg(text)
            .status()
            .context("failed to set tmux buffer")?;
        copied |= status.success();
    }
    if copied {
        Ok(())
    } else {
        anyhow::bail!("no clipboard backend succeeded")
    }
}

pub fn open_editor(editor: &str, path: &Path, line: Option<usize>) -> io::Result<ExitStatus> {
    let mut parts = editor.split_whitespace();
    let command = parts.next().unwrap_or("vi");
    let mut cmd = Command::new(command);
    for arg in parts {
        cmd.arg(arg);
    }
    if let Some(line) = line.filter(|line| *line > 0) {
        cmd.arg(format!("+{line}"));
    }
    cmd.arg(path);
    cmd.status()
}

pub fn command_exists(command: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
}
