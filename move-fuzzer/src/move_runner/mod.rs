use std::fmt::Debug;

use arbitrary::Unstructured;

use move_binary_format::errors::VMResult;
use move_binary_format::CompiledModule;
use move_core_types::account_address::AccountAddress;
use move_core_types::identifier::IdentStr;
use move_core_types::runtime_value::serialize_values;
use move_core_types::runtime_value::MoveValue;
use move_core_types::vm_status::StatusCode;
use move_vm_config::runtime::VMConfig;
use move_vm_runtime::move_vm::MoveVM;
use move_vm_types::gas::UnmeteredGasMeter;

mod utils;
use crate::move_runner::utils::generate_abi_from_bin;

mod types;
use crate::move_runner::types::FuzzerType as FuzzerType;
use crate::move_runner::types::Error;

mod arbitrary_inputs;
use crate::move_runner::arbitrary_inputs::arbitrary_inputs;

mod module_manager;
use self::module_manager::module_loader::ModuleLoader;
use self::module_manager::module_store::ModuleStore;

fn combine_signers_and_args(
    signers: Vec<AccountAddress>,
    non_signer_args: Vec<Vec<u8>>,
) -> Vec<Vec<u8>> {
    signers
        .into_iter()
        .map(|s| MoveValue::Signer(s).simple_serialize().unwrap())
        .chain(non_signer_args)
        .collect()
}


/// todo
#[derive(Debug, Clone)]
pub struct TargetFunction {
    name: String,
    args: Vec<FuzzerType>,
    // type_args: Option<Vec<FuzzerType>> // todo: capire se si possono implementare i type arguments
}

/// todo
pub struct MoveRunner {
    move_vm: MoveVM,
    module: CompiledModule,
    dependencies: Vec<CompiledModule>,
    target_module: String,
    target_function: TargetFunction,
    max_coverage: usize,
}

impl Debug for MoveRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SuiRunner").field("module", &self.module).field("target_module", &self.target_module).field("target_function", &self.target_function).field("max_coverage", &self.max_coverage).finish()
    }
}

impl MoveRunner {
    /// todo
    pub fn new(module_path: &str, target_module: &str, target_function: &str) -> Self {
        let move_vm = MoveVM::new_with_config(vec![], VMConfig::default()).unwrap();
        // Loading compiled module
        let mut module_loader = ModuleLoader::new(String::from(module_path));
        module_loader.load_depencencies();

        let params = generate_abi_from_bin(module_loader.get_all(), target_module, target_function);
        MoveRunner {
            move_vm,
            module: module_loader.get_module(),
            dependencies: module_loader.get_dependencies(),
            target_module: String::from(target_module),
            target_function: TargetFunction {
                name: String::from(target_function),
                args: params.0,
                //type_args: None,
            },
            max_coverage: params.1,
        }
    }

    // todo: capire se il coverage che c'è adesso funziona uguale
    // fn create_coverage(inputs: Vec<FuzzerType>, cov: Vec<u16>) -> Coverage {
    //     let mut coverage_data = vec![];
    //     for c in cov {
    //         coverage_data.push(CoverageData { pc: c as u64 });
    //     }
    //     Coverage {
    //         inputs,
    //         data: coverage_data,
    //     }
    // }

    fn get_target_parameters(&self) -> Vec<FuzzerType> {
        self.target_function.args.clone()
    }

    /// todo
    pub fn execute(
        &mut self,
        bytes: &[u8]
    ) -> Result<Option<()>, (Option<()>, Error)> {
        let inputs = self.get_target_parameters();
        let mut remote_view = ModuleStore::new(self.module.clone());
        remote_view.add_dependencies(&self.dependencies);
        let mut session = self.move_vm.new_session(&remote_view);

        let ty_args = vec![]
            .into_iter()
            .map(|tag| session.load_type(&tag))
            .collect::<VMResult<_>>()
            .unwrap();

        let mut data = Unstructured::new(bytes);
        let result = session.execute_function_bypass_visibility(
            &self.module.self_id(),
            IdentStr::new(&self.target_function.name).unwrap(),
            ty_args,
            combine_signers_and_args(vec![], serialize_values(&arbitrary_inputs(inputs.clone(), &mut data))),
            &mut UnmeteredGasMeter
        );

        match result {
            Ok(_values) => Ok(Some(())),
            Err(err) => {
                println!("{:?}", err);
                let mut message = String::from("");
                if let Some(m) = err.message() {
                    message = m.to_string();
                }
                let error = match err.major_status() {
                    StatusCode::ABORTED => Error::Abort { message },
                    StatusCode::ARITHMETIC_ERROR => Error::ArithmeticError { message },
                    StatusCode::MEMORY_LIMIT_EXCEEDED => Error::MemoryLimitExceeded { message },
                    StatusCode::OUT_OF_GAS => Error::OutOfGas { message },
                    _ => Error::Unknown { message },
                };
                Err((Some(()), error))
            }
        }
    } 
}