use crate::options::{self, BuildOptions};
use crate::utils::default_target;
use crate::{Build, Target};
use anyhow::{anyhow, bail, Context, Result};
use cargo_metadata::MetadataCommand;
use move_package::BuildConfig;
use std::collections::HashSet;
use std::ffi;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{
    env, fs,
    process::{Command, Stdio},
    time,
};

const DEFAULT_FUZZ_DIR: &str = "fuzz";

pub struct FuzzProject {
    /// The project with fuzz targets
    fuzz_dir: PathBuf,
    targets: Vec<String>,
}

impl FuzzProject {
    /// Creates a new instance.
    //
    /// Find an existing `cargo fuzz` project by starting at the current
    /// directory and walking up the filesystem.
    ///
    /// If `fuzz_dir_opt` is `None`, returns a new instance with the default fuzz project
    /// path.
    pub fn new(fuzz_dir_opt: Option<PathBuf>) -> Result<Self> {
        let mut project = Self::manage_initial_instance(fuzz_dir_opt)?;
        let manifest = project.manifest()?;
        if !is_fuzz_manifest(&manifest) {
            bail!(
                "manifest `{}` does not look like a move-fuzz manifest. \
                 The package name should end with \"fuzz\"",
                project.manifest_path().display()
            );
        }
        project.targets = collect_targets(&manifest);
        Ok(project)
    }

    /// Creates the fuzz project structure and returns a new instance.
    ///
    /// This will not clone move-fuzzer.
    /// Similar to `FuzzProject::new`, the fuzz directory will depend on `fuzz_dir_opt`.
    pub fn init(init: &options::Init, fuzz_dir_opt: Option<PathBuf>) -> Result<Self> {
        let project = Self::manage_initial_instance(fuzz_dir_opt)?;
        let fuzz_project = project.fuzz_dir();
        let manifest = Manifest::parse()?;
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

        project
            .create_target_template(&init.target, &manifest)
            .with_context(|| {
                format!(
                    "could not create template file for target {:?}",
                    init.target
                )
            })?;
        Ok(project)
    }

    pub fn list_targets(&self) -> Result<()> {
        for bin in &self.targets {
            println!("{}", bin);
        }
        Ok(())
    }

    fn move_build(&self, build: &BuildOptions) -> Result<Command> {
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

    /// Create a new fuzz target.
    pub fn add_target(&self, add: &options::Add, manifest: &Manifest) -> Result<()> {
        let target = Target {
            target_module: None,
            target_function: None,
            target_name: Some(add.target.clone()),
        };

        // Create corpus and artifact directories for the newly added target
        self.corpus_for(&target)?;
        self.artifacts_for(&target)?;
        self.create_target_template(&add.target, manifest)
            .with_context(|| format!("could not add target {:?}", add.target))
    }

    /// Add a new fuzz target script with a given name
    fn create_target_template(&self, target: &str, manifest: &Manifest) -> Result<()> {
        let move_target_path = self.move_target_path(target);

        // If the user manually created a fuzz project, but hasn't created any
        // targets yet, the `fuzz_targets` directory might not exist yet,
        // despite a `fuzz/Cargo.toml` manifest with the `metadata.cargo-fuzz`
        // key present. Make sure it does exist.
        fs::create_dir_all(self.move_targets_dir())
            .context("ensuring that `sources` directory exists failed")?;

        let mut move_script = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&move_target_path)
            .with_context(|| format!("could not create target script file at {:?}", move_target_path))?;
        move_script.write_fmt(move_target_template!(target))?;

        Ok(())
    }

    // note: never returns Ok(None) if build.coverage is true
    fn target_dir(&self, path: &Option<PathBuf>, coverage: bool) -> Result<Option<PathBuf>> {
        // Use the user-provided target directory, if provided. Otherwise if building for coverage,
        // use the coverage directory
        if let Some(target_dir) = path.clone() {
            return Ok(Some(target_dir));
        } else if coverage {
            // To ensure that fuzzing and coverage-output generation can run in parallel, we
            // produce a separate binary for the coverage command.
            let current_dir = env::current_dir()?;
            Ok(Some(
                current_dir
                    .join("target")
                    .join(default_target())
                    .join("coverage"),
            ))
        } else {
            Ok(None)
        }
    }

