use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use anyhow::{Context, Result, bail};

pub fn copy_text(text: &str, command: &str) -> Result<()> {
    if !command.trim().is_empty() {
        return copy_with_command(command, text)
            .with_context(|| format!("clipboard command failed: {command}"));
    }

    let mut failures = Vec::new();
    if command_exists("wl-copy") {
        match copy_with_stdin("wl-copy", &[], text) {
            Ok(()) => return Ok(()),
            Err(err) => failures.push(err),
        }
    }
    if command_exists("xclip") {
        match copy_with_stdin("xclip", &["-sel", "clip"], text) {
            Ok(()) => return Ok(()),
            Err(err) => failures.push(err),
        }
    }
    if command_exists("xsel") {
        match copy_with_stdin("xsel", &["--clipboard", "--input"], text) {
            Ok(()) => return Ok(()),
            Err(err) => failures.push(err),
        }
    }
    if std::env::var_os("TMUX").is_some() && command_exists("tmux") {
        match Command::new("tmux")
            .arg("set-buffer")
            .arg("--")
            .arg(text)
            .status()
            .context("failed to set tmux buffer")
        {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => failures.push(anyhow::anyhow!("tmux exited with {status}")),
            Err(err) => failures.push(err),
        }
    }

    if failures.is_empty() {
        anyhow::bail!("no clipboard backend found")
    } else {
        let details = failures
            .into_iter()
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        anyhow::bail!("no clipboard backend succeeded: {details}")
    }
}

fn copy_with_command(command: &str, text: &str) -> Result<()> {
    let parts = command.split_whitespace().collect::<Vec<_>>();
    let Some((program, args)) = parts.split_first() else {
        bail!("clipboard command is empty");
    };
    copy_with_stdin(program, args, text)
}

fn copy_with_stdin(command: &str, args: &[&str], text: &str) -> Result<()> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to start {command}"))?;
    let mut write_error = None;
    if let Some(mut stdin) = child.stdin.take()
        && let Err(err) = {
            use std::io::Write;
            stdin.write_all(text.as_bytes())
        }
    {
        write_error = Some(err);
    }
    if let Some(err) = write_error {
        let _ = child.wait();
        return Err(err).with_context(|| format!("failed to write to {command}"));
    }
    let status = child
        .wait()
        .with_context(|| format!("failed waiting for {command}"))?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("{command} exited with {status}")
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

pub fn open_editor_in_tmux_pane(editor: &str, path: &Path, line: Option<usize>) -> Result<()> {
    if std::env::var_os("TMUX").is_none() {
        bail!("not running inside tmux");
    }
    if !command_exists("tmux") {
        bail!("tmux not found in PATH");
    }

    let pane = target_tmux_pane()?;
    let cwd = tmux_pane_current_path(&pane)?;
    let split_flag = tmux_pane_split_flag(&pane)?;
    let command = editor_shell_command(editor, path, line);
    let status = Command::new("tmux")
        .arg("split-window")
        .arg(split_flag)
        .arg("-t")
        .arg(&pane)
        .arg("-c")
        .arg(&cwd)
        .arg(command)
        .status()
        .context("failed to start tmux split-window")?;
    if status.success() {
        Ok(())
    } else {
        bail!("tmux split-window exited with {status}")
    }
}

fn target_tmux_pane() -> Result<String> {
    for key in ["NEITH_TMUX_TARGET_PANE", "TMUX_PANE"] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim();
            if !value.is_empty() {
                return Ok(value.to_string());
            }
        }
    }
    bail!("tmux target pane not found")
}

fn tmux_pane_current_path(pane: &str) -> Result<String> {
    let output = Command::new("tmux")
        .arg("display-message")
        .arg("-p")
        .arg("-t")
        .arg(pane)
        .arg("#{pane_current_path}")
        .output()
        .context("failed to query tmux pane path")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            bail!("tmux display-message exited with {}", output.status);
        }
        bail!(
            "tmux display-message exited with {}: {stderr}",
            output.status
        );
    }
    let cwd = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if cwd.is_empty() {
        bail!("tmux pane path is empty");
    }
    Ok(cwd)
}

fn tmux_pane_split_flag(pane: &str) -> Result<&'static str> {
    let (width, height) = tmux_pane_size(pane)?;
    Ok(split_flag_for_size(width, height))
}

fn tmux_pane_size(pane: &str) -> Result<(usize, usize)> {
    let output = Command::new("tmux")
        .arg("display-message")
        .arg("-p")
        .arg("-t")
        .arg(pane)
        .arg("#{pane_width} #{pane_height}")
        .output()
        .context("failed to query tmux pane size")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            bail!("tmux display-message exited with {}", output.status);
        }
        bail!(
            "tmux display-message exited with {}: {stderr}",
            output.status
        );
    }
    parse_tmux_pane_size(&String::from_utf8_lossy(&output.stdout))
}

fn parse_tmux_pane_size(value: &str) -> Result<(usize, usize)> {
    let mut parts = value.split_whitespace();
    let width = parts
        .next()
        .context("tmux pane width is missing")?
        .parse::<usize>()
        .context("failed to parse tmux pane width")?;
    let height = parts
        .next()
        .context("tmux pane height is missing")?
        .parse::<usize>()
        .context("failed to parse tmux pane height")?;
    if parts.next().is_some() {
        bail!("tmux pane size has unexpected extra fields");
    }
    Ok((width, height))
}

fn split_flag_for_size(width: usize, height: usize) -> &'static str {
    if width > height { "-v" } else { "-h" }
}

fn editor_shell_command(editor: &str, path: &Path, line: Option<usize>) -> String {
    let mut parts = editor.split_whitespace();
    let mut args = Vec::new();
    args.push(shell_quote(parts.next().unwrap_or("vi")));
    args.extend(parts.map(shell_quote));
    if let Some(line) = line.filter(|line| *line > 0) {
        args.push(shell_quote(&format!("+{line}")));
    }
    args.push(shell_quote(path.to_string_lossy().as_ref()));
    format!("{}; exec \"${{SHELL:-sh}}\"", args.join(" "))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn command_exists(command: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
}

#[cfg(test)]
#[path = "../tests/unit/action.rs"]
mod tests;
