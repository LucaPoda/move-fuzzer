use crate::{
    options::{BuildOptions, FuzzDirWrapper},
    project::FuzzProject,
    RunCommand,
};
use anyhow::{bail, Context, Result};
use clap::Parser;


use std::{process::Command};

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
        exec_build(&self.build, &project, false)
    }
}

fn move_build(build: &BuildOptions) -> Result<Command> {
    let mut cmd = Command::new("move");
    cmd.arg("build");
    
    if build.verbose {
        cmd.arg("--verbose");
    }

    if build.build_config.fetch_deps_only {
        cmd.arg("--fetch-deps-only");
    }

    if build.build_config.force_recompilation {
        cmd.arg("--force");
    }

    if build.build_config.skip_fetch_latest_git_deps {
        cmd.arg("--skip-fetch-latest-git-deps");
    }

    Ok(cmd)
}

pub fn exec_build(
    build: &BuildOptions,
    project: &FuzzProject,
    coverage: bool
) -> Result<()> {
    let mut move_cmd = move_build(build)?;

    if let Some(target_dir) = project.get_target_dir(&build.package_path, coverage)? {
        move_cmd.arg("--path").arg(&target_dir);
    }
    else {
        move_cmd.arg("--path").arg(&project.get_fuzz_dir());
    }

    let mut move_build = Command::new("move");
    move_build.arg("build").current_dir("fuzz");

    let move_status = move_build
        .status()
        .with_context(|| format!("failed to execute: {:?}", move_build))?;
    if !move_status.success() {
        //bail!("failed to build fuzz script: {:?}", move_build);
        bail!("failed to build fuzz script: {:?}", move_build);
    }

    Ok(())
}

