use std::fmt::Display;

use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use move_model::{model::{GlobalEnv, ModuleId as ModelModuleId, StructId}, symbol::SymbolPool, ty::{PrimitiveType, Type as MoveType}};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, EnumAsInner)]
pub enum FuzzerType {
    U8,
    U16,
    U32,
    U64,
    U128,
    U256,
    Bool,
    Vector(Box<FuzzerType>),
    Struct(Vec<FuzzerType>),
    Signer,
    Address,
}


impl From<FuzzerType> for MoveType {
    fn from(value: FuzzerType) -> Self {
        match value {
            FuzzerType::U8 => MoveType::Primitive(PrimitiveType::U8),
            FuzzerType::U16 => MoveType::Primitive(PrimitiveType::U16),
            FuzzerType::U32 => MoveType::Primitive(PrimitiveType::U32),
            FuzzerType::U64 => MoveType::Primitive(PrimitiveType::U64),
            FuzzerType::U128 => MoveType::Primitive(PrimitiveType::U128),
            FuzzerType::Bool => MoveType::Primitive(PrimitiveType::Bool),
            FuzzerType::Vector(t) => MoveType::Vector(Box::new(MoveType::from(*t))),
            FuzzerType::Struct(types) => MoveType::Struct(
                ModelModuleId::new(42),
                StructId::new(SymbolPool::new().make("")),
                types.into_iter().map(|t| MoveType::from(t)).collect_vec(),
            ),
            FuzzerType::U256 => MoveType::Primitive(PrimitiveType::U256),
            FuzzerType::Signer => MoveType::Primitive(PrimitiveType::Signer),
            FuzzerType::Address => MoveType::Primitive(PrimitiveType::Address),
        }
    }
}

impl FuzzerType {
    pub fn from(env: &GlobalEnv, value: MoveType) -> Self {
        match value {
            MoveType::Primitive(p) => match p {
                move_model::ty::PrimitiveType::Bool => FuzzerType::Bool,
                move_model::ty::PrimitiveType::U8 => FuzzerType::U8,
                move_model::ty::PrimitiveType::U16 => FuzzerType::U16,
                move_model::ty::PrimitiveType::U32 => FuzzerType::U32,
                move_model::ty::PrimitiveType::U64 => FuzzerType::U64,
                move_model::ty::PrimitiveType::U128 => FuzzerType::U128,
                move_model::ty::PrimitiveType::U256 => FuzzerType::U256,
                move_model::ty::PrimitiveType::Address => FuzzerType::Address,
                move_model::ty::PrimitiveType::Signer => FuzzerType::Signer,
                move_model::ty::PrimitiveType::Num => todo!(),
                move_model::ty::PrimitiveType::Range => todo!(),
                move_model::ty::PrimitiveType::EventStore => todo!(),
            },
            MoveType::Vector(vec) => {
                FuzzerType::Vector(Box::new(FuzzerType::from(env, *vec)))
            },
            MoveType::Struct(module_id, struct_id, _) => {
                let module_env = env.get_modules().find(|m| m.get_id() == module_id).unwrap();
                let struct_env = module_env.get_struct(struct_id);
                let fields = struct_env.get_fields().map(|f| f.get_type()).collect::<Vec<MoveType>>();
                FuzzerType::Struct(fields.into_iter().map(|t| FuzzerType::from(env, t)).collect_vec())
            }
            MoveType::Tuple(_) => todo!(),
            MoveType::TypeParameter(_) => todo!(),
            MoveType::Reference(_, _) => todo!(),
            MoveType::Fun(_, _) => todo!(),
            MoveType::TypeDomain(_) => todo!(),
            MoveType::ResourceDomain(_, _, _) => todo!(),
            MoveType::Error => todo!(),
            MoveType::Var(_) => todo!(),
        }
    }
}

impl Display for FuzzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FuzzerType::U8
            | FuzzerType::U16
            | FuzzerType::U32
            | FuzzerType::U64
            | FuzzerType::U128
            | FuzzerType::U256 
            | FuzzerType::Bool 
            | FuzzerType::Vector(_)
            | FuzzerType::Signer
            | FuzzerType::Address => write!(f, "{:?}", self),
            FuzzerType::Struct(types) => {
                if types.is_empty() {
                    write!(f, "Struct([])")
                } else {
                    write!(f, "Struct([ ").unwrap();
                    for (i, t) in types.iter().enumerate() {
                        eprintln!("{:?}", t);
                        write!(f, "{}", t).unwrap();
                        if i != types.len() - 1 {
                            write!(f, ", ").unwrap();
                        }
                    }
                    write!(f, " ])")
                }
            }
        }
    }
}

pub struct Parameters(pub Vec<FuzzerType>);

impl Display for Parameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            write!(f, "[]")
        } else {
            write!(f, "[ ").unwrap();
            for (i, v) in self.0.clone().iter().enumerate() {
                write!(f, "{}", v).unwrap();
                if i != self.0.len() - 1 {
                    write!(f, ", ").unwrap();
                }
            }
            write!(f, " ]")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Error {
    Abort { message: String },
    Runtime { message: String },
    OutOfBound { message: String },
    OutOfGas { message: String },
    ArithmeticError { message: String },
    MemoryLimitExceeded { message: String },
    Unknown { message: String },
    AccountAddressParseError { message: String }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Abort { message  } => write!(f, "Abort - {}", message),
            Error::OutOfBound { message: _ } => write!(f, "OutOfBound"),
            Error::OutOfGas { message: _ } => write!(f, "OutOfGas"),
            Error::ArithmeticError { message: _ } => write!(f, "ArithmeticError"),
            Error::MemoryLimitExceeded { message: _ } => write!(f, "MemoryLimitExceeded"),
            Error::Unknown { message } => write!(f, "Unknown - {}", message),
            Error::Runtime { message } => write!(f, "Runtime - {}", message),
            Error::AccountAddressParseError { message } => write!(f, "AccountAddressParseError - {}", message),
        }
    }
}