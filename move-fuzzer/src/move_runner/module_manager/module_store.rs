use move_binary_format::errors::VMError;
use move_binary_format::CompiledModule;

use move_core_types::account_address::AccountAddress;
use move_core_types::language_storage::ModuleId;
use move_core_types::language_storage::StructTag;
use move_core_types::resolver::LinkageResolver;
use move_core_types::resolver::ModuleResolver;
use move_core_types::resolver::ResourceResolver;

use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ModuleStore {
    modules: HashMap<ModuleId, Vec<u8>>,
}   

impl ModuleStore {
    pub fn new(root_module: CompiledModule) -> Self {
        let mut loader = Self {
            modules: HashMap::new(),
        };
        loader.add_module(root_module);
        loader
    }

    fn add_module(&mut self, compiled_module: CompiledModule) {
        let id = compiled_module.self_id();
        let mut bytes = vec![];
        compiled_module.serialize(&mut bytes).unwrap();
        self.modules.insert(id, bytes);
    }

    pub fn add_dependencies(&mut self, dependencies: &Vec<CompiledModule>) {
        for dep in dependencies {
            self.add_module(dep.clone()); 
        }
    }
}

impl LinkageResolver for ModuleStore {
    type Error = VMError;
}

impl ModuleResolver for ModuleStore {
    type Error = VMError;
    fn get_module(&self, module_id: &ModuleId) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.modules.get(module_id).cloned())
    }
}

impl ResourceResolver for ModuleStore {
    type Error = VMError;

    fn get_resource(
        &self,
        _address: &AccountAddress,
        _tag: &StructTag,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(None)
    }
}