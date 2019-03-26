// Copyright 2015 Deyan Ginev. See the LICENSE
// file at the top-level directory of this distribution.
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed
// except according to those terms.

//! base class automating dispatcher communication via ZMQ

use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::Deref;
use std::path::Path;
use std::thread;
use std::time::Duration;
use zmq::{Context, Message, SNDMORE};
// use std::str;
use std::process::Command;

/// Generic requirements for CorTeX workers
pub trait Worker: Clone + Send {
  /// Core processing method
  fn convert(&self, _: &Path) -> Option<File>;
  /// Size of chunk for network communication, larger implies less IO, smaller implies less RAM use
  fn message_size(&self) -> usize;
  /// Name of the service, as registered in CorTeX
  fn service(&self) -> String;
  /// URL to the CorTeX dispatcher
  fn source(&self) -> String;
  /// URL to the CorTeX sink
  fn sink(&self) -> String;
  /// Simultaneous threads used for one worker each
  fn pool_size(&self) -> usize {
    1
  }
  /// sets up the worker process, with as many threads as requested
  fn start(&self, limit: Option<usize>) -> Result<(), Box<Error>>
  where
    Self: 'static + Sized,
  {
    let hostname = hostname::get_hostname().unwrap_or_else(|| String::from("hostname"));
    match self.pool_size() {
      1 => self.start_single(format!("{}:engrafo:1", hostname), limit),
      n => {
        let mut threads = Vec::new();
        for thread in 0..n {
          let thread_str = if thread < 10 {
            format!("0{}", thread)
          } else {
            thread.to_string()
          };
          let identity_single = format!("{}:engrafo:{}", hostname, thread_str);
          let thread_self: Self = self.clone();
          threads.push(thread::spawn(move || {
            // TODO: Errors can not be shared between threads safely? What should be the robustness strategy here?
            thread_self.start_single(identity_single, limit).unwrap();
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
  fn start_single(&self, identity: String, limit: Option<usize>) -> Result<(), Box<Error>> {
    let mut work_counter = 0;
    // Connect to a task ventilator
    let context_source = Context::new();
    let source = context_source.socket(zmq::DEALER).unwrap();
    source.set_identity(identity.as_bytes()).unwrap();

    assert!(source.connect(&self.source()).is_ok());
    // Connect to a task sink
    let context_sink = Context::new();
    let sink = context_sink.socket(zmq::PUSH).unwrap();
    assert!(sink.connect(&self.sink()).is_ok());
    // Work in perpetuity
    loop {
      let mut taskid_msg = Message::new();
      let mut recv_msg = Message::new();

      source.send(&self.service(), 0).unwrap();
      source.recv(&mut taskid_msg, 0).unwrap();
      let taskid = taskid_msg.as_str().unwrap();

      // Prepare a File for the input
      let input_filepath = env::temp_dir().to_str().unwrap().to_string() + "/" + taskid + ".zip";
      let mut file = File::create(input_filepath.clone()).unwrap();
      loop {
        source.recv(&mut recv_msg, 0).unwrap();

        if let Ok(uwritten) = file.write(recv_msg.deref()) {
          if uwritten <= 0 {
            break;
          }
        } else {
          break;
        }
        if !source.get_rcvmore().unwrap() {
          break;
        }
      }

      file.seek(SeekFrom::Start(0)).unwrap();
      let file_opt = self.convert(Path::new(&input_filepath));
      if file_opt.is_some() {
        let mut converted_file = file_opt.unwrap();
        sink.send(&identity, SNDMORE).unwrap();
        sink.send(&self.service(), SNDMORE).unwrap();
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
      } else {
        // If there was nothing to do, retry a minute later
        // throttle in case there is a temporary local issue, such as running out of available RAM, etc.
        // but also to protect the server from DDoS-like behavior where we send broken requests at nauseam.
        println!("-- Nothing to return, possible problems? Throttling for a minute.");
        thread::sleep(Duration::new(60, 0));
        continue;
      }

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
}
/// An echo worker for testing
#[derive(Clone, Debug)]
pub struct EchoWorker {
  /// the usual
  pub service: String,
  /// the usual
  pub version: f32,
  /// the usual
  pub message_size: usize,
  /// the usual
  pub source: String,
  /// the usual
  pub sink: String,
}
impl Default for EchoWorker {
  fn default() -> EchoWorker {
    EchoWorker {
      service: "echo_service".to_string(),
      version: 0.1,
      message_size: 100_000,
      source: "tcp://127.0.0.1:51695".to_string(),
      sink: "tcp://127.0.0.1:51696".to_string(),
    }
  }
}
impl Worker for EchoWorker {
  fn service(&self) -> String {
    self.service.clone()
  }
  fn source(&self) -> String {
    self.source.clone()
  }
  fn sink(&self) -> String {
    self.sink.clone()
  }
  fn message_size(&self) -> usize {
    self.message_size
  }

  fn convert(&self, path: &Path) -> Option<File> {
    Some(File::open(path).unwrap())
  }
}
/// A TeX to HTML conversion worker
#[derive(Clone, Debug)]
pub struct TexToHtmlWorker {
  /// the usual
  pub service: String,
  /// the usual
  pub version: f32,
  /// the usual
  pub message_size: usize,
  /// the usual
  pub source: String,
  /// the usual
  pub sink: String,
}
impl Default for TexToHtmlWorker {
  fn default() -> TexToHtmlWorker {
    TexToHtmlWorker {
      service: "tex_to_html".to_string(),
      version: 0.1,
      message_size: 100_000,
      source: "tcp://127.0.0.1:51695".to_string(),
      sink: "tcp://127.0.0.1:51696".to_string(),
    }
  }
}
impl Worker for TexToHtmlWorker {
  fn service(&self) -> String {
    self.service.clone()
  }
  fn source(&self) -> String {
    self.source.clone()
  }
  fn sink(&self) -> String {
    self.sink.clone()
  }
  fn message_size(&self) -> usize {
    self.message_size
  }

  fn convert(&self, path: &Path) -> Option<File> {
    let name = path.file_stem().unwrap().to_str().unwrap();
    let destination_path = env::temp_dir().to_str().unwrap().to_string() + "/" + name + ".zip";
    // println!("Source {:?}", path);
    Command::new("latexmlc")
      .arg("--whatsin")
      .arg("archive")
      .arg("--whatsout")
      .arg("archive")
      .arg("--format")
      .arg("html5")
      .arg("--pmml")
      .arg("--cmml")
      .arg("--mathtex")
      .arg("--preload")
      .arg("[ids]latexml.sty")
      .arg("--nodefaultresources")
      .arg("--inputencoding")
      .arg("iso-8859-1")
      .arg("--timeout")
      .arg("300")
      .arg("--log")
      .arg("cortex.log")
      .arg("--destination")
      .arg(destination_path.clone())
      .arg(path.to_string_lossy().to_string())
      .output()
      .unwrap_or_else(|e| panic!("failed to execute process: {}", e));

    // println!("Dest: {:?}", destination_path);
    let file_test = File::open(destination_path.clone());
    match file_test {
      Ok(file) => Some(file),
      Err(_) => None,
    }
  }
}

#[cfg(feature = "engrafo")]
mod engrafo;
#[cfg(feature = "engrafo")]
pub use engrafo::EngrafoWorker;
