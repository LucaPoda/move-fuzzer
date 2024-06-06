// Copyright 2016 rust-fuzz developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

#[macro_use]
mod templates;
pub mod options;
pub mod project;
mod utils;
use anyhow::Result;
use clap::{Args, Parser};
use move_package::BuildConfig;
use std::path::PathBuf;

use options::*;

// Template constants remain the same
static FUZZ_TARGETS_DIR_OLD: &str = "fuzzers";
static FUZZ_TARGETS_DIR: &str = "fuzz_targets";
static MOVE_TARGETS_DIR: &str = "sources";

// It turns out that `clap`'s `long_about()` makes `cargo fuzz --help`
// unreadable, and its `before_help()` injects our long about text before the
// version, so change the default template slightly.
const LONG_ABOUT_TEMPLATE: &str = "\
{bin} {version}
{about}

USAGE:
    {usage}

{before-help}

{all-args}

{after-help}";

const RUN_BEFORE_HELP: &str = "\
The fuzz target name is the same as the name of the fuzz target script in
fuzz/fuzz_targets/, i.e. the name picked when running `cargo fuzz add`.

This will run the script inside the fuzz target with varying inputs until it
finds a crash, at which point it will save the crash input to the artifact
directory, print some output, and exit. Unless you configure it otherwise (see
libFuzzer options below), this will run indefinitely.

By default fuzz targets are built with optimizations equivalent to
`cargo build --release`, but with debug assertions and overflow checks enabled.
Address Sanitizer is also enabled by default.";

const RUN_AFTER_HELP: &str = "\
A full list of libFuzzer options can be found at
http://llvm.org/docs/LibFuzzer.html#options

You can also get this by running `cargo fuzz run fuzz_target -- -help=1`

Some useful options (to be used as `cargo fuzz run fuzz_target -- <options>`)
include:

  * `-max_len=<len>`: Will limit the length of the input string to `<len>`

  * `-runs=<number>`: Will limit the number of tries (runs) before it gives up

  * `-max_total_time=<time>`: Will limit the amount of time (seconds) to
    fuzz before it gives up

  * `-timeout=<time>`: Will limit the amount of time (seconds) for a single
    run before it considers that run a failure

  * `-only_ascii`: Only provide ASCII input

  * `-dict=<file>`: Use a keyword dictionary from specified file. See
    http://llvm.org/docs/LibFuzzer.html#dictionaries\
";

const BUILD_BEFORE_HELP: &str = "\
By default fuzz targets are built with optimizations equivalent to
`cargo build --release`, but with debug assertions and overflow checks enabled.
Address Sanitizer is also enabled by default.";

const BUILD_AFTER_HELP: &str = "\
Sanitizers perform checks necessary for detecting bugs in unsafe code
at the cost of some performance. For more information on sanitizers see
https://doc.rust-lang.org/unstable-book/compiler-flags/sanitizer.html\
";
/// A trait for running our various commands.
trait RunCommand {
    /// Run this command!
    fn run_command(&mut self) -> Result<()>;
}

#[derive(Clone, Debug, Parser)]
#[clap(version, about)]
#[clap(subcommand_required = true)]
#[clap(arg_required_else_help = true)]
//#[clap(version_propagated = true)]
pub enum Fuzz {
    /// Initialize the fuzz directory
    Init(options::Init),

    /// Add a new fuzz target
    Add(options::Add),

    #[clap(
        long_about = LONG_ABOUT_TEMPLATE,
        before_help = BUILD_BEFORE_HELP,
        after_help = BUILD_AFTER_HELP
    )]
    /// Build fuzz targets
    Build(options::Build),

    /// Print the `std::fmt::Debug` output for an input
    Fmt(options::Fmt),

    /// List all the existing fuzz targets
    List(options::List),

    #[clap(
        long_about = LONG_ABOUT_TEMPLATE,
        before_help = RUN_BEFORE_HELP,
        after_help = RUN_AFTER_HELP
    )]
    /// Run a fuzz target
    Run(options::Run),

    /// Minify a corpus
    Cmin(options::Cmin),

    /// Minify a test case
    Tmin(options::Tmin),

    /// Run program on the generated corpus and generate coverage information
    Coverage(options::Coverage),
}

impl RunCommand for Fuzz {
    fn run_command(&mut self) -> Result<()> {
        match self {
            Fuzz::Init(x) => x.run_command(),
            Fuzz::Add(x) => x.run_command(),
            Fuzz::Build(x) => x.run_command(),
            Fuzz::List(x) => x.run_command(),
            Fuzz::Fmt(x) => x.run_command(),
            Fuzz::Run(x) => x.run_command(),
            Fuzz::Cmin(x) => x.run_command(),
            Fuzz::Tmin(x) => x.run_command(),
            Fuzz::Coverage(x) => x.run_command(),
        }
    }
}

use std::str::FromStr;

impl FromStr for Fuzz {
    type Err = String; // Replace with the actual error type

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "init" => Ok(Fuzz::Init(Init::parse())),
            "add" => Ok(Fuzz::Add(Add::parse())),
            "build" => Ok(Fuzz::Build(Build::parse())),
            "fmt" => Ok(Fuzz::Fmt(Fmt::parse())),
            "list" => Ok(Fuzz::List(List::parse())),
            "run" => Ok(Fuzz::Run(Run::parse())),
            "cmin" => Ok(Fuzz::Cmin(Cmin::parse())),
            "tmin" => Ok(Fuzz::Tmin(Tmin::parse())),
            "coverage" => Ok(Fuzz::Coverage(Coverage::parse())),
            _ => Err(format!("Unknown command: {}", s)),
        }
    }
}

impl Args for Fuzz {
    fn augment_args(mut cmd: clap::Command) -> clap::Command {
        match cmd.get_name().to_lowercase().as_str() {
            "init" => Init::augment_args(cmd),
            "add" => Add::augment_args(cmd),
            "build" => Build::augment_args(cmd),
            "fmt" => Fmt::augment_args(cmd),
            "list" => List::augment_args(cmd),
            "run" => Run::augment_args(cmd),
            "cmin" => Cmin::augment_args(cmd),
            "tmin" => Tmin::augment_args(cmd),
            "coverage" => Coverage::augment_args(cmd),
            _ => cmd, // Return unchanged command if unknown
        }
    }

    fn augment_args_for_update(mut cmd: clap::Command) -> clap::Command {
        match cmd.get_name().to_lowercase().as_str() {
            "init" => Init::augment_args_for_update(cmd),
            "add" => Add::augment_args_for_update(cmd),
            "build" => Build::augment_args_for_update(cmd),
            "fmt" => Fmt::augment_args_for_update(cmd),
            "list" => List::augment_args_for_update(cmd),
            "run" => Run::augment_args_for_update(cmd),
            "cmin" => Cmin::augment_args_for_update(cmd),
            "tmin" => Tmin::augment_args_for_update(cmd),
            "coverage" => Coverage::augment_args_for_update(cmd),
            _ => cmd, // Return unchanged command if unknown
        }
    }
}

impl Fuzz {
    pub fn execute(mut self) -> anyhow::Result<()> {
        self.run_command()
    }
}

fn main() {
    Fuzz::parse().execute();
}
