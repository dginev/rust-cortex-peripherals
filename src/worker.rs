// Copyright 2015 Deyan Ginev. See the LICENSE
// file at the top-level directory of this distribution.
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed
// except according to those terms.

//! base class automating dispatcher communication via ZMQ

use std::borrow::Cow;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::Deref;
use std::path::Path;
use std::thread;
use std::time::Duration;
use zmq::{Context, Message, Socket, SNDMORE};

/// Generic requirements for CorTeX workers
pub trait Worker: Clone + Send {
  /// Core processing method
  fn convert(&self, _: &Path) -> Result<File, Box<Error>>;
  /// Size of chunk for network communication, larger implies less IO, smaller implies less RAM use
  fn message_size(&self) -> usize;
  /// Name of the service, as registered in CorTeX
  fn get_service(&self) -> &str;
  /// URL to the CorTeX dispatcher
  fn get_source_address(&self) -> Cow<str>;
  /// URL to the CorTeX sink
  fn get_sink_address(&self) -> Cow<str>;
  /// Simultaneous threads used for one worker each
  fn pool_size(&self) -> usize {
    1
  }
  /// Sets a uniquely identifying string for this worker instance
  fn set_identity(&mut self, _identity: String) {
    unimplemented!()
  }
  /// Gets the uniquely identifying string of this worker instance
  fn get_identity(&self) -> &str {
    unimplemented!()
  }

  /// sets up the worker process, with as many threads as requested
  fn start(&mut self, limit: Option<usize>) -> Result<(), Box<Error>>
  where
    Self: 'static + Sized,
  {
    let hostname = hostname::get_hostname().unwrap_or_else(|| String::from("hostname"));
    match self.pool_size() {
      1 => {
        self.set_identity(format!("{}:engrafo:1", hostname));
        self.start_single(limit)
      }
      n => {
        let mut threads = Vec::new();
        for thread in 0..n {
          let thread_str = if thread < 10 {
            format!("0{}", thread)
          } else {
            thread.to_string()
          };
          let identity_single = format!("{}:engrafo:{}", hostname, thread_str);
          let mut thread_self: Self = self.clone();
          thread_self.set_identity(identity_single);
          threads.push(thread::spawn(move || {
            // TODO: Errors can not be shared between threads safely? What should be the robustness strategy here?
            thread_self.start_single(limit).unwrap();
          }));
        }
        for t in threads {
          t.join().unwrap();
        }
        Ok(())
      }
    }
  }
  /// main worker loop for a single thread, works in perpetuity or up to a specified `limit`
  fn start_single(&self, limit: Option<usize>) -> Result<(), Box<Error>> {
    let mut work_counter = 0;
    // Connect to a task ventilator
    let context_source = Context::new();
    let source = context_source.socket(zmq::DEALER).unwrap();
    source.set_identity(self.get_identity().as_bytes()).unwrap();

    assert!(source.connect(&self.get_source_address()).is_ok());
    // Connect to a task sink
    let context_sink = Context::new();
    let sink = context_sink.socket(zmq::PUSH).unwrap();
    assert!(sink.connect(&self.get_sink_address()).is_ok());
    // Work in perpetuity
    loop {
      let mut taskid_msg = Message::new();
      let mut recv_msg = Message::new();

      source.send(&self.get_service(), 0).unwrap();
      source.recv(&mut taskid_msg, 0).unwrap();
      let taskid = taskid_msg.as_str().unwrap();

      // Prepare a File for the input
      let input_filepath = env::temp_dir().to_str().unwrap().to_string() + "/" + taskid + ".zip";
      let mut file = File::create(input_filepath.clone()).unwrap();
      let mut input_size = 0;
      loop {
        source.recv(&mut recv_msg, 0).unwrap();

        if let Ok(written) = file.write(recv_msg.deref()) {
          input_size += written;
        }
        if !source.get_rcvmore().unwrap() {
          break;
        }
      }

      let file_result = if input_size > 0 {
        file.seek(SeekFrom::Start(0)).unwrap();
        self.convert(Path::new(&input_filepath))
      } else {
        Err(From::from("Input was empty.")) // No input, no conversion needed
      };

      self.respond_to_cortex(file_result, input_size, taskid, &sink);

      work_counter += 1;
      if let Some(upper_bound) = limit {
        if work_counter >= upper_bound {
          // Give enough time to complete the Final job.
          thread::sleep(Duration::new(1, 0));
          break;
        }
      }
    }
    Ok(())
  }

  /// Respond to the sink endpoint
  fn respond_to_cortex(
    &self,
    file_result: Result<File, Box<Error>>,
    input_size: usize,
    taskid: &str,
    sink: &Socket,
  ) {
    match file_result {
    Ok(mut converted_file) => {
      sink.send(self.get_identity(), SNDMORE).unwrap();
      sink.send(self.get_service(), SNDMORE).unwrap();
      sink.send(taskid, SNDMORE).unwrap();
      loop {
        // Stream converted data via zmq
        let message_size = self.message_size();
        let mut data = vec![0; message_size];
        let size = converted_file.read(&mut data).unwrap();
        data.truncate(size);
        if size < message_size {
          // If exhausted, send the last frame
          sink.send(&data, 0).unwrap();
          // And terminate
          break;
        } else {
          // If more to go, send the frame and indicate there's more to come
          sink.send(&data, SNDMORE).unwrap();
        }
      }
    },
    Err(e) => {
      // If there was nothing to do, retry a minute later
      // throttle in case there is a temporary local issue, such as running out of available RAM, etc.
      // but also to protect the server from DDoS-like behavior where we send broken requests at nauseam.
      if input_size == 0 {
        info!(
          target: &format!("Result:{}", self.get_identity()),
          "Nothing to return, empty input. Throttling for a minute."
        );
      } else {
        info!(
          target: &format!("Result:{}", self.get_identity()),
          "Nothing to return, conversion came back empty: {:?}. Throttling for a minute.", e
        );
        // Also, send an empty reply, so that cortex knows this is an aberrant task
        // ....
      }
      thread::sleep(Duration::new(60, 0));
    }
    }
  }
}

mod echo;
pub use echo::EchoWorker;

mod tex_to_html;
pub use tex_to_html::TexToHtmlWorker;

#[cfg(feature = "engrafo")]
mod engrafo;
#[cfg(feature = "engrafo")]
pub use engrafo::EngrafoWorker;
