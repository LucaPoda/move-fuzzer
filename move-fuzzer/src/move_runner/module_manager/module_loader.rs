use std::path::Path;

use move_binary_format::CompiledModule;
use move_command_line_common::files::MOVE_COMPILED_EXTENSION;
use walkdir::WalkDir;

use crate::move_runner::utils::load_compiled_module;

pub struct ModuleLoader {
    module_path: String,
    module: CompiledModule,
    dependencies: Vec<CompiledModule>
}

impl ModuleLoader {
    pub fn new(module_path: String) -> Self {
        let module = load_compiled_module(module_path.as_str());
        ModuleLoader {
            module_path,
            module,
            dependencies: vec![],
        }
    }

    fn get_root_dir(&self) -> &Path {
        Path::new(self.module_path.as_str()).parent().unwrap()
    }

    pub fn load_depencencies(&mut self) {
        // Iterate over all entries in the directory recursively
        for entry in WalkDir::new(self.get_root_dir()).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && path != Path::new(self.module_path.as_str()) {
                // Check if the file is a Move compiled module
                if let Some(ext) = path.extension() {
                    if ext == MOVE_COMPILED_EXTENSION{
                        self.dependencies.push(load_compiled_module(path.to_str().unwrap()));
                    }
                }
            }
        }
    }

    pub fn get_module(&self) -> CompiledModule {
        self.module.clone()
    }

    pub fn get_dependencies(&self) -> Vec<CompiledModule> {
        self.dependencies.clone()
    }

    pub fn get_all(&self) -> Vec<CompiledModule> {
        let mut res = self.get_dependencies();
        res.insert(0, self.get_module());
        res
    }
}