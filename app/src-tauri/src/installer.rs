//! Install/uninstall the Git merge driver for *.uasset and *.umap conflicts.
//! Idempotent: running install twice yields the same file contents.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

const ATTR_MARK_BEGIN: &str = "# >>> unreal-merge driver (do not edit between markers) <<<";
const ATTR_MARK_END: &str = "# <<< unreal-merge driver >>>";
const ATTR_BODY: &str = "*.uasset merge=unrealbin\n*.umap   merge=unrealbin\n";

const CFG_SECTION: &str = "[merge \"unrealbin\"]";

pub fn install(repo: &Path, unreal_merge_exe: &Path) -> Result<()> {
    install_gitattributes(repo)?;
    install_git_config(repo, unreal_merge_exe)?;
    Ok(())
}

pub fn uninstall(repo: &Path) -> Result<()> {
    uninstall_gitattributes(repo)?;
    uninstall_git_config(repo)?;
    Ok(())
}

fn install_gitattributes(repo: &Path) -> Result<()> {
    let path = repo.join(".gitattributes");
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if current.contains(ATTR_MARK_BEGIN) {
        return Ok(()); // already installed
    }
    let separator = if current.is_empty() || current.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let appended = format!(
        "{}{}{}\n{}{}\n",
        current, separator, ATTR_MARK_BEGIN, ATTR_BODY, ATTR_MARK_END
    );
    std::fs::write(&path, appended).context("write .gitattributes")?;
    Ok(())
}

fn uninstall_gitattributes(repo: &Path) -> Result<()> {
    let path = repo.join(".gitattributes");
    let current = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let mut out = String::new();
    let mut skipping = false;
    for line in current.lines() {
        if line.trim_end() == ATTR_MARK_BEGIN {
            skipping = true;
            continue;
        }
        if line.trim_end() == ATTR_MARK_END {
            skipping = false;
            continue;
        }
        if skipping {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    // Trim trailing blank lines for tidiness.
    while out.ends_with("\n\n") {
        out.pop();
    }
    std::fs::write(&path, out).context("rewrite .gitattributes")?;
    Ok(())
}

fn config_path(repo: &Path) -> PathBuf {
    repo.join(".git").join("config")
}

fn install_git_config(repo: &Path, exe: &Path) -> Result<()> {
    let path = config_path(repo);
    if !path.exists() {
        bail!("not a git repository: {} missing", path.display());
    }
    let current = std::fs::read_to_string(&path)?;
    if current.contains(CFG_SECTION) {
        return Ok(()); // already installed
    }
    let exe_display = exe.display().to_string().replace('\\', "/");
    let block = format!(
        "\n{}\n\tname = Unreal binary merge\n\tdriver = \"{}\" --git-driver %O %A %B %P\n\trecursive = binary\n",
        CFG_SECTION, exe_display
    );
    let mut updated = current;
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&block);
    std::fs::write(&path, updated)?;
    Ok(())
}

fn uninstall_git_config(repo: &Path) -> Result<()> {
    let path = config_path(repo);
    let current = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let mut out = String::new();
    let mut skipping = false;
    for line in current.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("[") {
            skipping = line.contains(CFG_SECTION);
            if skipping {
                continue;
            }
        }
        if skipping {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    std::fs::write(&path, out)?;
    Ok(())
}
