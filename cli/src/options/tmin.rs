use crate::{
    build::exec_build, options::{BuildOptions, FuzzDirWrapper}, project::FuzzProject, run::run_fuzz_target_debug_formatter, utils::strip_current_dir_prefix, RunCommand
};
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::{path::PathBuf, time};



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
        self.exec_tmin(&project)
    }
}

impl Tmin {
    pub fn exec_tmin(&self, project: &FuzzProject) -> Result<()> {
        exec_build(&self.build, project)?;
        let mut cmd = project.get_run_fuzzer_command(&self.build.target, None, vec![])?;
        cmd.arg("-minimize_crash=1")
            .arg(format!("-runs={}", self.runs))
            .arg(&self.test_case);

        for arg in &self.args {
            cmd.arg(arg);
        }

        let before_tmin = time::SystemTime::now();

        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn command: {:?}", cmd))?;
        let status = child
            .wait()
            .with_context(|| format!("failed to wait on child process for command: {:?}", cmd))?;
        if !status.success() {
            eprintln!("\n{:─<80}\n", "");
            return Err(anyhow!("Command `{:?}` exited with {}", cmd, status)).with_context(|| {
                "Test case minimization failed.\n\
                 \n\
                 Usually this isn't a hard error, and just means that libfuzzer\n\
                 doesn't know how to minimize the test case any further while\n\
                 still reproducing the original crash.\n\
                 \n\
                 See the logs above for details."
            });
        }

        // Find and display the most recently modified artifact, which is
        // presumably the result of minification. Yeah, this is a little hacky,
        // but it seems to work. I don't want to parse libfuzzer's stderr output
        // and hope it never changes.
        let minimized_artifact = project
            .get_artifacts_since(&self.build.target, &before_tmin)?
            .into_iter()
            .max_by_key(|a| {
                a.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(time::SystemTime::UNIX_EPOCH)
            });

        if let Some(artifact) = minimized_artifact {
            let artifact = strip_current_dir_prefix(&artifact);

            eprintln!("\n{:─<80}\n", "");
            eprintln!("Minimized artifact:\n\n\t{}\n", artifact.display());

            // Note: ignore errors when running the debug formatter. This most
            // likely just means that we're dealing with a fuzz target that uses
            // an older version of the libfuzzer crate, and doesn't support
            // `MOVE_LIBFUZZER_DEBUG_PATH`.
            if let Ok(debug) = run_fuzz_target_debug_formatter(project, &self.build, &self.build.target, artifact)
            {
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
                "Reproduce with:\n\n\tcargo fuzz run{fuzz_dir}{options} {target} {artifact}\n",
                fuzz_dir = &fuzz_dir,
                options = &self.build,
                target = self.build.target.get_command(),
                artifact = artifact.display()
            );
        }

        Ok(())
    }
}