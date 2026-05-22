//! Top-level CLI dispatch using clap.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::{diff::diff_properties, git, installer, merge, schema, sidecar};

#[derive(Parser, Debug)]
#[command(name = "unreal-merge", about = "Resolve UE binary merge conflicts")]
pub struct Cli {
    /// Git-driver mode: invoked positionally by Git's merge driver dispatch.
    /// When set, all four following positional arguments must be present.
    #[arg(long = "git-driver", num_args = 4, value_names = ["ANCESTOR", "OURS", "THEIRS", "PATH"])]
    pub git_driver: Option<Vec<String>>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Install the merge driver into the current Git repo.
    Install {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Remove the merge driver from the current Git repo.
    Uninstall {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// List conflicted .uasset/.umap files in the current repo.
    Scan {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Export one .uasset to JSON via the commandlet (debug helper).
    Export {
        /// Path to the .uasset to export.
        path: PathBuf,
        /// Override sidecar executable (defaults to UnrealEditor.exe lookup).
        #[arg(long)]
        sidecar: Option<PathBuf>,
        /// Override host project (defaults to ue-host/HostProject.uproject relative to cwd).
        #[arg(long)]
        host_project: Option<PathBuf>,
    },
    /// Compare two .uasset files via the commandlet and print property diffs.
    Diff {
        ours: PathBuf,
        theirs: PathBuf,
        #[arg(long)]
        sidecar: Option<PathBuf>,
        #[arg(long)]
        host_project: Option<PathBuf>,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    if let Some(args) = cli.git_driver {
        return run_git_driver(&args);
    }

    match cli.command {
        Some(Command::Install { repo }) => {
            let exe = std::env::current_exe().context("current_exe")?;
            installer::install(&repo, &exe)?;
            println!("Installed unreal-merge driver in {}", repo.display());
            Ok(())
        }
        Some(Command::Uninstall { repo }) => {
            installer::uninstall(&repo)?;
            println!("Uninstalled unreal-merge driver from {}", repo.display());
            Ok(())
        }
        Some(Command::Scan { repo }) => {
            let conflicts = git::list_conflicts(&repo)?;
            if conflicts.is_empty() {
                println!("No conflicts.");
            } else {
                for c in conflicts {
                    println!("{}", c);
                }
            }
            Ok(())
        }
        Some(Command::Export {
            path,
            sidecar,
            host_project,
        }) => run_export(&path, sidecar.as_deref(), host_project.as_deref()),
        Some(Command::Diff {
            ours,
            theirs,
            sidecar,
            host_project,
        }) => run_diff(&ours, &theirs, sidecar.as_deref(), host_project.as_deref()),
        None => {
            // No subcommand and no --git-driver: print help and exit 2.
            <Cli as clap::CommandFactory>::command().print_help()?;
            println!();
            std::process::exit(2);
        }
    }
}

fn default_sidecar() -> PathBuf {
    PathBuf::from(r"C:\Program Files\Epic Games\UE_5.6\Engine\Binaries\Win64\UnrealEditor.exe")
}

fn default_host_project() -> PathBuf {
    PathBuf::from("ue-host/HostProject.uproject")
}

fn build_sidecar(
    executable_override: Option<&std::path::Path>,
    host_project_override: Option<&std::path::Path>,
) -> sidecar::Sidecar {
    let executable = executable_override
        .map(PathBuf::from)
        .unwrap_or_else(default_sidecar);
    let host_project = host_project_override
        .map(PathBuf::from)
        .unwrap_or_else(default_host_project);
    // Mock sidecar takes no args; UE needs the project + commandlet flags.
    let args = if executable.to_string_lossy().to_lowercase().contains("unrealeditor") {
        vec![
            host_project.display().to_string(),
            "-run=MergeBinariesExport".to_string(),
            "-stdio".to_string(),
            "-nullrhi".to_string(),
            "-unattended".to_string(),
            "-NoCrashReports".to_string(),
        ]
    } else {
        Vec::new()
    };
    let log_redirect = if executable
        .to_string_lossy()
        .to_lowercase()
        .contains("unrealeditor")
    {
        Some(std::env::temp_dir().join(format!(
            "unreal-merge-{}.log",
            std::process::id()
        )))
    } else {
        None
    };
    sidecar::Sidecar::new(sidecar::SidecarConfig {
        executable,
        args,
        prepend_warmup: true,
        log_redirect,
    })
}

fn export_via_sidecar(
    sidecar: &sidecar::Sidecar,
    path: &std::path::Path,
) -> Result<schema::AssetSnapshot> {
    let abs = std::fs::canonicalize(path).with_context(|| format!("canonicalise {}", path.display()))?;
    let path_str = abs.to_string_lossy().replace('\\', "/");
    let requests = vec![serde_json::json!({"id": 1, "cmd": "export", "path": path_str})];
    let responses = sidecar.run_batch(&requests)?;
    let response = responses
        .into_iter()
        .find(|r| r.get("id").and_then(|i| i.as_u64()) == Some(1))
        .context("no id=1 response from sidecar")?;
    let snap: schema::AssetSnapshot = serde_json::from_value(response)
        .context("parse AssetSnapshot")?;
    if !snap.ok {
        anyhow::bail!("commandlet reported ok=false");
    }
    Ok(snap)
}

fn run_export(
    path: &std::path::Path,
    sidecar_override: Option<&std::path::Path>,
    host_project_override: Option<&std::path::Path>,
) -> Result<()> {
    let s = build_sidecar(sidecar_override, host_project_override);
    let snap = export_via_sidecar(&s, path)?;
    println!("{}", serde_json::to_string_pretty(&snap)?);
    Ok(())
}

fn run_diff(
    ours: &std::path::Path,
    theirs: &std::path::Path,
    sidecar_override: Option<&std::path::Path>,
    host_project_override: Option<&std::path::Path>,
) -> Result<()> {
    let s = build_sidecar(sidecar_override, host_project_override);
    let snap_ours = export_via_sidecar(&s, ours)?;
    let snap_theirs = export_via_sidecar(&s, theirs)?;
    let diffs = diff_properties(&snap_ours.asset.properties, &snap_theirs.asset.properties);
    println!("ours saved_hash:   {}", snap_ours.package.saved_hash);
    println!("theirs saved_hash: {}", snap_theirs.package.saved_hash);
    if diffs.is_empty() {
        println!("No property-level diffs (hashes still differ - see Plan 1 done report).");
    } else {
        println!("Property diffs:");
        for d in diffs {
            println!("  {:?}", d);
        }
    }
    Ok(())
}

fn run_git_driver(args: &[String]) -> Result<()> {
    let [ancestor, ours, theirs, path] = match args {
        [a, b, c, d] => [a.clone(), b.clone(), c.clone(), d.clone()],
        _ => anyhow::bail!("--git-driver needs exactly 4 positional args"),
    };
    eprintln!("unreal-merge --git-driver:");
    eprintln!("  ancestor: {}", ancestor);
    eprintln!("  ours:     {}", ours);
    eprintln!("  theirs:   {}", theirs);
    eprintln!("  path:     {}", path);

    let resolution: merge::Resolution = std::env::var("UNREAL_MERGE_RESOLUTION")
        .unwrap_or_else(|_| "abort".to_string())
        .parse()?;
    eprintln!("  resolution from env: {:?}", resolution);

    let dest = std::path::PathBuf::from(&ours);
    match merge::apply_resolution(
        resolution,
        std::path::Path::new(&ours),
        std::path::Path::new(&theirs),
        &dest,
    ) {
        Ok(()) => {
            eprintln!("Resolution applied; exiting 0 (Git marks file resolved).");
            Ok(())
        }
        Err(e) => {
            eprintln!("Aborted ({}); exiting 1 (Git leaves conflict).", e);
            std::process::exit(1);
        }
    }
}
