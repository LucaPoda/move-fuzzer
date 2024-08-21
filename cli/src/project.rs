


use crate::utils::{collect_targets, manage_initial_instance};
use crate::{Target};
use anyhow::{Context, Result};


use std::collections::HashSet;
use std::ffi::{self, OsStr};
use std::fs::create_dir_all;
use std::io::Read;

use std::path::{Path, PathBuf};
use std::{
    fs,
    process::{Command},
    time,
};

pub(crate) const DEFAULT_FUZZ_DIR: &str = "fuzz";

pub struct FuzzProject {
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
        // if !is_fuzz_manifest(&manifest) {
        //     bail!(
        //         "manifest `{}` does not look like a move-fuzz manifest. \
        //          The package name should end with \"fuzz\"",
        //         project.get_manifest_path().display()
        //     );
        // }
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

    pub(crate) fn get_target_dir(&self, path: &Option<PathBuf>) -> Option<PathBuf> {
        // Use the user-provided target directory, if provided.
        if let Some(target_dir) = path.clone() {
            return Some(target_dir);
        } 
        else {
            None
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

    pub(crate) fn get_run_fuzzer_command(&self, target: &Target, coverage_dir: Option<&PathBuf>, args: Vec<Box<dyn AsRef<OsStr>>>) -> Result<Command> {
        let module_path = target.get_module_path(&self.fuzz_dir).expect("Module path not found");

        let mut module_path_arg = ffi::OsString::from("--module-path=");    
        module_path_arg.push(module_path);
        
        let mut target_module_arg = ffi::OsString::from("--target-module=");    
        target_module_arg.push(target.get_target_module());
        
        let mut target_function_arg = ffi::OsString::from("--target-function=");    
        target_function_arg.push(target.get_target_function());
        
        let mut artifact_arg = ffi::OsString::from("-artifact_prefix=");
        artifact_arg.push(self.artifacts_for(target)?);
        
        let mut runs_arg = ffi::OsString::from("-runs=");
        runs_arg.push("100000");
        

        let mut cmd = Command::new("move-fuzzer-worker");
        
        cmd.arg(module_path_arg)
            .arg(target_module_arg)
            .arg(target_function_arg);

        if let Some(coverage_dir) = coverage_dir {
            create_dir_all(coverage_dir)?;
        
            cmd.arg("--coverage");
            cmd.arg("--coverage-map-dir").arg(coverage_dir);
        }

        for arg in args {
            cmd.arg(arg.as_ref());
        }

        cmd.arg(artifact_arg);
        cmd.arg(runs_arg);

        Ok(cmd)
    }

    /// Returns paths to the `coverage/<target>/raw` directory and `coverage/<target>/coverage.profdata` file.
    pub(crate) fn coverage_for(&self, target: &Target) -> Result<(PathBuf, PathBuf, PathBuf)> {
        println!("fesu");
        let mut coverage_data = self.get_fuzz_dir().to_owned();
        coverage_data.push("coverage");
        coverage_data.push(target.get_target_module());
        coverage_data.push(target.get_target_function());

        let mut coverage_raw = coverage_data.clone();
        let mut coverage_map = coverage_data.clone();
        coverage_data.push("coverage.profdata");
        coverage_raw.push("raw");
        coverage_map.push("map");

        fs::create_dir_all(&coverage_raw).with_context(|| {
            format!("could not make a coverage directory at {:?}", coverage_raw)
        })?;

        println!("Data: {:?}", coverage_data);
        println!("Raw: {:?}", coverage_raw);
        println!("Map: {:?}", coverage_map);

        Ok((coverage_raw, coverage_data, coverage_map))
    }

    pub(crate) fn corpus_for(&self, target: &Target) -> Result<PathBuf> {
        let mut p = self.get_fuzz_dir().to_owned();
        p.push("corpus");
        p.push(target.get_target_module());
        p.push(target.get_target_function());
        fs::create_dir_all(&p)
            .with_context(|| format!("could not make a corpus directory at {:?}", p))?;
        Ok(p)
    }

    pub(crate) fn artifacts_for(&self, target: &Target) -> Result<PathBuf> {
        let mut p = self.get_fuzz_dir().to_owned();
        p.push("artifacts");
        p.push(target.get_target_module());
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