macro_rules! move_toml_template {
    () => {
        format_args!(
            r##"[package]
name = "fuzz"
version = "0.0.0"
edition = "legacy"

[dependencies]
MoveStdlib = {{ git = "https://github.com/move-language/move-sui.git", subdir = "crates/move-stdlib", rev = "main" }}
MoveNursery = {{ git = "https://github.com/move-language/move-sui.git", subdir = "crates/move-stdlib/nursery", rev = "main" }}

[addresses]
std =  "0x1"
fuzz = "0x0"
"##
        )
    };
}

macro_rules! gitignore_template {
    () => {
        format_args!(
            r##"target
corpus
artifacts
coverage
"##
        )
    };
}

macro_rules! move_target_template {
    ($target_name:expr) => {
        format_args!(
            r##"module fuzz::{target_name} {{
    public fun fuzz_target(bytes: vector<u8>) {{
        
    }}
}}
"##,
target_name = $target_name
        )
    };
}
