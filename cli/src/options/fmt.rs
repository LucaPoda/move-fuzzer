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
pub struct Fmt {
    #[clap(flatten)] 
    pub build: BuildOptions,

    #[clap(flatten)] 
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    /// Path to the input testcase to debug print
    pub input: PathBuf,
}

impl RunCommand for Fmt {
    fn run_command(&mut self)-> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.to_owned())?;
        project.debug_fmt_input(self)
    }
}
