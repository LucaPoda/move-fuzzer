use crate::{
    options::{BuildOptions, FuzzDirWrapper},
    project::FuzzProject,
    RunCommand,
};
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use move_package::BuildConfig;

#[derive(Clone, Debug, Parser)]
pub struct Tmin {
    #[clap(flatten)] 
    pub build: BuildOptions,

    #[clap(flatten)] 
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    #[clap(
        short = 'r',
        long,
        default_value = "255",
    )]
    /// Number of minimization attempts to perform
    pub runs: u32,

    #[clap()]
    /// Path to the failing test case to be minimized
    pub test_case: PathBuf,

    #[clap(last(true))]
    /// Additional libFuzzer arguments passed through to the binary
    pub args: Vec<String>,
}

impl RunCommand for Tmin {
    fn run_command(&mut self)-> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.to_owned())?;
        project.exec_tmin(self)
    }
}
