use crate::{
    build::exec_build, options::{BuildOptions, FuzzDirWrapper}, project::FuzzProject, RunCommand
};
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::{fs, path::PathBuf};

use move_package::BuildConfig;

#[derive(Clone, Debug, Parser)]
pub struct Cmin {
    #[clap(flatten)]  
    pub build: BuildOptions,

    #[clap(flatten)] 
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    #[clap()]
    /// The corpus directory to minify into
    pub corpus: Option<PathBuf>,

    #[clap(last(true))]
    /// Additional libFuzzer arguments passed through to the binary
    pub args: Vec<String>,
}

impl RunCommand for Cmin {
    fn run_command(&mut self)-> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.to_owned())?;
        self.exec_cmin(&project)
    }
}

impl Cmin {
    pub fn exec_cmin(&self, project: &FuzzProject) -> Result<()> {
        exec_build(&self.build, project, false)?;
        let mut cmd = project.get_run_fuzzer_command(&self.build.target)?;
        // todo: trasformare cargo run nel comando che ritorna la chiamata al fuzzer installato

        for arg in &self.args {
            cmd.arg(arg);
        }

        let corpus = if let Some(corpus) = self.corpus.clone() {
            corpus
        } else {
            project.corpus_for(&self.build.target)?
        };
        let corpus = corpus
            .to_str()
            .ok_or_else(|| anyhow!("corpus must be valid unicode"))?
            .to_owned();

        let tmp: tempfile::TempDir = tempfile::TempDir::new_in(project.get_fuzz_dir())?;
        let tmp_corpus = tmp.path().join("corpus");
        fs::create_dir(&tmp_corpus)?;

        // cmd.arg("-merge=1").arg(&tmp_corpus).arg(&corpus); // todo: passare argomento a move-fuzzer

        // Spawn cmd in child process instead of exec-ing it
        let status = cmd
            .status()
            .with_context(|| format!("could not execute command: {:?}", cmd))?;
        if status.success() {
            // move corpus directory into tmp to auto delete it
            fs::rename(&corpus, tmp.path().join("old"))?;
            fs::rename(tmp.path().join("corpus"), corpus)?;
        } else {
            println!("Failed to minimize corpus: {}", status);
        }

        Ok(())
    }
}