#![no_main]

use move_fuzzer::MOVE_RUNNER;
use move_fuzzer::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    // data generation logic goes here
    let mut runner = MOVE_RUNNER.get().unwrap().lock().unwrap();
    let res = (*runner).execute(bytes);
    if let Err(e) = res {
        println!("{:?}", e.1);
        std::process::abort();
    }
});
