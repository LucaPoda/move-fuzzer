use std::fs::File;
use std::io::Read;

use move_binary_format::file_format::{FunctionDefinitionIndex, StructDefinitionIndex};
use move_binary_format::CompiledModule;use move_model::addr_to_big_uint;
use move_model::ast::ModuleName;
use move_model::model::FunId;
use move_model::model::FunctionData;
use move_model::model::GlobalEnv;
use move_model::model::Loc;
use move_model::model::ModuleData;
use move_model::model::ModuleId as ModelModuleId;
use move_model::model::StructId;
use move_model::ty::Type as MoveType;
use move_bytecode_utils::Modules;

use crate::move_runner::types::FuzzerType;

/// From https://github.com/kunalabs-io/sui-client-gen
pub fn add_modules_to_model<'a>(
    env: &mut GlobalEnv,
    modules: impl IntoIterator<Item = &'a CompiledModule>,
) {
    for (i, m) in modules.into_iter().enumerate() {
        let id = m.self_id();
        let addr = addr_to_big_uint(id.address());
        let module_name = ModuleName::new(addr, env.symbol_pool().make(id.name().as_str()));
        let module_id = ModelModuleId::new(i);
        let mut module_data = ModuleData::stub(module_name.clone(), module_id, m.clone());

        // add functions
        for (i, def) in m.function_defs().iter().enumerate() {
            let def_idx = FunctionDefinitionIndex(i as u16);
            let name = m.identifier_at(m.function_handle_at(def.function).name);
            let symbol = env.symbol_pool().make(name.as_str());
            let fun_id = FunId::new(symbol);
            let data = FunctionData::stub(symbol, def_idx, def.function);
            module_data.function_data.insert(fun_id, data);
            module_data.function_idx_to_id.insert(def_idx, fun_id);
        }

        // add structs
        for (i, def) in m.struct_defs().iter().enumerate() {
            let def_idx = StructDefinitionIndex(i as u16);
            let name = m.identifier_at(m.struct_handle_at(def.struct_handle).name);
            let symbol = env.symbol_pool().make(name.as_str());
            let struct_id = StructId::new(symbol);
            let data =
                env.create_move_struct_data(m, def_idx, symbol, Loc::default(), Vec::default());
            module_data.struct_data.insert(struct_id, data);
            module_data.struct_idx_to_id.insert(def_idx, struct_id);
        }

        env.module_data.push(module_data);
    }
}

pub fn generate_abi_from_bin(
    modules: Vec<CompiledModule>,
    module_name: &str,
    function_name: &str,
) -> (Vec<FuzzerType>, usize) {
    let params;
    let max_coverage;

    let module_map = Modules::new(modules.iter());
    let dep_graph = module_map.compute_dependency_graph();
    let topo_order = dep_graph.compute_topological_order().unwrap();

    let mut env = GlobalEnv::new();
    add_modules_to_model(&mut env, topo_order);

    let module_env = env.get_modules().find(|m| m.matches_name(module_name));
    if let Some(env) = module_env {

        let func = env
            .get_functions()
            .find(|f| f.get_name_str() == function_name);
        if let Some(f) = func {
            max_coverage = f.get_bytecode().len();
            params = f.get_parameter_types();
        } else {
            panic!("Could not find target function !");
        }
    } else {
        panic!("Could not find target module !")
    }
    println!("ABI generation completed...");
    (transform_params(&env, params), max_coverage)
}

pub fn load_compiled_module(path: &str) -> CompiledModule {
    let mut f = File::open(path).unwrap();
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer).unwrap();
    CompiledModule::deserialize_with_defaults(&buffer).unwrap()
}

fn transform_params(env: &GlobalEnv, params: Vec<MoveType>) -> Vec<FuzzerType> {
    let mut res = vec![];
    for param in params {
        res.push(FuzzerType::from(env, param));
    }
    res
}