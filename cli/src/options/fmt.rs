use crate::{
    options::{BuildOptions, FuzzDirWrapper}, project::FuzzProject, run::run_fuzz_target_debug_formatter, RunCommand
};
use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::PathBuf;


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
        self.debug_fmt_input(&project)
    }
}

impl Fmt {

    /// Prints the debug output of an input test case
    pub fn debug_fmt_input(&self, project: &FuzzProject) -> Result<()> {
        if !self.input.exists() {
            bail!(
                "Input test case does not exist: {}",
                self.input.display()
            );
        }

        let debug = run_fuzz_target_debug_formatter(project, &self.build, &self.build.target, &self.input)
            .with_context(|| {
                format!(
                    "failed to run `cargo fuzz fmt` on input: {}",
                    self.input.display()
                )
            })?;

        eprintln!("\nOutput of `std::fmt::Debug`:\n");
        for l in debug.lines() {
            eprintln!("{}", l);
        }

        Ok(())
    }
}