use anyhow::{bail, Result};
use console::style;
use std::path::{Path, PathBuf};

fn is_home_directory(path: &Path) -> bool {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Ok(home) = std::env::var("HOME")
        && let Ok(home_path) = Path::new(&home).canonicalize()
        && canonical == home_path
    {
        return true;
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE")
        && let Ok(user_path) = Path::new(&userprofile).canonicalize()
        && canonical == user_path
    {
        return true;
    }
    false
}

fn find_ide_binary() -> Option<PathBuf> {
    let bin_name = if cfg!(windows) {
        "uneven-ui.exe"
    } else {
        "uneven-ui"
    };

    // 1. Check relative to current CLI executable
    if let Ok(current_exe) = std::env::current_exe()
        && let Some(parent) = current_exe.parent()
    {
        let candidate = parent.join(bin_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // 2. Check PATH environment variable
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // 3. Check standard OS installation directories
    let mut search_paths: Vec<PathBuf> = Vec::new();

    if let Ok(home) = std::env::var("HOME") {
        search_paths.push(PathBuf::from(&home).join(".uneven/bin").join(bin_name));
        search_paths.push(PathBuf::from("/Applications/Uneven IDE.app/Contents/MacOS/uneven-ui"));
        search_paths.push(PathBuf::from("/Applications/Uneven.app/Contents/MacOS/uneven-ui"));
    }

    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        search_paths.push(
            PathBuf::from(&userprofile)
                .join(r"AppData\Local\Programs\Uneven IDE\uneven-ui.exe"),
        );
        search_paths.push(PathBuf::from(r"C:\Program Files\Uneven IDE\uneven-ui.exe"));
    }

    search_paths.push(PathBuf::from(format!("/usr/bin/{}", bin_name)));
    search_paths.push(PathBuf::from(format!("/usr/local/bin/{}", bin_name)));
    search_paths.push(PathBuf::from(format!("/opt/uneven-ui/{}", bin_name)));

    // 4. Check Cargo workspace build artifacts (for local development)
    if let Ok(cwd) = std::env::current_dir() {
        search_paths.push(cwd.join(format!("target/release/{}", bin_name)));
        search_paths.push(cwd.join(format!("target/debug/{}", bin_name)));
    }

    search_paths.into_iter().find(|path| path.is_file())
}

pub async fn run(path_arg: Option<&str>) -> Result<()> {
    // 1. Resolve project path if requested
    let target_project = match path_arg {
        Some("~") => None,
        Some(raw) => {
            let p = PathBuf::from(raw);
            let resolved = p.canonicalize().unwrap_or(p);
            if is_home_directory(&resolved) {
                None
            } else if resolved.is_dir() {
                Some(resolved)
            } else {
                None
            }
        }
        None => {
            // No argument: check current directory
            if let Ok(cwd) = std::env::current_dir() {
                let resolved = cwd.canonicalize().unwrap_or(cwd);
                if is_home_directory(&resolved) {
                    None
                } else {
                    Some(resolved)
                }
            } else {
                None
            }
        }
    };

    // 2. Locate Uneven IDE binary
    let ide_binary = match find_ide_binary() {
        Some(bin) => bin,
        None => {
            println!(
                "{}",
                style("❌ Uneven IDE executable ('uneven-ui') was not found on your system.").red().bold()
            );
            println!(
                "{}",
                style("   Please install the Uneven IDE package (.deb / .dmg / .exe) from https://uneven.dev").dim()
            );
            println!(
                "{}",
                style("   Or compile it locally from source using: cargo build --release -p uneven-ui").dim()
            );
            bail!("Uneven IDE binary not found");
        }
    };

    // 3. Display launch notification
    match &target_project {
        Some(proj) => {
            println!(
                "{} {}",
                style("🚀 Launching Uneven IDE in workspace:").bold().cyan(),
                style(proj.display()).bold().white()
            );
        }
        None => {
            println!(
                "{}",
                style("🚀 Launching Uneven IDE (welcome mode)...").bold().cyan()
            );
        }
    }

    // 4. Spawn IDE detached so the CLI returns control immediately
    let mut cmd = std::process::Command::new(ide_binary);
    if let Some(ref proj) = target_project {
        cmd.arg(proj);
    }

    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    cmd.spawn()?;

    Ok(())
}
