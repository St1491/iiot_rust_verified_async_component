use std::fmt::Debug;
use std::time::Duration;
use std::error::Error;
use std::io;
use std::sync::{Arc,Mutex};
use std::result::Result;
use trpl;
use super::*;

///State helps to make the actors work as a FSM. 
/// All possible state is represented as a different struct that implemets State
/// All the transitions are represented as the method for each struct
trait State : Debug{
    fn act(&self) -> Result<Box<dyn State>,io::Error>;
    fn stop(&self) -> Result<Box<dyn State>,io::Error>;
    fn check_integrity(&self) -> Box<dyn State>;
    fn to_string(&self)->String;
}

///A State representing a new actor not yet initialized
/// An idle actor cannot act nor stop, its integrity must be checked before any other transition
#[derive(Debug)]
struct Idle{}
impl Idle{
    ///The function actually doing the check to ensure integrity of the actor (currently just for example it olny returns true)
    fn check() -> bool {
        //return random_bool(0.9); 
        return true
    }
}
impl State for Idle{
    fn act(&self) -> Result<Box<dyn State>,io::Error> {
        return Err(io::Error::new(std::io::ErrorKind::Unsupported, "Actor not yet Initialized"));
    }
    fn stop(&self) -> Result<Box<dyn State>,io::Error> {
        return Err(io::Error::new(std::io::ErrorKind::Unsupported, "Not Acting actors cannot be stopped"));
    }
    fn check_integrity(&self) -> Box<dyn State> {
        if Idle::check() {
            return Box::new(Initialized {})
        }
        return Box::new(Broken {})
    }
    fn to_string(&self)->String {
        return "Idle".to_string();
    }
}

///A State representing an actor whose integrity has already been checked and is waiting to act
/// Rechecking its intergity does nothing
/// Acting change the state to Acting
/// Stopping cannot be performed since it's doing nothing yet
#[derive(Debug)]
struct Initialized{}
impl State for Initialized{
    fn act(&self) -> Result<Box<dyn State>,io::Error>{
        Ok(Box::new(Acting {}))
    }
    fn stop(&self) -> Result<Box<dyn State>,io::Error> {
        return Err(io::Error::new(std::io::ErrorKind::Unsupported, "Not Acting actors cannot be stopped"));
    }
    fn check_integrity(&self) -> Box<dyn State> {
        Box::new(Initialized {})
    }
    fn to_string(&self)->String {
        return "Initialized".to_string();
    }
}

///State representing an actor while it is working
/// Rechecking its intergity does nothing
/// Stopping change the state to Initialized
/// Acting cannot be performed since it's already acting
#[derive(Debug)]
struct Acting{}
impl State for Acting{
    fn act(&self) -> Result<Box<dyn State>,io::Error> {
        return Err(io::Error::new(std::io::ErrorKind::ResourceBusy, "Actor already in use"));
    }
    fn stop(&self) -> Result<Box<dyn State>,io::Error> {
        return Ok(Box::new(Initialized {}));
    }
    fn check_integrity(&self) -> Box<dyn State> {
        Box::new(Acting {})
    }
    fn to_string(&self)->String {
        return "Acting".to_string();
    }
}

///State representing an actor that encountered an error while being initialized
/// It cannot act to garantee safety nor stop since it can never be acting
/// Its integrity can be rechecked but nothing will change 
#[derive(Debug)]
struct Broken{}
impl State for Broken{
    fn act(&self) -> Result<Box<dyn State>,io::Error> {
        return Err(io::Error::new(std::io::ErrorKind::ConnectionAborted, "Actor's integrity is compromised and cannot operate"));
    }
    fn stop(&self) -> Result<Box<dyn State>,io::Error> {
        return Err(io::Error::new(std::io::ErrorKind::Unsupported, "Not Acting actors cannot be stopped"));
    }
    fn check_integrity(&self) -> Box<dyn State> {
        Box::new(Broken {})
    }
    fn to_string(&self)->String {
        return "Broken".to_string();
    }
}

