use crate::{
    build::exec_build, options::{BuildOptions, FuzzDirWrapper}, project::FuzzProject, utils::strip_current_dir_prefix, RunCommand, Target
};
use anyhow::{bail, Context, Result};
use clap::Parser;

use std::{fs, path::Path, process::Stdio, time};

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
        self.exec_fuzz(&project)
    }
}


pub fn run_fuzz_target_debug_formatter(
    project: &FuzzProject,
    build: &BuildOptions,
    target: &Target,
    artifact: &Path,
) -> Result<String> {
    let debug_output = tempfile::NamedTempFile::new().context("failed to create temp file")?;

    let mut cmd = project.get_run_fuzzer_command(&build.target)?;
    cmd.stdin(Stdio::null());
    cmd.env("MOVE_LIBFUZZER_DEBUG_PATH", debug_output.path());
    cmd.arg(artifact);

    let output = cmd
        .output()
        .with_context(|| format!("failed to run command: {:?}", cmd))?;

    let target_message = if let Some(target_name) = &target.target_name {
        format!("Fuzz target '{target_name}")
    }
    else {
        let module = target.target_module.clone().expect("Module name is missing");
        let function = target.target_function.clone().expect("Target function is missing");

        format!("Function '{function}' in module '{module}")
    };

    if !output.status.success() {
        bail!(
            "{target_message} exited with failure when attempting to \
             debug formatting an interesting input that we discovered!\n\n\
             Artifact: {artifact}\n\n\
             Command: {cmd:?}\n\n\
             Status: {status}\n\n\
             === stdout ===\n\
             {stdout}\n\n\
             === stderr ===\n\
             {stderr}",
            status = output.status,
            cmd = cmd,
            artifact = artifact.display(),
            stdout = String::from_utf8_lossy(&output.stdout),
            stderr = String::from_utf8_lossy(&output.stderr),
        );
    }

    let debug = fs::read_to_string(&debug_output).context("failed to read temp file")?;
    Ok(debug)
}


impl Run {
    /// Fuzz a given fuzz target
    pub fn exec_fuzz(&self, project: &FuzzProject) -> Result<()> {
        exec_build(&self.build, project, false)?;
        let mut cmd = project.get_run_fuzzer_command(&self.build.target)?;

        for arg in &self.args {
            cmd.arg(arg);
        }

        if !self.corpus.is_empty() {
            for corpus in &self.corpus {
                cmd.arg(corpus);
            }
        } else {
            cmd.arg(project.corpus_for(&self.build.target)?);
        }

        if self.jobs != 1 {
            cmd.arg(format!("-fork={}", self.jobs));
        }

        // When libfuzzer finds failing inputs, those inputs will end up in the
        // artifacts directory. To easily filter old artifacts from new ones,
        // get the current time, and then later we only consider files modified
        // after now.
        let before_fuzzing = time::SystemTime::now();

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn command: {:?}", cmd))?;
        let status = child
            .wait()
            .with_context(|| format!("failed to wait on child process for command: {:?}", cmd))?;
        if status.success() {
            return Ok(());
        }

        // Get and print the `Debug` formatting of any new artifacts, along with
        // tips about how to reproduce failures and/or minimize test cases.

        let new_artifacts = project.get_artifacts_since(&self.build.target, &before_fuzzing)?;

        for artifact in new_artifacts {
            // To make the artifact a little easier to read, strip the current
            // directory prefix when possible.
            let artifact = strip_current_dir_prefix(&artifact);

            eprintln!("\n{:─<80}", "");
            eprintln!("\nFailing input:\n\n\t{}\n", artifact.display());

            // Note: ignore errors when running the debug formatter. This most
            // likely just means that we're dealing with a fuzz target that uses
            // an older version of the libfuzzer crate, and doesn't support
            // `MOVE_LIBFUZZER_DEBUG_PATH`.
            if let Ok(debug) = run_fuzz_target_debug_formatter(project, &self.build, &self.build.target, artifact) {
                eprintln!("Output of `std::fmt::Debug`:\n");
                for l in debug.lines() {
                    eprintln!("\t{}", l);
                }
                eprintln!();
            }

            let fuzz_dir = if project.fuzz_dir_is_default_path() {
                String::new()
            } else {
                format!(" --fuzz-dir {}", project.get_fuzz_dir().display())
            };

            eprintln!(
                "Reproduce with:\n\n\tcargo fuzz run{fuzz_dir}{options} {target} {artifact} \n",
                fuzz_dir = &fuzz_dir,
                options = &self.build,
                target = self.build.target.get_command(),
                artifact = artifact.display()
            );
            eprintln!(
                "Minimize test case with:\n\n\tcargo fuzz tmin{fuzz_dir}{options} {target} {artifact}\n",
                fuzz_dir = &fuzz_dir,
                options = &self.build,
                target = self.build.target.get_command(),
                artifact = artifact.display()
            );
        }

        eprintln!("{:─<80}\n", "");
        bail!("Fuzz target exited with {}", status)
    }
}
