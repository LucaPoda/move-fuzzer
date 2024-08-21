use std::{ffi::OsStr, fs::{self}, path::{Path, PathBuf}, process::Command};

use crate::{
    build::exec_build, options::{BuildOptions, FuzzDirWrapper}, project::FuzzProject, RunCommand
};
use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;


#[derive(Clone, Debug, Parser)]
pub struct Coverage {
    #[clap(flatten)] 
    pub build: BuildOptions,

    #[clap(flatten)] 
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    /// Sets the path to the LLVM bin directory. By default, it will use the one installed with rustc
    #[clap(long)]
    pub llvm_path: Option<PathBuf>,

    /// Custom corpus directories or artifact files
    pub corpus: Vec<String>,

    #[clap(last(true))]
    /// Additional libFuzzer arguments passed through to the binary
    pub args: Vec<String>,
}

impl RunCommand for Coverage {
    fn run_command(&mut self)-> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.to_owned())?;
        self.exec_coverage(&project)
    }
}

impl Coverage {
    /// Produce self information for a given corpus
    pub fn exec_coverage(&self, project: &FuzzProject) -> Result<()> {
        // Build project with source-based self generation enabled.
        exec_build(&self.build, project)?;

        // Retrieve corpus directories.
        let corpora = if self.corpus.is_empty() {
            vec![project.corpus_for(&self.build.target)?]
        } else {
            self
                .corpus
                .iter()
                .map(|name| Path::new(name).to_path_buf())
                .collect()
        };

        println!("Corpora: {:?}", corpora);

        // Collect the (non-directory) readable input files from the corpora.
        let files_and_dirs = corpora.iter().flat_map(fs::read_dir).flatten().flatten();
        let mut readable_input_files = files_and_dirs
        .filter(|file| match file.file_type() {
            Ok(ft) => ft.is_file(),
            _ => false,
            })
            .peekable();
        
        if readable_input_files.peek().is_none() {
            bail!(
                "The corpus does not contain program-input files. \
                Coverage information requires existing input files. \
                Try running the fuzzer first (`cargo fuzz run ...`) to generate a corpus, \
                or provide a nonempty corpus directory."
            )
        }

        let (self_out_raw_dir, self_out_file, self_coverage_map) = project.coverage_for(&self.build.target)?;
        println!("Raw dir:{:?}", self_out_raw_dir);
        println!("Out file:{:?}", self_out_file);
        println!("Map file:{:?}", self_coverage_map);

        for corpus in corpora.iter() {
            // _tmp_dir is deleted when it goes of of scope.
            let (mut cmd, _tmp_dir) =
                self.create_coverage_cmd(project, &self_coverage_map, corpus)?;
            eprintln!("Generating self data for corpus {:?}", corpus);
            let status = cmd
                .status()
                .with_context(|| format!("Failed to run command: {:?}", cmd))?;
            if !status.success() {
                Err(anyhow!(
                    "Command exited with failure status {}: {:?}",
                    status,
                    cmd
                ))
                .context("Failed to generage self data")?;
            }
        }

        // coverage merging not implemented yet

        // let mut profdata_bin_path = self.llvm_path.clone().unwrap_or(rustlib()?);
        // profdata_bin_path.push(format!("llvm-profdata{}", env::consts::EXE_SUFFIX));
        // Self::merge_coverage(
        //     &profdata_bin_path,
        //     &self_out_raw_dir,
        //     &self_out_file,
        // )?;

        Ok(())
    }

    fn create_coverage_cmd(
        &self,
        project: &FuzzProject,
        coverage_dir: &PathBuf,
        corpus_dir: &PathBuf,
    ) -> Result<(Command, tempfile::TempDir)> {
        let dummy_corpus = tempfile::tempdir()?;
        let args: Vec<Box<dyn AsRef<OsStr>>> = vec![
            Box::new(PathBuf::from(dummy_corpus.path())),
            Box::new(corpus_dir.clone())
        ];

        let mut cmd = project.get_run_fuzzer_command(&self.build.target, Some(coverage_dir), args)?;
        
        cmd.arg("-merge=1");

        for arg in &self.args {
            cmd.arg(arg);
        }

        println!("CMD: {:?}", cmd);

        Ok((cmd, dummy_corpus))
    }

    fn merge_coverage(
        profdata_bin_path: &Path,
        profdata_raw_path: &Path,
        profdata_out_path: &Path,
    ) -> Result<()> {
        let mut merge_cmd = Command::new(profdata_bin_path);
        merge_cmd.arg("merge").arg("-sparse");
        merge_cmd.arg(profdata_raw_path);
        merge_cmd.arg("-o").arg(profdata_out_path);

        eprintln!("Merging raw coverage data...");
        let status = merge_cmd
            .status()
            .with_context(|| format!("Failed to run command: {:?}", merge_cmd))
            .with_context(|| "Merging raw coverage files failed.\n\
                              \n\
                              Do you have LLVM coverage tools installed?\n\
                              https://doc.rust-lang.org/rustc/instrument-coverage.html#installing-llvm-coverage-tools")?;
        if !status.success() {
            Err(anyhow!(
                "Command exited with failure status {}: {:?}",
                status,
                merge_cmd
            ))
            .context("Merging raw coverage files failed")?;
        }

        if profdata_out_path.exists() {
            eprintln!("Coverage data merged and saved in {:?}.", profdata_out_path);
            Ok(())
        } else {
            bail!("Coverage data could not be merged.")
        }
    }
}