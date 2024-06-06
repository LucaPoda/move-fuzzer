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
pub struct Build {
    #[clap(flatten)]  
    pub build: BuildOptions,

    #[clap(flatten)] 
    pub fuzz_dir_wrapper: FuzzDirWrapper,    
}

impl RunCommand for Build {
    fn run_command(&mut self)-> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.to_owned())?;
        project.exec_build(&self.build, false)
    }
}
