extern crate pericortex;
extern crate zmq;
use std::thread;
use zmq::{SNDMORE};
use pericortex::worker::{EchoWorker, Worker};

#[test]
fn mock_round_trip() {
  // Let's get a minimal ZMQ ventilator/sink pair to test the worker
  let test_payload = "cortex peripherals - echo worker test".to_string();
  let sink_test_payload = test_payload.clone();
  let vent_thread = thread::spawn(move || {
    let ventilator_context = zmq::Context::new();
    let ventilator = ventilator_context.socket(zmq::ROUTER).unwrap();
    let ventilator_address = "tcp://*:5555";
    assert!(ventilator.bind(&ventilator_address).is_ok());

    // We expect one request
    let mut msg = zmq::Message::new().unwrap();
    let mut identity = zmq::Message::new().unwrap();
    ventilator.recv(&mut identity, 0).unwrap();
    ventilator.recv(&mut msg, 0).unwrap();
    let service_name = msg.as_str().unwrap().to_string();
    assert!(service_name == "echo_service");

    ventilator.send_msg(identity, SNDMORE).unwrap();
    ventilator.send_str("1", SNDMORE).unwrap();
    ventilator.send_str(&test_payload, 0).unwrap();
  });

  let sink_thread = thread::spawn(move || {
    let sink_context = zmq::Context::new();
    let sink = sink_context.socket(zmq::PULL).unwrap();
    let sink_address = "tcp://*:5556";
    assert!(sink.bind(&sink_address).is_ok());

    let mut service_msg = zmq::Message::new().unwrap();
    sink.recv(&mut service_msg, 0).unwrap();
    let service_name = service_msg.as_str().unwrap();
    assert!(service_name == "echo_service");

    let mut taskid_msg = zmq::Message::new().unwrap();
    sink.recv(&mut taskid_msg, 0).unwrap();
    let taskid_str = taskid_msg.as_str().unwrap();
    assert!(taskid_str == "1");

    let mut recv_msg = zmq::Message::new().unwrap();
    sink.recv(&mut recv_msg, 0).unwrap();
    let recv_payload = recv_msg.as_str().unwrap();
    assert!(recv_payload == sink_test_payload);
  });

  // Start up an echo worker
  let worker = EchoWorker::default();
  // Perform a single echo task
  assert!(worker.start(Some(1)).is_ok());

  assert!(vent_thread.join().is_ok());
  assert!(sink_thread.join().is_ok());
}
