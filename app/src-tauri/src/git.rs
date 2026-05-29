//! Thin shell-out helpers over `git`. One responsibility per function.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

/// List paths in unmerged (conflicted) state matching `*.uasset` or `*.umap`.
pub fn list_conflicts(repo: &Path) -> Result<Vec<String>> {
    let out = Command::new("git")
        .args(["ls-files", "-u", "-z"])
        .current_dir(repo)
        .output()
        .context("git ls-files -u")?;
    if !out.status.success() {
        bail!("git ls-files -u failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    // Format: each entry is `<mode> <sha> <stage>\t<path>\0`. Same path appears
    // at stages 1, 2, 3 - we dedupe.
    let text = String::from_utf8_lossy(&out.stdout);
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in text.split('\0').filter(|e| !e.is_empty()) {
        if let Some(idx) = entry.find('\t') {
            let path = &entry[idx + 1..];
            if path.ends_with(".uasset") || path.ends_with(".umap") {
                seen.insert(path.to_string());
            }
        }
    }
    Ok(seen.into_iter().collect())
}

pub struct ConflictStages {
    pub base: PathBuf,
    pub ours: PathBuf,
    pub theirs: PathBuf,
    _tmp: tempfile::TempDir,
}

/// Materialise the three stages of `path` (base=1, ours=2, theirs=3) to temp files.
/// The returned `ConflictStages` owns a `TempDir`; when it drops, the files vanish.
pub fn read_stages(repo: &Path, path: &str) -> Result<ConflictStages> {
    let tmp = tempfile::tempdir().context("create tempdir for stages")?;
    let base = stage_to_path(repo, path, 1, tmp.path(), "base")?;
    let ours = stage_to_path(repo, path, 2, tmp.path(), "ours")?;
    let theirs = stage_to_path(repo, path, 3, tmp.path(), "theirs")?;
    Ok(ConflictStages {
        base,
        ours,
        theirs,
        _tmp: tmp,
    })
}

fn stage_to_path(
    repo: &Path,
    path: &str,
    stage: u8,
    dir: &Path,
    label: &str,
) -> Result<PathBuf> {
    // Preserve the filename so UE's loader sees a reasonable extension.
    let leaf = Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "stage.bin".to_string());
    let out_path = dir.join(format!("{}_{}", label, leaf));
    let spec = format!(":{}:{}", stage, path);
    let out = Command::new("git")
        .args(["show", &spec])
        .current_dir(repo)
        .output()
        .context("git show stage")?;
    if !out.status.success() {
        bail!(
            "git show {} failed: {}",
            spec,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    std::fs::write(&out_path, &out.stdout).context("write stage temp")?;
    Ok(out_path)
}

/// Mark `path` as resolved (`git add`).
pub fn mark_resolved(repo: &Path, path: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["add", "--", path])
        .current_dir(repo)
        .status()
        .context("git add")?;
    if !status.success() {
        bail!("git add {} failed", path);
    }
    Ok(())
}
