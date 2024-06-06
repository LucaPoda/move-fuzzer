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
        project.exec_cmin(self)
    }
}
