use crate::project::FuzzProject;
use crate::templates::create_target_template;
use crate::Target;
use crate::{options::FuzzDirWrapper, RunCommand};
use anyhow::{Context, Result};
use clap::*;

use move_package::BuildConfig;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Parser)]
pub struct Add {
    #[clap(flatten)] 
    pub fuzz_dir_wrapper: FuzzDirWrapper,

    /// Name of the new fuzz target
    pub target: String,
}

impl RunCommand for Add {
    fn run_command(&mut self)-> Result<()> {
        let project = FuzzProject::new(self.fuzz_dir_wrapper.fuzz_dir.to_owned())?;
        self.add_target(&project)
    }
}

impl Add {
    /// Create a new fuzz target.
    pub fn add_target(&self, project: &FuzzProject) -> Result<()> {
        let target = Target {
            target_module: None,
            target_function: None,
            target_name: Some(self.target.clone()),
        };

        // Create corpus and artifact directories for the newly added target
        project.corpus_for(&target)?;
        project.artifacts_for(&target)?;
        
        create_target_template(project, &self.target)
            .with_context(|| format!("could not add target {:?}", self.target))
    }
}