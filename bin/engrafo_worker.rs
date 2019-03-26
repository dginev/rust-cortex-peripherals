#![cfg(feature = "engrafo")]
use pericortex::worker::{EngrafoWorker, Worker};

/// Start working on an Engrafo task for a given CorTeX endpoint
fn main() {
  let worker = EngrafoWorker::default();
  worker.start(None).expect("Worker started successfully.");
}
