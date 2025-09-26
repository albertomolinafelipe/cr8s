use crate::controllers::{
    garbage_collector::GCController, replicaset::RSController, scheduler::Scheduler,
};

mod garbage_collector;
mod replicaset;
mod scheduler;

pub fn run(apiserver: String) {
    tokio::spawn(Scheduler::run(apiserver.clone()));
    tokio::spawn(GCController::run(apiserver.clone()));
    tokio::spawn(RSController::run(apiserver.clone()));
}
