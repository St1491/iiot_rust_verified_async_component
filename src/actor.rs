use std::fmt::Debug;
use std::marker::PhantomData;
use std::time::Duration;
use std::error::Error;
use std::io;
use std::sync::{Arc,Mutex};
use std::result::Result;
use trpl;
use super::*;


///A State representing a new actor not yet initialized
/// An idle actor cannot act nor stop, its integrity must be checked before any other transition
#[derive(Clone,Debug)]
pub struct Idle{}
///A State representing an actor whose integrity has already been checked and is waiting to act
/// Rechecking its intergity does nothing
/// Acting change the state to Acting
/// Stopping cannot be performed since it's doing nothing yet
#[derive(Clone,Debug)]
pub struct Initialized{}
///State representing an actor while it is working
/// Rechecking its intergity does nothing
/// Stopping change the state to Initialized
/// Acting cannot be performed since it's already acting
#[derive(Clone,Debug)]
pub struct Acting{}
///State representing an actor that encountered an error while being initialized
/// It cannot act to garantee safety nor stop since it can never be acting
/// Its integrity can be rechecked but nothing will change 
#[derive(Clone,Debug)]
pub struct Broken{}

///Simulates the actual internal check of the actor, this is currently a placeholder that always returns true
pub fn debug_check() -> bool{
    true
}


///The struct that represent the actor itself, it contains an id to identify the actor and depends on the state the actor is currently in
#[derive(Debug,Clone)]
pub struct Actor<S>{
    id: String,
    _phantom : PhantomData<S>,
}
impl Actor<Idle>{
    ///Spawn a new Actor with chosen id and the initial state Idle
    pub fn new(id: String) -> Actor<Idle>{
        Actor {
            id,
            _phantom: PhantomData,
        }
    }
    ///Write the actor as string
    pub fn to_string(&self) -> String{
        return format!("Actor ID: {}\n State: Idle", self.id);
    }
    ///Simulate an integrity check for the actor, necessary to use an actor
    pub fn check_integrity(self,check:bool) -> Result<Actor<Initialized>,Actor<Broken>>{
        if check{
            Ok(Actor {
                id: self.id,
                _phantom: PhantomData,
            })
        }
        else {
            Err(Actor {
                id: self.id,
                _phantom: PhantomData,
            })
        }
    } 
}
impl Actor<Initialized>{
    ///Simulate the actor starting to work, only works with initialized actors
    pub fn act(self) -> Actor<Acting>{
        Actor {
            id: self.id,
            _phantom: PhantomData,
        }
    }
    ///Write the actor as string
    pub fn to_string(&self) -> String{
        return format!("Actor ID: {}\n State: Initialized", self.id);
    }
}
impl Actor<Acting>{
    ///Simulate the actor ending its work, only works with acting actors
    pub fn stop(self) -> Actor<Initialized>{
        Actor {
            id: self.id,
            _phantom: PhantomData,
        }
    }
    ///Write the actor as string
    pub fn to_string(&self) -> String{
        return format!("Actor ID: {}\n State: Acting", self.id);
    }
}
impl Actor<Broken>{
    ///Write the actor as string
    pub fn to_string(&self) -> String{
        return format!("Actor ID: {}\n State: Broken", self.id);
    }
}
impl <S> Actor<S>{
    ///Lock one of the shared resources for the use of an actor
    pub fn reserve_resource(&self, shared_resource:Arc<Mutex<u32>>) -> bool{
        let mut n = shared_resource.lock().unwrap();
        if *n > 0 {
            *n = n.saturating_sub(1);
            return true;
        }
        return false;
    }
    ///Unlock the resource after the actor finished its work
    pub fn free_resource(&self, shared_resource:Arc<Mutex<u32>>, max:u32){
        let mut n = shared_resource.lock().unwrap();
        assert!(*n < max, "double-free");
        *n += 1;
    }

}

///Simulate a possible execution for the actor (can only be used on Idle Actors)
pub async fn run_one(id:String,shared_resource: Arc<Mutex<u32>>, config:Config) -> Result<(),Box<dyn Error>>{
    let actor= Actor::new(id);
    match actor.check_integrity(debug_check()){                                                             //Initiliaze the actor
        Ok(mut initialized) => {
            for _ in 0..config.numbers_of_retry{                                                            //Repeat for the numbers of retry        
                if initialized.reserve_resource(Arc::clone(&shared_resource)){        //Try to lock the resource
                    let acting = initialized.act();                                          //Start the actor
                    trpl::sleep(Duration::from_secs(config.process_duration as u64)).await;                 //Simulate the work as a sleep
                    initialized = acting.stop();                                                            //Stop the actor
                    initialized.free_resource(Arc::clone(&shared_resource),config.shared_resources);   //frees the resource
                    println!("'{}' ran for {}s",initialized.id,config.process_duration);
                    return Ok(())                                                                           //End the simulation without errors    
                }
                trpl::sleep(Duration::from_secs(config.wait_before_retry as u64)).await;                    //Wait before retrying
            }
            return Err(Box::new(io::Error::new(io::ErrorKind::TimedOut,format!("actor {} could not act",initialized.id))))    //return error as the actor could not act
        },
        Err(broken)  => return Err (Box::new(io::Error::new(io::ErrorKind::Unsupported,format!("actor {} is not safe and cannot act",broken.id)))),
        //The actor is broken and so it can't do anything (should never be reachable since debug_check is always true)
    };
}