///The struct that represent the actor itself, it contains an id to identify the actor and the state the actor is currently in
#[derive(Debug)]
pub struct Actor{
    id: String,
    state: Option<Box<dyn State>>,
}
impl Actor{
    ///Spawn a new Actor with chosen id and the initial state Idle
    pub fn new(id: String) -> Actor{
        Actor{
            id,
            state: Some(Box::new(Idle{})),
        }

    }
    ///Write the actor as string
    pub fn to_string(&mut self) -> String{
        if let Some(s) =  self.state.take() {
            let string = s.to_string();
            self.state = Some(s);
            return format!("Actor ID: {}\n State: {}", self.id, string);
        }
        panic!("Unknown state or state is unreachable");
    }
    ///Simulate an integrity check for the actor, necessary to use an actor
    pub fn check_integrity(&mut self){
        if let Some(s) = self.state.take() {
            self.state = Some(s.check_integrity())
        }
    } 
    ///Simulate the actor starting to work, only works with initialized actors, raises errors otherwise
    pub fn act(&mut self) -> Result<(),Box<dyn Error>>{
        if let Some(s) = self.state.take() {
            match s.act() {
                Ok(new_state) => {
                    self.state = Some(new_state);
                    return Ok(());
                }
                Err(e) => {
                    self.state = Some(s);
                    return Err(Box::new(e));
                }
            }
        }
        //Should never execute
        return Err(Box::new(io::Error::new(io::ErrorKind::InvalidData,"Unknown State or State not found")))
    }
    ///Simulate the actor ending its work, only works with acting actos, raises errors otherwise
    pub fn stop(&mut self)-> Result<(),Box<dyn Error>>{
        if let Some(s) = self.state.take() {
            match s.stop() {
                Ok(new_state) => {
                    self.state = Some(new_state);
                    return Ok(());
                }
                Err(e) => {
                    self.state = Some(s);
                    return Err(Box::new(e));
                }
            }
        }
        //Should never execute
        return Err(Box::new(io::Error::new(io::ErrorKind::InvalidData,"Unknown State or State not found")))
    }
    ///Lock one of the shared resources for the use of an actor
    pub fn reserve_resource(&self, shared_resource:Arc<Mutex<u32>>) -> bool{
        let mut n = shared_resource.lock().unwrap();
        if *n > 0 {
            *n -= 1;
            return true;
        }
        return false;
    }
    ///Unlock the resource after the actor finished its work
    pub fn free_resource(&self, shared_resource:Arc<Mutex<u32>>){
        let mut n = shared_resource.lock().unwrap();
        *n += 1;
    }
    ///Simulate a possible execution for the actor
    pub async fn run(mut self, shared_resource: Arc<Mutex<u32>>, config:Config) -> Result<(),Box<dyn Error>>{
        self.check_integrity();                                                                         //Initiliaze the actor
        for _ in 0..config.numbers_of_retry{                                                            //Repeat for the numbers of retry        
            if self.reserve_resource(Arc::clone(&shared_resource)){               //Try to lock the resource
                match self.act(){                                                                       //Try to act (should always work in the example)
                    Ok(_) => {                  
                        trpl::sleep(Duration::from_secs(config.process_duration as u64)).await;         //Simulate the work as a sleep
                        if let Err(e) = self.stop(){                                    //Try to stop the actor (should always work in the example)
                            self.free_resource(Arc::clone(&shared_resource));
                            return Err(e)                   
                        }                             
                        self.free_resource(Arc::clone(&shared_resource));         //Free the resource after the work
                        println!("'{}' ran for {}s",self.id,config.process_duration);
                        return Ok(())                                                                   //End the simulation without errors
                    }
                    Err(e) => {                                                
                        self.free_resource(Arc::clone(&shared_resource));         //Free the resource in case of an error (should never run in the example)
                        return Err(e)                                                                   //Pass the error to the caller (should never run in the example)
                    }
                }
                
            }
            trpl::sleep(Duration::from_secs(config.wait_before_retry as u64)).await;                    //Wait before retrying
        }
        return Err(Box::new(io::Error::new(io::ErrorKind::TimedOut,format!("actor {} could not act",self.id))))    //return error as the actor could not act   
    }

}
