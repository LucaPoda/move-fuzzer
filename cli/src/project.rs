use crate::options::{self, BuildOptions};
use crate::run::run_fuzz_target_debug_formatter;
use crate::templates::create_target_template;
use crate::utils::{collect_targets, default_target, is_fuzz_manifest, manage_initial_instance};
use crate::{Build, Target};
use anyhow::{anyhow, bail, Context, Result};
use cargo_metadata::MetadataCommand;
use move_package::BuildConfig;
use std::collections::HashSet;
use std::ffi;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{
    env, fs,
    process::{Command, Stdio},
    time,
};

pub(crate) const DEFAULT_FUZZ_DIR: &str = "fuzz";

pub(crate) struct FuzzProject {
    /// The project with fuzz targets
    pub(crate) fuzz_dir: PathBuf,
    pub(crate) targets: Vec<String>,
}

impl FuzzProject {
    /// Creates a new instance.
    //
    /// Find an existing `cargo fuzz` project by starting at the current
    /// directory and walking up the filesystem.
    ///
    /// If `fuzz_dir_opt` is `None`, returns a new instance with the default fuzz project
    /// path.
    pub(crate) fn new(fuzz_dir_opt: Option<PathBuf>) -> Result<Self> {
        let mut project = manage_initial_instance(fuzz_dir_opt)?;
        let manifest = project.manifest()?;
        if !is_fuzz_manifest(&manifest) {
            bail!(
                "manifest `{}` does not look like a move-fuzz manifest. \
                 The package name should end with \"fuzz\"",
                project.get_manifest_path().display()
            );
        }
        project.targets = collect_targets(&manifest);
        Ok(project)
    }

    pub(crate) fn get_fuzz_dir(&self) -> &Path {
        &self.fuzz_dir
    }

    pub(crate) fn fuzz_dir_is_default_path(&self) -> bool {
        self.fuzz_dir.ends_with(DEFAULT_FUZZ_DIR)
    }

    pub(crate) fn get_manifest_path(&self) -> PathBuf {
        self.get_fuzz_dir().join("Move.toml")
    }

    pub(crate) fn list_targets(&self) -> Result<()> {
        for bin in &self.targets {
            println!("{}", bin);
        }
        Ok(())
    }

    pub(crate) fn get_targets_dir(&self) -> PathBuf {
        let mut root = self.get_fuzz_dir().to_owned();
        root.push(crate::MOVE_TARGETS_DIR);
        root
    }
    
    pub(crate) fn get_target_path(&self, target: &str) -> PathBuf {
        let mut root = self.get_targets_dir();
        root.push(target);
        root.set_extension("move");
        root
    }

    // note: never returns Ok(None) if build.coverage is true
    pub(crate) fn get_target_dir(&self, path: &Option<PathBuf>, coverage: bool) -> Result<Option<PathBuf>> {
        // Use the user-provided target directory, if provided. Otherwise if building for coverage,
        // use the coverage directory
        if let Some(target_dir) = path.clone() {
            return Ok(Some(target_dir));
        } else if coverage {
            // To ensure that fuzzing and coverage-output generation can run in parallel, we
            // produce a separate binary for the coverage command.
            let current_dir = env::current_dir()?;
            Ok(Some(
                current_dir
                    .join("target")
                    .join(default_target())
                    .join("coverage"),
            ))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn get_artifacts_since(&self, target: &Target, since: &time::SystemTime) -> Result<HashSet<PathBuf>> {
        let mut artifacts = HashSet::new();

        let artifacts_dir = self.artifacts_for(target)?;

        for entry in fs::read_dir(&artifacts_dir).with_context(|| {
            format!(
                "failed to read directory entries of {}",
                artifacts_dir.display()
            )
        })? {
            let entry = entry.with_context(|| {
                format!(
                    "failed to read directory entry inside {}",
                    artifacts_dir.display()
                )
            })?;

            let metadata = entry
                .metadata()
                .context("failed to read artifact metadata")?;
            let modified = metadata
                .modified()
                .context("failed to get artifact modification time")?;
            if !metadata.is_file() || modified <= *since {
                continue;
            }

            artifacts.insert(entry.path());
        }

        Ok(artifacts)
    }

    pub(crate) fn get_run_fuzzer_command(&self, target: &Target) -> Result<Command> {
        let mut module_path = self.fuzz_dir.clone();
        module_path.push("build");
        module_path.push("fuzz");
        module_path.push("bytecode_modules");
        module_path.push(format!("{}.mv", target.get_module_name()));

        let mut cmd = Command::new("move-fuzzer-worker");

        let mut module_path_arg = ffi::OsString::from("--module-path=");    
        module_path_arg.push(module_path);

        let mut target_module_arg = ffi::OsString::from("--target-module=");    
        target_module_arg.push(target.get_module_name());

        let mut target_function_arg = ffi::OsString::from("--target-function=");    
        target_function_arg.push(target.get_target_function());

        let mut artifact_arg = ffi::OsString::from("-artifact_prefix=");
        artifact_arg.push(self.artifacts_for(target)?);
        
        cmd.arg(module_path_arg)
            .arg(target_module_arg)
            .arg(target_function_arg)
            .arg(artifact_arg);

        Ok(cmd)
    }

    /// Returns paths to the `coverage/<target>/raw` directory and `coverage/<target>/coverage.profdata` file.
    pub(crate) fn coverage_for(&self, target: &Target) -> Result<(PathBuf, PathBuf)> {
        let mut coverage_data = self.get_fuzz_dir().to_owned();
        coverage_data.push("coverage");
        coverage_data.push(target.get_module_name());
        coverage_data.push(target.get_target_function());

        let mut coverage_raw = coverage_data.clone();
        coverage_data.push("coverage.profdata");
        coverage_raw.push("raw");

        fs::create_dir_all(&coverage_raw).with_context(|| {
            format!("could not make a coverage directory at {:?}", coverage_raw)
        })?;
        Ok((coverage_raw, coverage_data))
    }

    pub(crate) fn corpus_for(&self, target: &Target) -> Result<PathBuf> {
        let mut p = self.get_fuzz_dir().to_owned();
        p.push("corpus");
        p.push(target.get_module_name());
        p.push(target.get_target_function());
        fs::create_dir_all(&p)
            .with_context(|| format!("could not make a corpus directory at {:?}", p))?;
        Ok(p)
    }

    pub(crate) fn artifacts_for(&self, target: &Target) -> Result<PathBuf> {
        let mut p = self.get_fuzz_dir().to_owned();
        p.push("artifacts");
        p.push(target.get_module_name());
        p.push(target.get_target_function());

        // This adds a trailing slash, which is necessary for libFuzzer, because
        // it does simple string concatenation when joining paths.
        p.push("");

        fs::create_dir_all(&p)
            .with_context(|| format!("could not make a artifact directory at {:?}", p))?;

        Ok(p)
    }

    fn manifest(&self) -> Result<toml::Value> {
        let filename = self.get_manifest_path();
        let mut file = fs::File::open(&filename)
            .with_context(|| format!("could not read the manifest file: {}", filename.display()))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        toml::from_slice(&data).with_context(|| {
            format!(
                "could not decode the manifest file at {}",
                filename.display()
            )
        })
    }
}