    pub fn exec_build(
        &self,
        build: &options::BuildOptions,
        coverage: bool
    ) -> Result<()> {
        let mut move_cmd = self.move_build(build)?;

        if let Some(target_dir) = self.target_dir(&build.package_path, coverage)? {
            move_cmd.arg("--path").arg(&target_dir);
        }
        else {
            move_cmd.arg("--path").arg(&self.fuzz_dir());
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

    fn get_artifacts_since(
        &self,
        target: &Target,
        since: &time::SystemTime,
    ) -> Result<HashSet<PathBuf>> {
        let mut artifacts = HashSet::new();

        let artifacts_dir = self.artifacts_for(target)?;

        for entry in fs::read_dir(&artifacts_dir).with_context(|| {
            format!(
                "failed to read directory entries of {}",
                artifacts_dir.display()
            )
        })? {
            let entry = entry.with_context(|| {
                format!(
                    "failed to read directory entry inside {}",
                    artifacts_dir.display()
                )
            })?;

            let metadata = entry
                .metadata()
                .context("failed to read artifact metadata")?;
            let modified = metadata
                .modified()
                .context("failed to get artifact modification time")?;
            if !metadata.is_file() || modified <= *since {
                continue;
            }

            artifacts.insert(entry.path());
        }

        Ok(artifacts)
    }

    fn run_fuzz_target_debug_formatter(
        &self,
        build: &BuildOptions,
        target: &Target,
        artifact: &Path,
    ) -> Result<String> {
        let debug_output = tempfile::NamedTempFile::new().context("failed to create temp file")?;

        let mut cmd = self.cargo_run(&build.target)?;
        cmd.stdin(Stdio::null());
        cmd.env("RUST_LIBFUZZER_DEBUG_PATH", debug_output.path());
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

    /// Prints the debug output of an input test case
    pub fn debug_fmt_input(&self, debugfmt: &options::Fmt) -> Result<()> {
        if !debugfmt.input.exists() {
            bail!(
                "Input test case does not exist: {}",
                debugfmt.input.display()
            );
        }

        let debug = self
            .run_fuzz_target_debug_formatter(&debugfmt.build, &debugfmt.build.target, &debugfmt.input)
            .with_context(|| {
                format!(
                    "failed to run `cargo fuzz fmt` on input: {}",
                    debugfmt.input.display()
                )
            })?;

        eprintln!("\nOutput of `std::fmt::Debug`:\n");
        for l in debug.lines() {
            eprintln!("{}", l);
        }

        Ok(())
    }

    /// Fuzz a given fuzz target
    pub fn exec_fuzz(&self, run: &options::Run) -> Result<()> {
        self.exec_build(&run.build, false)?;
        let mut cmd = self.cargo_run(&run.build.target)?;

        for arg in &run.args {
            cmd.arg(arg);
        }

        if !run.corpus.is_empty() {
            for corpus in &run.corpus {
                cmd.arg(corpus);
            }
        } else {
            cmd.arg(self.corpus_for(&run.build.target)?);
        }

        if run.jobs != 1 {
            cmd.arg(format!("-fork={}", run.jobs));
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

        let new_artifacts = self.get_artifacts_since(&run.build.target, &before_fuzzing)?;

        for artifact in new_artifacts {
            // To make the artifact a little easier to read, strip the current
            // directory prefix when possible.
            let artifact = strip_current_dir_prefix(&artifact);

            eprintln!("\n{:─<80}", "");
            eprintln!("\nFailing input:\n\n\t{}\n", artifact.display());

            // Note: ignore errors when running the debug formatter. This most
            // likely just means that we're dealing with a fuzz target that uses
            // an older version of the libfuzzer crate, and doesn't support
            // `RUST_LIBFUZZER_DEBUG_PATH`.
            if let Ok(debug) =
                self.run_fuzz_target_debug_formatter(&run.build, &run.build.target, artifact)
            {
                eprintln!("Output of `std::fmt::Debug`:\n");
                for l in debug.lines() {
                    eprintln!("\t{}", l);
                }
                eprintln!();
            }

            let fuzz_dir = if self.fuzz_dir_is_default_path() {
                String::new()
            } else {
                format!(" --fuzz-dir {}", self.fuzz_dir().display())
            };

            eprintln!(
                "Reproduce with:\n\n\tcargo fuzz run{fuzz_dir}{options} {target} {artifact} \n",
                fuzz_dir = &fuzz_dir,
                options = &run.build,
                target = run.build.target.get_command(),
                artifact = artifact.display()
            );
            eprintln!(
                "Minimize test case with:\n\n\tcargo fuzz tmin{fuzz_dir}{options} {target} {artifact}\n",
                fuzz_dir = &fuzz_dir,
                options = &run.build,
                target = run.build.target.get_command(),
                artifact = artifact.display()
            );
        }

        eprintln!("{:─<80}\n", "");
        bail!("Fuzz target exited with {}", status)
    }

    pub fn exec_tmin(&self, tmin: &options::Tmin) -> Result<()> {
        self.exec_build(&tmin.build, false)?;
        let mut cmd = self.cargo_run(&tmin.build.target)?;
        cmd.arg("-minimize_crash=1")
            .arg(format!("-runs={}", tmin.runs))
            .arg(&tmin.test_case);

        for arg in &tmin.args {
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
        let minimized_artifact = self
            .get_artifacts_since(&tmin.build.target, &before_tmin)?
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
            // `RUST_LIBFUZZER_DEBUG_PATH`.
            if let Ok(debug) =
                self.run_fuzz_target_debug_formatter(&tmin.build, &tmin.build.target, artifact)
            {
                eprintln!("Output of `std::fmt::Debug`:\n");
                for l in debug.lines() {
                    eprintln!("\t{}", l);
                }
                eprintln!();
            }

            let fuzz_dir = if self.fuzz_dir_is_default_path() {
                String::new()
            } else {
                format!(" --fuzz-dir {}", self.fuzz_dir().display())
            };

            eprintln!(
                "Reproduce with:\n\n\tcargo fuzz run{fuzz_dir}{options} {target} {artifact}\n",
                fuzz_dir = &fuzz_dir,
                options = &tmin.build,
                target = tmin.build.target.get_command(),
                artifact = artifact.display()
            );
        }

        Ok(())
    }

    fn cargo_run(&self, target: &Target) -> Result<Command> {
        let mut module_path = self.fuzz_dir.clone();
        module_path.push("build");
        module_path.push("fuzz");
        module_path.push("bytecode_modules");
        module_path.push(format!("{}.mv", target.get_module_name()));

        let mut cmd = Command::new("move-fuzzer-worker");

        let mut module_path_arg = ffi::OsString::from("--module-path=");    
        module_path_arg.push(module_path);

        let mut target_module_arg = ffi::OsString::from("--target-module=");    
        target_module_arg.push(target.get_module_name());

        let mut target_function_arg = ffi::OsString::from("--target-function=");    
        target_function_arg.push(target.get_target_function());

        let mut artifact_arg = ffi::OsString::from("-artifact_prefix=");
        artifact_arg.push(self.artifacts_for(target)?);
        
        cmd.arg(module_path_arg)
            .arg(target_module_arg)
            .arg(target_function_arg)
            .arg(artifact_arg);

        Ok(cmd)
    }

    pub fn exec_cmin(&self, cmin: &options::Cmin) -> Result<()> {
        self.exec_build(&cmin.build, false)?;
        let mut cmd = self.cargo_run(&cmin.build.target)?;
        // todo: trasformare cargo run nel comando che ritorna la chiamata al fuzzer installato

        for arg in &cmin.args {
            cmd.arg(arg);
        }

        let corpus = if let Some(corpus) = cmin.corpus.clone() {
            corpus
        } else {
            self.corpus_for(&cmin.build.target)?
        };
        let corpus = corpus
            .to_str()
            .ok_or_else(|| anyhow!("corpus must be valid unicode"))?
            .to_owned();

        let tmp: tempfile::TempDir = tempfile::TempDir::new_in(self.fuzz_dir())?;
        let tmp_corpus = tmp.path().join("corpus");
        fs::create_dir(&tmp_corpus)?;

        // cmd.arg("-merge=1").arg(&tmp_corpus).arg(&corpus); // todo: passare argomento a move-fuzzer

        // Spawn cmd in child process instead of exec-ing it
        let status = cmd
            .status()
            .with_context(|| format!("could not execute command: {:?}", cmd))?;
        if status.success() {
            // move corpus directory into tmp to auto delete it
            fs::rename(&corpus, tmp.path().join("old"))?;
            fs::rename(tmp.path().join("corpus"), corpus)?;
        } else {
            println!("Failed to minimize corpus: {}", status);
        }

        Ok(())
    }

    /// Produce coverage information for a given corpus
    pub fn exec_coverage(self, coverage: &options::Coverage) -> Result<()> {
        // Build project with source-based coverage generation enabled.
        self.exec_build(&coverage.build, true)?;

        // Retrieve corpus directories.
        let corpora = if coverage.corpus.is_empty() {
            vec![self.corpus_for(&coverage.build.target)?]
        } else {
            coverage
                .corpus
                .iter()
                .map(|name| Path::new(name).to_path_buf())
                .collect()
        };

        // Collect the (non-directory) readable input files from the corpora.
        let files_and_dirs = corpora.iter().flat_map(fs::read_dir).flatten().flatten();
        let mut readable_input_files = files_and_dirs
            .filter(|file| match file.file_type() {
                Ok(ft) => ft.is_file(),
                _ => false,
            })
            .peekable();
        if readable_input_files.peek().is_none() {
            bail!(
                "The corpus does not contain program-input files. \
                 Coverage information requires existing input files. \
                 Try running the fuzzer first (`cargo fuzz run ...`) to generate a corpus, \
                 or provide a nonempty corpus directory."
            )
        }

        let (coverage_out_raw_dir, coverage_out_file) = self.coverage_for(&coverage.build.target)?;

        for corpus in corpora.iter() {
            // _tmp_dir is deleted when it goes of of scope.
            let (mut cmd, _tmp_dir) =
                self.create_coverage_cmd(coverage, &coverage_out_raw_dir, &corpus.as_path())?;
            eprintln!("Generating coverage data for corpus {:?}", corpus);
            let status = cmd
                .status()
                .with_context(|| format!("Failed to run command: {:?}", cmd))?;
            if !status.success() {
                Err(anyhow!(
                    "Command exited with failure status {}: {:?}",
                    status,
                    cmd
                ))
                .context("Failed to generage coverage data")?;
            }
        }

        let mut profdata_bin_path = coverage.llvm_path.clone().unwrap_or(rustlib()?);
        profdata_bin_path.push(format!("llvm-profdata{}", env::consts::EXE_SUFFIX));
        self.merge_coverage(
            &profdata_bin_path,
            &coverage_out_raw_dir,
            &coverage_out_file,
        )?;

        Ok(())
    }

    fn create_coverage_cmd(
        &self,
        coverage: &options::Coverage,
        coverage_dir: &Path,
        corpus_dir: &Path,
    ) -> Result<(Command, tempfile::TempDir)> {

        // todo: probabilmente binpath è semplicemente il nome dell'eseguibile
        let bin_path = {
            let profile_subdir = if coverage.build.build_config.dev_mode {
                "debug"
            } else {
                "release"
            };

            let target_dir = self
                .target_dir(&coverage.build.package_path, true)?
                .expect("target dir for coverage command should never be None");
            target_dir
                .join(profile_subdir)
                // .join(&coverage.target)
        };

        let mut cmd = Command::new(bin_path);

        // Raw coverage data will be saved in `coverage/<target>` directory.
        let corpus_dir_name = corpus_dir
            .file_name()
            .and_then(|x| x.to_str())
            .with_context(|| format!("Invalid corpus directory: {:?}", corpus_dir))?;
        cmd.env(
            "LLVM_PROFILE_FILE",
            coverage_dir.join(format!("default-{}.profraw", corpus_dir_name)),
        );
        cmd.arg("-merge=1");
        let dummy_corpus = tempfile::tempdir()?;
        cmd.arg(dummy_corpus.path());
        cmd.arg(corpus_dir);

        for arg in &coverage.args {
            cmd.arg(arg);
        }

        Ok((cmd, dummy_corpus))
    }

    fn merge_coverage(
        &self,
        profdata_bin_path: &Path,
        profdata_raw_path: &Path,
        profdata_out_path: &Path,
    ) -> Result<()> {
        let mut merge_cmd = Command::new(profdata_bin_path);
        merge_cmd.arg("merge").arg("-sparse");
        merge_cmd.arg(profdata_raw_path);
        merge_cmd.arg("-o").arg(profdata_out_path);

        eprintln!("Merging raw coverage data...");
        let status = merge_cmd
            .status()
            .with_context(|| format!("Failed to run command: {:?}", merge_cmd))
            .with_context(|| "Merging raw coverage files failed.\n\
                              \n\
                              Do you have LLVM coverage tools installed?\n\
                              https://doc.rust-lang.org/rustc/instrument-coverage.html#installing-llvm-coverage-tools")?;
        if !status.success() {
            Err(anyhow!(
                "Command exited with failure status {}: {:?}",
                status,
                merge_cmd
            ))
            .context("Merging raw coverage files failed")?;
        }

        if profdata_out_path.exists() {
            eprintln!("Coverage data merged and saved in {:?}.", profdata_out_path);
            Ok(())
        } else {
            bail!("Coverage data could not be merged.")
        }
    }

    pub(crate) fn fuzz_dir(&self) -> &Path {
        &self.fuzz_dir
    }

    fn manifest_path(&self) -> PathBuf {
        self.fuzz_dir().join("Move.toml")
    }

    fn move_fuzzer_path(&self) -> PathBuf {
        // il path può essere passato in 2 modi: 
        // 1) come argomento del comando
        // 2) è già salvato in una variabile d'ambiente
        // 3) a runtime se non è richiesta la silent mode (verrà richiesto se salvarlo per usi futuri)
        let libfuzzer_path = "/Users/lucapodavini/Projects/Thesis/move-sui/crates/move-fuzzer/Cargo.toml";
        PathBuf::from(libfuzzer_path)
    }

    /// Returns paths to the `coverage/<target>/raw` directory and `coverage/<target>/coverage.profdata` file.
    fn coverage_for(&self, target: &Target) -> Result<(PathBuf, PathBuf)> {
        let mut coverage_data = self.fuzz_dir().to_owned();
        coverage_data.push("coverage");
        coverage_data.push(target.get_module_name());
        coverage_data.push(target.get_target_function());

        let mut coverage_raw = coverage_data.clone();
        coverage_data.push("coverage.profdata");
        coverage_raw.push("raw");

        fs::create_dir_all(&coverage_raw).with_context(|| {
            format!("could not make a coverage directory at {:?}", coverage_raw)
        })?;
        Ok((coverage_raw, coverage_data))
    }

    fn corpus_for(&self, target: &Target) -> Result<PathBuf> {
        let mut p = self.fuzz_dir().to_owned();
        p.push("corpus");
        p.push(target.get_module_name());
        p.push(target.get_target_function());
        fs::create_dir_all(&p)
            .with_context(|| format!("could not make a corpus directory at {:?}", p))?;
        Ok(p)
    }

    fn artifacts_for(&self, target: &Target) -> Result<PathBuf> {
        let mut p = self.fuzz_dir().to_owned();
        p.push("artifacts");
        p.push(target.get_module_name());
        p.push(target.get_target_function());

        // This adds a trailing slash, which is necessary for libFuzzer, because
        // it does simple string concatenation when joining paths.
        p.push("");

        fs::create_dir_all(&p)
            .with_context(|| format!("could not make a artifact directory at {:?}", p))?;

        Ok(p)
    }

    fn fuzz_targets_dir(&self) -> PathBuf {
        let mut root = self.fuzz_dir().to_owned();
        if root.join(crate::FUZZ_TARGETS_DIR_OLD).exists() {
            println!(
                "warning: The `fuzz/fuzzers/` directory has renamed to `fuzz/fuzz_targets/`. \
                 Please rename the directory as such. This will become a hard error in the \
                 future."
            );
            root.push(crate::FUZZ_TARGETS_DIR_OLD);
        } else {
            root.push(crate::FUZZ_TARGETS_DIR);
        }
        root
    }

    fn move_targets_dir(&self) -> PathBuf {
        let mut root = self.fuzz_dir().to_owned();
        root.push(crate::MOVE_TARGETS_DIR);
        root
    }

    fn rust_target_path(&self, target: &str) -> PathBuf {
        let mut root = self.fuzz_targets_dir();
        root.push(target);
        root.set_extension("rs");
        root
    }

    fn move_target_path(&self, target: &str) -> PathBuf {
        let mut root = self.move_targets_dir();
        root.push(target);
        root.set_extension("move");
        root
    }

    fn manifest(&self) -> Result<toml::Value> {
        let filename = self.manifest_path();
        let mut file = fs::File::open(&filename)
            .with_context(|| format!("could not read the manifest file: {}", filename.display()))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        toml::from_slice(&data).with_context(|| {
            format!(
                "could not decode the manifest file at {}",
                filename.display()
            )
        })
    }

    // If `fuzz_dir_opt` is `None`, returns a new instance with the default fuzz project
    // path. Otherwise, returns a new instance with the inner content of `fuzz_dir_opt`.
    fn manage_initial_instance(fuzz_dir_opt: Option<PathBuf>) -> Result<Self> {
        let project_dir = find_package()?;
        let fuzz_dir = if let Some(el) = fuzz_dir_opt {
            el
        } else {
            project_dir.join(DEFAULT_FUZZ_DIR)
        };
        Ok(FuzzProject {
            fuzz_dir,
            targets: Vec::new(),
        })
    }

    fn fuzz_dir_is_default_path(&self) -> bool {
        self.fuzz_dir.ends_with(DEFAULT_FUZZ_DIR)
    }
}

fn sysroot() -> Result<String> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc).arg("--print").arg("sysroot").output()?;
    // Note: We must trim() to remove the `\n` from the end of stdout
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn rustlib() -> Result<PathBuf> {
    let sysroot = sysroot()?;
    let mut pathbuf = PathBuf::from(sysroot);
    pathbuf.push("lib");
    pathbuf.push("rustlib");
    pathbuf.push(rustc_version::version_meta()?.host);
    pathbuf.push("bin");
    Ok(pathbuf)
}

fn collect_targets(value: &toml::Value) -> Vec<String> {
    let bins = value
        .as_table()
        .and_then(|v| v.get("bin"))
        .and_then(toml::Value::as_array);
    let mut bins = if let Some(bins) = bins {
        bins.iter()
            .map(|bin| {
                bin.as_table()
                    .and_then(|v| v.get("name"))
                    .and_then(toml::Value::as_str)
            })
            .filter_map(|name| name.map(String::from))
            .collect()
    } else {
        Vec::new()
    };
    // Always sort them, so that we have deterministic output.
    bins.sort();
    bins
}

#[derive(Debug)]
pub struct Manifest {
    crate_name: String,
    edition: Option<String>,
}

impl Manifest {
    pub fn parse() -> Result<Self> {
        let metadata = MetadataCommand::new().no_deps().exec()?;
        let package = metadata.packages.last().with_context(|| {
            anyhow!(
                "Expected to find at least one package in {}",
                metadata.target_directory
            )
        })?;
        let crate_name = package.name.clone();
        let edition = Some(String::from(package.edition.as_str()));

        Ok(Manifest {
            crate_name,
            edition,
        })
    }
}

fn is_fuzz_manifest(value: &toml::Value) -> bool {
    let is_fuzz = value
            .as_table()
            .and_then(|v| v.get("package"))
            .and_then(toml::Value::as_table)
            .and_then(|v| v.get("name"))
            .and_then(toml::Value::as_str)
            .map(|name| name.ends_with("fuzz"));
    is_fuzz == Some(true)
}

/// Returns the path for the first found non-fuzz Cargo package
fn find_package() -> Result<PathBuf> {
    let mut dir = env::current_dir()?;
    let mut data = Vec::new();
    loop {
        let manifest_path = dir.join("Cargo.toml");
        match fs::File::open(&manifest_path) {
            Err(_) => {}
            Ok(mut f) => {
                data.clear();
                f.read_to_end(&mut data)
                    .with_context(|| format!("failed to read {}", manifest_path.display()))?;
                let value: toml::Value = toml::from_slice(&data).with_context(|| {
                    format!(
                        "could not decode the manifest file at {}",
                        manifest_path.display()
                    )
                })?;
                if !is_fuzz_manifest(&value) {
                    // Not a cargo-fuzz project => must be a proper cargo project :)
                    return Ok(dir);
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }
    bail!("could not find a cargo project")
}

fn strip_current_dir_prefix(path: &Path) -> &Path {
    env::current_dir()
        .ok()
        .and_then(|curdir| path.strip_prefix(curdir).ok())
        .unwrap_or(path)
}
