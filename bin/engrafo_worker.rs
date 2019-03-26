use pericortex::worker::{EngrafoWorker, Worker};

fn main() {
  // Let's get a minimal ZMQ ventilator/sink pair to test the worker
  // Start up an echo worker
  let worker = EngrafoWorker::default();
  // Perform a single echo task
  worker.start(None).expect("Worker started successfully.");
}
