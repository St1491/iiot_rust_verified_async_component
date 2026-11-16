use std::env;
use rust_verified_async_component::*;

#[cfg(creusot)]
fn main() {
}

#[cfg(not(creusot))]
fn main() { //the main function just collect the arguments and create the config via the parse method, then execute run
    let args: Vec<String> = env::args().collect();
    if args.len() >= 2{
        let config = match Config::parse(args){
            Ok(config ) => config,
            Err(e) => panic!("Wrong inputs,arguments: {}",e)
        };
        run(config);
    }
    else{
        direct_simulation();
    } 
}
