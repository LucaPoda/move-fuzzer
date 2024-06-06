use std::{fs, io::Write};

use crate::project::FuzzProject;
use anyhow::{Context, Result};

macro_rules! move_toml_template {
    () => {
        format_args!(
            r##"[package]
name = "fuzz"
version = "0.0.0"
edition = "legacy"

[dependencies]
MoveStdlib = {{ git = "https://github.com/move-language/move-sui.git", subdir = "crates/move-stdlib", rev = "main" }}
MoveNursery = {{ git = "https://github.com/move-language/move-sui.git", subdir = "crates/move-stdlib/nursery", rev = "main" }}

[addresses]
std =  "0x1"
fuzz = "0x0"
"##
        )
    };
}

macro_rules! gitignore_template {
    () => {
        format_args!(
            r##"target
corpus
artifacts
coverage
"##
        )
    };
}

macro_rules! move_target_template {
    ($target_name:expr) => {
        format_args!(
            r##"module fuzz::{target_name} {{
    public fun fuzz_target(bytes: vector<u8>) {{
        
    }}
}}
"##,
target_name = $target_name
        )
    };
}

/// Add a new fuzz target script with a given name
pub fn create_target_template(project: &FuzzProject, target: &str) -> Result<()> {
    let move_target_path = project.get_target_path(target);

    // If the user manually created a fuzz project, but hasn't created any
    // targets yet, the `fuzz_targets` directory might not exist yet,
    // despite a `fuzz/Cargo.toml` manifest with the `metadata.cargo-fuzz`
    // key present. Make sure it does exist.
    fs::create_dir_all(project.get_targets_dir())
        .context("ensuring that `sources` directory exists failed")?;

    let mut move_script = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&move_target_path)
        .with_context(|| format!("could not create target script file at {:?}", move_target_path))?;
    move_script.write_fmt(move_target_template!(target))?;

    Ok(())
}