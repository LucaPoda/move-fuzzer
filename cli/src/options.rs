pub mod add;
pub mod build;
pub mod cmin;
pub mod coverage;
pub mod fmt;
pub mod init;
pub mod list;
pub mod run;
pub mod tmin;

pub use self::{
    add::Add, build::Build, cmin::Cmin, coverage::Coverage, fmt::Fmt, init::Init,
    list::List, run::Run, tmin::Tmin,
};

use clap::*;
use std::str::FromStr;
use std::{fmt as stdfmt, path::PathBuf};
use std::fmt::Debug;
use move_package::BuildConfig;

#[derive(Clone, Debug, Eq, PartialEq, Parser)]
pub struct BuildOptions {
    /// Path to a package which the command should be run with respect to.
    #[clap(long = "path", short = 'p', global = true)]
    pub package_path: Option<PathBuf>,

    /// Print additional diagnostics if available.
    #[clap(short = 'v', global = true)]
    pub verbose: bool,

    #[clap(flatten)]
    pub target: Target,

    #[clap(flatten)] 
    /// move build options
    pub build_config: BuildConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Parser)]
#[command(group = clap::ArgGroup::new("target")
    .required(true)
    .args(&["target_name", "target_function"]))] // Define a mutually exclusive group
pub struct Target {
    #[clap(long)]
    pub target_module: Option<String>,
    
    #[clap(long, group = "target", requires = "target_module")]
    pub target_function: Option<String>,

    #[clap(long, group = "target")]
    pub target_name: Option<String>,
}

impl Target {
    pub fn get_module_name(&self) -> String {
        if let Some (module) = self.target_module.clone() {
            module
        }
        else {
            self.target_name.clone().expect("Module name or target name is required")
        }
    }

    pub fn get_target_function(&self) -> String {
        if let Some (fun) = self.target_function.clone() {
            fun
        }
        else {
            String::from("fuzz_target")
        }
    }

    pub fn get_command(&self) -> String {
        if let Some(target_name) = self.target_name.clone() {
            format!("--target '{target_name}")
        }
        else {
            let module = self.target_module.clone().expect("Module name is missing");
            let function = self.target_function.clone().expect("Target function is missing");

            format!("--target-module '{module}' --target-function '{function}")
        }
    }
}

impl FromStr for BuildOptions {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let build_options: Self = s.parse()?;
        Ok(build_options)
    }
}

impl std::fmt::Display for BuildOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.verbose {
            write!(f, " -v")?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Parser)]
pub struct FuzzDirWrapper {
    /// The path to the fuzz project directory.
    #[clap(long)]
    pub fuzz_dir: Option<PathBuf>,
}

impl stdfmt::Display for FuzzDirWrapper {
    fn fmt(&self, f: &mut stdfmt::Formatter) -> stdfmt::Result {
        if let Some(ref elem) = self.fuzz_dir {
            write!(f, " --fuzz-dir={}", elem.display())?;
        }

        Ok(())
    }
}

impl FromStr for FuzzDirWrapper {
    type Err = anyhow::Error; // Or any other error type you prefer

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse the string as a path
        let path = if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        };

        Ok(FuzzDirWrapper { fuzz_dir: path })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use clap::Parser;
    use std::path::PathBuf;
    use std::collections::BTreeMap;
    use move_core_types::account_address::AccountAddress;
    use move_compiler::command_line::LintFlag;

    #[test]
    fn display_build_options() {
        // Create default BuildOptions
        let default_build_options = BuildOptions {
            package_path: None,
            verbose: false,
            target: Target {
                target_module: None,
                target_function: None,
                target_name: None,
            },
            build_config: BuildConfig {
                dev_mode: false,
                test_mode: false,
                generate_docs: false,
                install_dir: None,
                force_recompilation: false,
                lock_file: None,
                fetch_deps_only: false,
                skip_fetch_latest_git_deps: false,
                default_flavor: None,
                default_edition: None,
                deps_as_root: false,
                silence_warnings: false,
                warnings_are_errors: false,
                additional_named_addresses: BTreeMap::new(),
                lint_flag: LintFlag::default(),
            },
        };

        let opts = vec![
            default_build_options.clone(),
            BuildOptions {
                package_path: Some(PathBuf::from("path/to/package")),
                ..default_build_options.clone()
            },
            BuildOptions {
                verbose: true,
                ..default_build_options.clone()
            },
            BuildOptions {
                target: Target {
                    target_module: Some(PathBuf::from("module_name")),
                    target_function: Some("target_function".to_string()),
                    ..default_build_options.target.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                target: Target {
                    target_name: Some("target_name".to_string()),
                    ..default_build_options.target.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    dev_mode: true,
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    test_mode: true,
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    generate_docs: true,
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    install_dir: Some(PathBuf::from("install/dir")),
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    force_recompilation: true,
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    fetch_deps_only: true,
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    skip_fetch_latest_git_deps: true,
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    default_flavor: Some(Flavor::default()),
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    default_edition: Some(Edition::default()),
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    deps_as_root: true,
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    silence_warnings: true,
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
            BuildOptions {
                build_config: BuildConfig {
                    warnings_are_errors: true,
                    ..default_build_options.build_config.clone()
                },
                ..default_build_options.clone()
            },
        ];

        for (i, case) in opts.iter().enumerate() {
            println!("{i}");
            println!("{:?}", case);
            println!();
            let case_str = format_build_options(case);
            println!("{:?}", case_str);
            println!();
            let parsed_case = BuildOptions::parse_from(case_str.split_whitespace());
            assert_eq!(case, &parsed_case);
        }
    }

    fn format_build_options(opts: &BuildOptions) -> String {
        let mut args = vec![];
        
        if let Some(path) = &opts.package_path {
            args.push(format!("--path {}", path.display()));
        }
        if opts.verbose {
            args.push("-v".to_string());
        }
        if let Some(module_name) = &opts.target.target_module {
            args.push(format!("--module_name {}", module_name.display()));
        }
        if let Some(target_function) = &opts.target.target_function {
            args.push(format!("--target_function {}", target_function));
        }
        if let Some(target_name) = &opts.target.target_name {
            args.push(format!("--target_name {}", target_name));
        }
        if opts.build_config.dev_mode {
            args.push("--dev".to_string());
        }
        if opts.build_config.test_mode {
            args.push("--test".to_string());
        }
        if opts.build_config.generate_docs {
            args.push("--doc".to_string());
        }
        if let Some(install_dir) = &opts.build_config.install_dir {
            args.push(format!("--install-dir {}", install_dir.display()));
        }
        if opts.build_config.force_recompilation {
            args.push("--force".to_string());
        }
        if opts.build_config.fetch_deps_only {
            args.push("--fetch-deps-only".to_string());
        }
        if opts.build_config.skip_fetch_latest_git_deps {
            args.push("--skip-fetch-latest-git-deps".to_string());
        }
        if let Some(default_flavor) = &opts.build_config.default_flavor {
            args.push(format!("--default-move-flavor {:?}", default_flavor));
        }
        if let Some(default_edition) = &opts.build_config.default_edition {
            args.push(format!("--default-move-edition {:?}", default_edition));
        }
        if opts.build_config.deps_as_root {
            args.push("--dependencies-are-root".to_string());
        }
        if opts.build_config.silence_warnings {
            args.push("--silence-warnings".to_string());
        }
        if opts.build_config.warnings_are_errors {
            args.push("--warnings-are-errors".to_string());
        }

        args.join(" ")
    }
}
