//! Embeds the git commit into the binary for the `tool_version` recorded with every dataset.
//!
//! `WATER_SORT_GIT_COMMIT` overrides the value (e.g. for builds outside a git checkout).

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=WATER_SORT_GIT_COMMIT");
    let git_dir = Path::new("../.git");
    for path in ["HEAD", "refs/heads", "packed-refs"] {
        let p = git_dir.join(path);
        if p.exists() {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
    let commit = std::env::var("WATER_SORT_GIT_COMMIT")
        .ok()
        .or_else(|| {
            let out = Command::new("git")
                .args(["rev-parse", "--short=12", "HEAD"])
                .output()
                .ok()?;
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        })
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=WATER_SORT_GIT_COMMIT={commit}");
}
