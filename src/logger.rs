//! This module is a trimmed-down copy of rtx_core::util::logger,
//! which is still waiting to get released as a crate...
//! maybe there is a simple logger crate that achieves this exact behavior?
use ansi_term::Colour::{Green, Red, White, Yellow};
use ansi_term::Style;
use chrono::Local;
use log::max_level;
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};

struct RtxLogger;
static LOGGER: RtxLogger = RtxLogger;

/// Convenient printing to STDERR (with \n)
#[macro_export]
macro_rules! println_stderr(
    ($($arg:tt)*) => ({
      use std::io::Write;
      match writeln!(&mut ::std::io::stderr(), $($arg)* ) {
        Ok(_) => {},
        Err(x) => panic!("Unable to write to stderr: {}", x),
      }
    })
);

/// Convenient printing to STDERR
#[macro_export]
macro_rules! print_stderr(
    ($($arg:tt)*) => ({
      use std::io::Write;
      match write!(&mut ::std::io::stderr(), $($arg)* ) {
        Ok(_) => {},
        Err(x) => panic!("Unable to write to stderr: {}", x),
      }
    })
);

impl log::Log for RtxLogger {
  fn enabled(&self, metadata: &Metadata) -> bool {
    metadata.level() <= max_level()
  }

  fn log(&self, record: &Record) {
    if self.enabled(record.metadata()) {
      let record_target = record.target();
      let details = record.args();
      let category_object = if record_target.is_empty() {
        "" // "unknown:unknown" ???
      } else {
        record_target
      };
      // Following the reporting syntax at: http://dlmf.nist.gov/LaTeXML/manual/errorcodes/
      // let severity = if category_object.starts_with("Fatal:") {
      //   ""
      // } else {
      //   match record.level() {
      //     Level::Info => "Info",
      //     Level::Warn => "Warn",
      //     Level::Error => "Error",
      //     Level::Debug => "Debug",
      //     Level::Trace => "Trace",
      //   }
      // };

      let message = format!("{}\t", category_object);

      let painted_message = match record.level() {
        Level::Info => Green.paint(message),
        Level::Warn => Yellow.paint(message),
        Level::Error => Red.paint(message),
        Level::Debug => Style::default().paint(message),
        _ => White.paint(message),
      }
      .to_string()
        + &details.to_string();

      println_stderr!(
        "\r[{}] {}",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        painted_message
      );
    }
  }

  fn flush(&self) {}
}

/// Initialize the logger with an appropriate level of verbosity
pub fn init(level: LevelFilter) -> Result<(), SetLoggerError> {
  log::set_logger(&LOGGER).unwrap();
  log::set_max_level(level);
  Ok(())
}
