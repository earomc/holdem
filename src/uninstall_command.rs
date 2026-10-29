//! `holdem uninstall`: removes a copy put in place by the release installer.

use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

use holdem::uninstall::{self, Install, Plan};

/// Asks a yes/no question on the terminal; anything but y or yes is no.
fn ask(question: &str) -> bool {
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

/// `holdem uninstall [--yes] [--purge]`
pub(crate) fn run_uninstall(yes: bool, purge: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot tell where holdem is: {e}"))?;
    let Some(install) = uninstall::find(&exe, &uninstall::receipt_files()) else {
        return Err(format!(
            "this copy of holdem was not put there by the installer (there is no receipt for \
             it), so there is nothing to uninstall: delete {} yourself",
            exe.display()
        ));
    };
    if !yes && !std::io::stdin().is_terminal() {
        return Err("nobody to ask: run `holdem uninstall --yes` to uninstall anyway".to_string());
    }
    let settings = holdem::config::user_file().and_then(|f| f.parent().map(Path::to_path_buf));
    let version = if install.version.is_empty() {
        String::new()
    } else {
        format!(" {}", install.version)
    };
    if !yes
        && !ask(&format!(
            "Remove holdem{version} from {}?",
            install.bin_dir.display()
        ))
    {
        println!("Nothing was removed.");
        return Ok(());
    }
    let has_data = settings
        .as_deref()
        .is_some_and(|d| !uninstall::data_files(d).is_empty());
    let purge = purge
        || (!yes
            && has_data
            && ask(&format!(
                "Also delete your settings and statistics in {}?",
                settings.as_deref().unwrap_or(Path::new("")).display()
            )));
    let plan = uninstall::plan(&install, settings.as_deref(), purge);
    carry_out(&install, &plan)
}

fn carry_out(install: &Install, plan: &Plan) -> Result<(), String> {
    let mut failed = Vec::new();
    for file in &plan.files {
        match std::fs::remove_file(file) {
            Ok(()) => println!("removed  {}", file.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => failed.push(format!("{}: {e}", file.display())),
        }
    }
    match remove_program(&install.exe) {
        Ok(()) => println!("removed  {}", install.exe.display()),
        Err(e) => failed.push(format!("{}: {e}", install.exe.display())),
    }
    for dir in &plan.dirs {
        // Only empty folders go: remove_dir refuses the others.
        if std::fs::remove_dir(dir).is_ok() {
            println!("removed  {}", dir.display());
        }
    }
    for file in &plan.kept {
        println!(
            "kept     {} (`holdem uninstall --purge` deletes it)",
            file.display()
        );
    }
    if let Some(dir) = &plan.path_entry {
        match remove_from_path(dir) {
            Ok(Some(note)) => println!("{note}"),
            Ok(None) => println!("removed  {} from your PATH", dir.display()),
            Err(e) => failed.push(format!("taking {} off your PATH: {e}", dir.display())),
        }
    }
    if failed.is_empty() {
        println!("holdem is uninstalled.");
        Ok(())
    } else {
        Err(format!(
            "some things could not be removed:\n  {}",
            failed.join("\n  ")
        ))
    }
}

/// Deletes the running program. Unix lets a running program be deleted.
#[cfg(not(windows))]
fn remove_program(exe: &Path) -> std::io::Result<()> {
    std::fs::remove_file(exe)
}

/// Deletes the running program. Windows does not let a running program be
/// deleted, but it may be moved: out of the way it goes, and a hidden command
/// deletes it once holdem has exited.
#[cfg(windows)]
fn remove_program(exe: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let parked =
        std::env::temp_dir().join(format!("holdem-uninstalled-{}.exe", std::process::id()));
    // Moving fails across drives; then the program waits where it is.
    let doomed = if std::fs::rename(exe, &parked).is_ok() {
        parked
    } else {
        exe.to_path_buf()
    };
    Command::new("cmd")
        .arg("/C")
        // cmd does its own quoting: pass the line as it is.
        .raw_arg(format!(
            "ping -n 3 127.0.0.1 >nul & del /f /q \"{}\"",
            doomed.display()
        ))
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

/// Takes `dir` off your PATH. On Windows that is one entry of your own Path
/// variable; elsewhere the installer added a line to your shell's startup
/// files, which is left alone and named instead (returned as a note).
#[cfg(windows)]
fn remove_from_path(dir: &Path) -> Result<Option<String>, String> {
    let dir = dir.display().to_string().replace('\'', "''");
    let script = format!(
        "$p = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         $kept = ($p -split ';') | Where-Object {{ $_ -and ($_.TrimEnd('\\') -ne '{dir}'.TrimEnd('\\')) }}; \
         [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User')"
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(None)
    } else {
        Err(format!("powershell ended with {status}"))
    }
}

#[cfg(not(windows))]
fn remove_from_path(dir: &Path) -> Result<Option<String>, String> {
    Ok(Some(format!(
        "{} is empty now: if nothing else uses it, you may remove the line \
         `. \"$HOME/.cargo/env\"` from your shell's startup files (~/.profile, ~/.bashrc, ~/.zshrc)",
        dir.display()
    )))
}
