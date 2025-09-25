use crate::controllers::{
    garbage_collector::GCController, replicaset::RSController, scheduler::Scheduler,
};

mod garbage_collector;
mod replicaset;
mod scheduler;

pub async fn run() {
    let _ = tokio::try_join!(
        tokio::spawn(Scheduler::run()),
        tokio::spawn(GCController::run()),
        tokio::spawn(RSController::run()),
    );
}
