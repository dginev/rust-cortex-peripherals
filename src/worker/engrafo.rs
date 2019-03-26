#![cfg(feature = "engrafo")]
// Copyright 2015 Deyan Ginev. See the LICENSE
// file at the top-level directory of this distribution.
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed
// except according to those terms.

//! base class automating dispatcher communication via ZMQ

use std::fs::File;
use std::io::{Error, Write};
use std::path::Path;
use std::process::Command;
use tempdir::TempDir;

use super::Worker;
use crate::adaptor;

/// An echo worker for testing
pub struct EngrafoWorker {
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
impl Default for EngrafoWorker {
  fn default() -> EngrafoWorker {
    EngrafoWorker {
      service: "engrafo".to_string(),
      version: 2.0,
      message_size: 100_000,
      source: "tcp://127.0.0.1:51695".to_string(),
      sink: "tcp://127.0.0.1:51696".to_string(),
    }
  }
}

impl Worker for EngrafoWorker {
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
    match self.convert_result(path) {
      Ok(file) => Some(file),
      Err(e) => {
        println!("Error encountered while converting: {:?}", e);
        None
      }
    }
  }
}

impl EngrafoWorker {
  fn convert_result(&self, path: &Path) -> Result<File, Error> {
    let input_tmpdir = adaptor::extract_zip_to_tmpdir(path, "engrafo_input")?;
    let unpacked_dir_path = input_tmpdir.path().to_str().unwrap().to_string() + "/";
    let destination_tmpdir = TempDir::new("engrafo_output").unwrap();
    let destination_dir_path = destination_tmpdir.path().to_str().unwrap();

    let cmd_result = Command::new("docker")
      .arg("run")
      .arg("-v")
      .arg("/tmp:/tmp")
      .arg("-w")
      .arg("/tmp")
      .arg("arxivvanity/engrafo:2.0.0")
      .arg("engrafo")
      .arg(unpacked_dir_path)
      .arg(destination_dir_path)
      .output()
      .expect("failed to execute process engrafo docker process.");

    // Package the output -- cortex requires a single ZIP return,
    // with all logging information stored in a "cortex.log" file at the ZIP's root.

    let log_name = format!("{}/cortex.log", destination_dir_path);
    let cortex_log_path = Path::new(&log_name);
    {
      // write log file and close it before archiving.
      let mut log_file = File::create(&cortex_log_path)?;
      log_file.write_all(&cmd_result.stderr)?;
      log_file.write_all(&cmd_result.stdout)?;
    }

    // cleanup
    // By closing the `TempDir` explicitly, we can check that it has
    // been deleted successfully. If we don't close it explicitly,
    // the directory will still be deleted when `tmp_dir` goes out
    // of scope, but we won't know whether deleting the directory
    // succeeded.
    input_tmpdir.close().unwrap();

    adaptor::archive_tmpdir_to_zip(destination_tmpdir)
  }
}
