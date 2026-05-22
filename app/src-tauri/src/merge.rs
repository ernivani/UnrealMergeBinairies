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

/// Copy `ours` or `theirs` over `dest`. Returns Err on Abort (deliberately —
/// `--git-driver` mode then exits non-zero, signalling Git to leave the
/// conflict in place).
pub fn apply_resolution(res: Resolution, ours: &Path, theirs: &Path, dest: &Path) -> Result<()> {
    let source = match res {
        Resolution::Ours => ours,
        Resolution::Theirs => theirs,
        Resolution::Abort => bail!("aborted by user; conflict left in place"),
    };

    // If dest is read-only (e.g. LFS lockable), clear the bit before writing
    // and restore it after. This is spec §8 case 8a.
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
