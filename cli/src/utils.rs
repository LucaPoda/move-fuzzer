use std::{env, fs, io::Read, path::{Path, PathBuf}, process::Command};

use anyhow::{bail, Context, Result};

use crate::project::{FuzzProject, DEFAULT_FUZZ_DIR};

/// The default target to pass to cargo, to workaround issue #11.
pub fn default_target() -> &'static str {
    current_platform::CURRENT_PLATFORM
}

/// Returns the path for the first found non-fuzz Cargo package
pub fn find_package() -> Result<PathBuf> {
    let mut dir = env::current_dir()?;
    let mut data = Vec::new();
    loop {
        let manifest_path = dir.join("Cargo.toml");
        match fs::File::open(&manifest_path) {
            Err(_) => {}
            Ok(mut f) => {
                data.clear();
                f.read_to_end(&mut data)
                    .with_context(|| format!("failed to read {}", manifest_path.display()))?;
                let value: toml::Value = toml::from_slice(&data).with_context(|| {
                    format!(
                        "could not decode the manifest file at {}",
                        manifest_path.display()
                    )
                })?;
                if !is_fuzz_manifest(&value) {
                    // Not a cargo-fuzz project => must be a proper cargo project :)
                    return Ok(dir);
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }
    bail!("could not find a cargo project")
}

pub fn is_fuzz_manifest(value: &toml::Value) -> bool {
    let is_fuzz = value
            .as_table()
            .and_then(|v| v.get("package"))
            .and_then(toml::Value::as_table)
            .and_then(|v| v.get("name"))
            .and_then(toml::Value::as_str)
            .map(|name| name.ends_with("fuzz"));
    is_fuzz == Some(true)
}

// If `fuzz_dir_opt` is `None`, returns a new instance with the default fuzz project
// path. Otherwise, returns a new instance with the inner content of `fuzz_dir_opt`.
pub fn manage_initial_instance(fuzz_dir_opt: Option<PathBuf>) -> Result<FuzzProject> {
    let project_dir = find_package()?;
    let fuzz_dir = if let Some(el) = fuzz_dir_opt {
        el
    } else {
        project_dir.join(DEFAULT_FUZZ_DIR)
    };
    Ok(FuzzProject {
        fuzz_dir,
        targets: Vec::new(),
    })
}

pub fn collect_targets(value: &toml::Value) -> Vec<String> {
    let bins = value
        .as_table()
        .and_then(|v| v.get("bin"))
        .and_then(toml::Value::as_array);
    let mut bins = if let Some(bins) = bins {
        bins.iter()
            .map(|bin| {
                bin.as_table()
                    .and_then(|v| v.get("name"))
                    .and_then(toml::Value::as_str)
            })
            .filter_map(|name| name.map(String::from))
            .collect()
    } else {
        Vec::new()
    };
    // Always sort them, so that we have deterministic output.
    bins.sort();
    bins
}


pub fn strip_current_dir_prefix(path: &Path) -> &Path {
    env::current_dir()
        .ok()
        .and_then(|curdir| path.strip_prefix(curdir).ok())
        .unwrap_or(path)
}

pub fn sysroot() -> Result<String> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc).arg("--print").arg("sysroot").output()?;
    // Note: We must trim() to remove the `\n` from the end of stdout
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

pub fn rustlib() -> Result<PathBuf> {
    let sysroot = sysroot()?;
    let mut pathbuf = PathBuf::from(sysroot);
    pathbuf.push("lib");
    pathbuf.push("rustlib");
    pathbuf.push(rustc_version::version_meta()?.host);
    pathbuf.push("bin");
    Ok(pathbuf)
}