use std::mem;

use arbitrary::{Unstructured, Arbitrary, Result as ArbitraryResult};

use move_core_types::account_address::{AccountAddress, AccountAddressParseError};
use move_core_types::runtime_value::{MoveStruct, MoveValue};
use move_core_types::u256::U256 as MoveU256;

use super::types::{FuzzerType, Error};

struct ArbitraryIter<'a, 'b> {
    u: &'b mut Unstructured<'a>,
    t: FuzzerType
}

impl<'a, 'b> Iterator for ArbitraryIter<'a, 'b> {
    type Item = ArbitraryResult<Result<MoveValue, Error>>;
    fn next(&mut self) -> Option<ArbitraryResult<Result<MoveValue, Error>>> {
        let keep_going = self.u.arbitrary().unwrap_or(false);
        if keep_going {
            Some(arbitrary_input(self.t.clone(), self.u))
        } else {
            None
        }
    }
}

fn arbitrary_iter<'a, 'b>(u: &'b mut Unstructured<'a>, fuzzer_type: FuzzerType) -> ArbitraryResult<ArbitraryIter<'a, 'b>> {
    Ok(ArbitraryIter {
        u,
        t: fuzzer_type,
    })
}

fn arbitrary_vec<'a, 'b>(u: &'b mut Unstructured<'a>, fuzzer_type: FuzzerType) -> ArbitraryResult<Result<MoveValue, Error>> {
    Ok(Ok(MoveValue::Vector(arbitrary_iter(u, fuzzer_type)?.map(|x| x.unwrap().unwrap()).collect()))) // todo: capire se si possono levare gli unwrap
}

fn arbitrary_u256(u: &mut Unstructured) -> ArbitraryResult<MoveU256> {
    let mut buf = [0; mem::size_of::<MoveU256>()];
    u.fill_buffer(&mut buf)?;
    Ok(MoveU256::from_le_bytes(&buf))
}

fn arbitrary_account(u: &mut Unstructured) -> ArbitraryResult<Result<AccountAddress, AccountAddressParseError>> {
    let mut buf = [0; mem::size_of::<AccountAddress>()];
    u.fill_buffer(&mut buf)?;
    Ok(AccountAddress::from_bytes(&buf))
}

fn arbitrary_address(u: &mut Unstructured) -> ArbitraryResult<Result<MoveValue, Error>> {
    let res = match arbitrary_account(u)? {
        Ok(account) => Ok(MoveValue::Address(account)),
        Err(e) => Err(Error::AccountAddressParseError { message: e.to_string() }),
    };
    Ok(res)
}

fn arbitrary_signer(u: &mut Unstructured) -> ArbitraryResult<Result<MoveValue, Error>> {
    let res = match arbitrary_account(u)? {
        Ok(account) => Ok(MoveValue::Signer(account)),
        Err(e) => Err(Error::AccountAddressParseError { message: e.to_string() }),
    };
    Ok(res)
}

fn arbitrary_input(input: FuzzerType, data: &mut arbitrary::Unstructured) -> ArbitraryResult<Result<MoveValue, Error>> {
    match input {
        FuzzerType::Bool => Ok(Ok(MoveValue::Bool(<bool as Arbitrary>::arbitrary(data)?))),
        FuzzerType::U8 => Ok(Ok(MoveValue::U8(<u8 as Arbitrary>::arbitrary(data)?))),
        FuzzerType::U16 => Ok(Ok(MoveValue::U16(<u16 as Arbitrary>::arbitrary(data)?))),
        FuzzerType::U32 => Ok(Ok(MoveValue::U32(<u32 as Arbitrary>::arbitrary(data)?))),
        FuzzerType::U64 => Ok(Ok(MoveValue::U64(<u64 as Arbitrary>::arbitrary(data)?))),
        FuzzerType::U128 => Ok(Ok(MoveValue::U128(<u128 as Arbitrary>::arbitrary(data)?))),
        FuzzerType::U256 => Ok(Ok(MoveValue::U256(arbitrary_u256(data)?))),
        FuzzerType::Vector(t) => Ok(arbitrary_vec(data, *t)?),
        FuzzerType::Struct(values) => Ok(Ok(MoveValue::Struct(MoveStruct(arbitrary_inputs(values, data))))),
        FuzzerType::Address => Ok(arbitrary_address(data)?),
        FuzzerType::Signer => Ok(arbitrary_signer(data)?),
    }
}

/// todo
pub fn arbitrary_inputs(inputs: Vec<FuzzerType>, data: &mut arbitrary::Unstructured) -> Vec<MoveValue> {
    let mut res = vec![];
    for input in inputs {
        let arbitrary_result = arbitrary_input(input, data);
        match arbitrary_result {
            Ok(parse_result) => {
                match parse_result {
                    Ok(value) => res.push(value),
                    Err(e) => eprintln!("{}", e), // todo: abort or not?
                }
            }
            Err(e) => eprintln!("{}", e),
        }
    }
    println!("{:?}", res);
    res
}

