//! Apply a Resolution to the working-tree file. Handles read-only LFS-locked
//! files per spec §8 case 8a.

use anyhow::{Result, bail};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Ours,
    Theirs,
    Abort,
}

impl std::str::FromStr for Resolution {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ours" => Ok(Self::Ours),
            "theirs" => Ok(Self::Theirs),
            "abort" => Ok(Self::Abort),
            other => bail!("unknown resolution {:?}; expected ours|theirs|abort", other),
        }
    }
}

/// Copy `ours` or `theirs` over `dest`. Returns Err on Abort.
pub fn apply_resolution(res: Resolution, ours: &Path, theirs: &Path, dest: &Path) -> Result<()> {
    let source = match res {
        Resolution::Ours => ours,
        Resolution::Theirs => theirs,
        Resolution::Abort => bail!("aborted by user; conflict left in place"),
    };
    apply_merged_file(source, dest)
}

/// Copy `source` over `dest`, preserving the read-only bit if `dest` had it
/// set (e.g. for LFS-locked files — spec §8 case 8a).
pub fn apply_merged_file(source: &Path, dest: &Path) -> Result<()> {
    let dest_meta = std::fs::metadata(dest).ok();
    let was_readonly = dest_meta
        .as_ref()
        .map(|m| m.permissions().readonly())
        .unwrap_or(false);
    if was_readonly {
        let mut perms = dest_meta.unwrap().permissions();
        perms.set_readonly(false);
        std::fs::set_permissions(dest, perms)?;
    }

    std::fs::copy(source, dest)?;

    if was_readonly {
        let mut perms = std::fs::metadata(dest)?.permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(dest, perms)?;
    }
    Ok(())
}
