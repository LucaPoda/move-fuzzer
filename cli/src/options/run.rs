use crate::{
    options::{BuildOptions, FuzzDirWrapper},
    project::FuzzProject,
    RunCommand,
};
use anyhow::Result;
use clap::Parser;

use move_package::BuildConfig;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Parser)]
pub struct Run {
    #[clap(flatten)] 
    pub build: BuildOptions,

    /// Custom corpus directories or artifact files.
    pub corpus: Vec<String>,

    #[clap(flatten)] 
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    #[clap(
        short,
        long,
        default_value = "1",
    )]
    /// Number of concurrent jobs to run
    pub jobs: u16,

    #[clap(last(true))]
    /// Additional libFuzzer arguments passed through to the binary
    pub args: Vec<String>,
}

impl RunCommand for Run {
    fn run_command(&mut self) -> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.to_owned())?;
        project.exec_fuzz(self)
    }
}
