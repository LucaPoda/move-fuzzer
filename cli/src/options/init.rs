use crate::{options::FuzzDirWrapper, project::FuzzProject, RunCommand};
use anyhow::Result;
use clap::Parser;

use move_package::BuildConfig;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Parser)]
pub struct Init {
    #[clap(short, long, required = false, default_value = "fuzz_target_1")]
    /// Name of the first fuzz target to create
    pub target: String,

    #[clap(long)]
    /// Whether to create a separate workspace for fuzz targets crate
    pub fuzzing_workspace: Option<bool>,

    #[clap(flatten)] 
    pub fuzz_dir_wrapper: FuzzDirWrapper,
}

impl RunCommand for Init {
    fn run_command(&mut self)-> Result<()> {
        FuzzProject::init(self, self.fuzz_dir_wrapper.fuzz_dir.to_owned())?;
        Ok(())
    }
}
