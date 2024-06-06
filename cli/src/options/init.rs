use crate::{options::FuzzDirWrapper, project::FuzzProject, templates::create_target_template, utils::manage_initial_instance, RunCommand};
use anyhow::{Context, Result};
use clap::Parser;


use std::{fs, io::Write, path::{PathBuf}};

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
        Self::init(self, self.fuzz_dir_wrapper.fuzz_dir.to_owned())?;
        Ok(())
    }
}

impl Init {
    /// Creates the fuzz project structure and returns a new instance.
    ///
    /// This will not clone move-fuzzer.
    /// Similar to `FuzzProject::new`, the fuzz directory will depend on `fuzz_dir_opt`.
    pub fn init(&self, fuzz_dir_opt: Option<PathBuf>) -> Result<FuzzProject> {
        let project = manage_initial_instance(fuzz_dir_opt)?;
        let fuzz_project = project.get_fuzz_dir();

        // TODO: check if the project is already initialized
        fs::create_dir(fuzz_project)
            .with_context(|| format!("failed to create directory {}", fuzz_project.display()))?;

        let move_toml_path = fuzz_project.join("Move.toml");

        let mut move_toml = fs::File::create(&move_toml_path)
            .with_context(|| format!("failed to create {}", move_toml_path.display()))?;

        move_toml
            .write_fmt(move_toml_template!())
            .with_context(|| format!("failed to write to {}", move_toml_path.display()))?;

        let gitignore = fuzz_project.join(".gitignore");
        let mut ignore = fs::File::create(&gitignore)
            .with_context(|| format!("failed to create {}", gitignore.display()))?;
        ignore
            .write_fmt(gitignore_template!())
            .with_context(|| format!("failed to write to {}", gitignore.display()))?;

        create_target_template(&project, &self.target)
            .with_context(|| {
                format!(
                    "could not create template file for target {:?}",
                    self.target
                )
            })?;
        Ok(project)
    }
}
