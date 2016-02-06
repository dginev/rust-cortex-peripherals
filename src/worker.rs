// Copyright 2015 Deyan Ginev. See the LICENSE
// file at the top-level directory of this distribution.
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed
// except according to those terms.

//! base class automating dispatcher communication via ZMQ

extern crate zmq;
extern crate rand;
extern crate tempfile;

use zmq::{Error, Message, Context, SNDMORE};
use std::ops::Deref;
use std::thread;
use std::time::Duration;
use std::env;
use std::fs::File;
use std::io::{Read,Write, Seek, SeekFrom};
use std::path::Path;
// use std::str;
use std::process::Command;

/// Generic requirements for CorTeX workers
pub trait Worker {
  /// Core processing method
  fn convert(&self, &Path) -> Option<File>;
  /// Size of chunk for network communication, larger implies less IO, smaller implies less RAM use
  fn message_size(&self) -> usize;
  /// Name of the service, as registered in CorTeX
  fn service(&self) -> String;
  /// URL to the CorTeX dispatcher
  fn source(&self) -> String;
  /// URL to the CorTeX sink
  fn sink(&self) -> String;
  /// main worker loop, works in perpetuity or up to a specified `limit`
  fn start(&self, limit : Option<usize>) -> Result<(), Error> {
    let mut work_counter = 0;
    // Connect to a task ventilator
    let mut context_source = Context::new();
    let mut source = context_source.socket(zmq::DEALER).unwrap();
    let identity : String = (0..10).map(|_| rand::random::<u8>() as char).collect();
    source.set_identity(identity.as_bytes()).unwrap();

    assert!(source.connect(&self.source()).is_ok());
    // Connect to a task sink
    let mut context_sink = Context::new();
    let mut sink = context_sink.socket(zmq::PUSH).unwrap();
    assert!(sink.connect(&self.sink()).is_ok());
    // Work in perpetuity
    loop {
      let mut taskid_msg = Message::new().unwrap();
      let mut recv_msg = Message::new().unwrap();

      source.send_str(&self.service(), 0).unwrap();
      source.recv(&mut taskid_msg, 0).unwrap();
      let taskid = taskid_msg.as_str().unwrap();

      // Prepare a File for the input
      let input_filepath = env::temp_dir().to_str().unwrap().to_string() + "/" + taskid + ".zip";
      let mut file = File::create(input_filepath.clone()).unwrap();
      loop {
        source.recv(&mut recv_msg, 0).unwrap();

        file.write(recv_msg.deref()).unwrap();
        if !source.get_rcvmore().unwrap() {
          break;
        }
      }

      file.seek(SeekFrom::Start(0)).unwrap();
      let file_opt = self.convert(Path::new(&input_filepath));
      if file_opt.is_some() {
        let mut converted_file = file_opt.unwrap();
        sink.send_str(&self.service(), SNDMORE).unwrap();
        sink.send_str(taskid, SNDMORE).unwrap();
        loop {
          // Stream converted data via zmq
          let message_size = self.message_size();
          let mut data = vec![0; message_size];
          let size = converted_file.read(&mut data).unwrap();
          data.truncate(size);
          if size < message_size {
            // If exhausted, send the last frame
            sink.send(&data,0).unwrap();
            // And terminate
            break;
          } else {
            // If more to go, send the frame and indicate there's more to come
            sink.send(&data,SNDMORE).unwrap();
          }
        }
      }
      else {
        // If there was nothing to do, retry a minute later
        thread::sleep(Duration::new(60,0));
        continue
      }

      work_counter += 1;
      match limit {
        Some(upper_bound) => {
          if work_counter >= upper_bound {
            // Give enough time to complete the Final job.
            thread::sleep(Duration::new(1,0));
            break;
          }
        },
        None => {}
      };
    }
    Ok(())
  }
}
/// An echo worker for testing
pub struct EchoWorker {
  /// the usual
  pub service : String,
  /// the usual
  pub version : f32,
  /// the usual
  pub message_size : usize,
  /// the usual
  pub source : String,
  /// the usual
  pub sink : String
}
impl Default for EchoWorker {
  fn default() -> EchoWorker {
    EchoWorker {
      service: "echo_service".to_string(),
      version: 0.1,
      message_size: 100000,
      source: "tcp://localhost:5555".to_string(),
      sink: "tcp://localhost:5556".to_string()
    }
  }
}
impl Worker for EchoWorker {
  fn service(&self) -> String {self.service.clone()}
  fn source(&self) -> String {self.source.clone()}
  fn sink(&self) -> String {self.sink.clone()}
  fn message_size(&self) -> usize {self.message_size.clone()}

  fn convert(&self, path : &Path) -> Option<File> {
    Some(File::open(path).unwrap())
  }
}
/// A TeX to HTML conversion worker
pub struct TexToHtmlWorker {
  /// the usual
  pub service : String,
  /// the usual
  pub version : f32,
  /// the usual
  pub message_size : usize,
  /// the usual
  pub source : String,
  /// the usual
  pub sink : String
}
impl Default for TexToHtmlWorker {
  fn default() -> TexToHtmlWorker {
    TexToHtmlWorker {
      service: "tex_to_html".to_string(),
      version: 0.1,
      message_size: 100000,
      source: "tcp://localhost:5555".to_string(),
      sink: "tcp://localhost:5556".to_string()
    }
  }
}
impl Worker for TexToHtmlWorker {
  fn service(&self) -> String {self.service.clone()}
  fn source(&self) -> String {self.source.clone()}
  fn sink(&self) -> String {self.sink.clone()}
  fn message_size(&self) -> usize {self.message_size.clone()}

  fn convert(&self, path : &Path) -> Option<File> {
    let name = path.file_stem().unwrap().to_str().unwrap();
    let destination_path = env::temp_dir().to_str().unwrap().to_string() + "/" +name+ ".zip";
    println!("Source {:?}", path);
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
      .arg(path.clone())
      .output()
      .unwrap_or_else(|e| { panic!("failed to execute process: {}", e) });

    println!("Dest: {:?}", destination_path);
    let file_test = File::open(destination_path.clone());
    match file_test {
      Ok(file) => Some(file),
      Err(_) => None
    }
  }
}
