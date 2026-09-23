//! The footer's end of the socket.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixStream;

use anyhow::{Context as _, Result};

use crate::wire::{FromDaemon, SOCKET, ToDaemon};

/// A connection to the daemon: one half reads what it says, the other asks.
#[derive(Debug)]
pub struct Link {
    stream: UnixStream,
}

impl Link {
    /// Connects.
    ///
    /// # Errors
    ///
    /// The daemon is not running — Together mode, or it failed.
    pub fn open() -> Result<(Self, Messages)> {
        let stream =
            UnixStream::connect(SOCKET).with_context(|| format!("connecting to {SOCKET}"))?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok((Self { stream }, Messages { reader }))
    }

    /// Asks one thing.
    ///
    /// # Errors
    ///
    /// The daemon went away.
    pub fn ask(&mut self, ask: &ToDaemon) -> Result<()> {
        let mut line = serde_json::to_string(ask)?;
        line.push('\n');
        self.stream
            .write_all(line.as_bytes())
            .context("asking the keys daemon")
    }
}

/// What the daemon says, one message at a time — blocking, for a thread.
#[derive(Debug)]
pub struct Messages {
    reader: BufReader<UnixStream>,
}

impl Iterator for Messages {
    type Item = FromDaemon;

    fn next(&mut self) -> Option<FromDaemon> {
        let mut line = String::new();
        loop {
            line.clear();
            if self.reader.read_line(&mut line).ok()? == 0 {
                return None;
            }
            if let Ok(said) = serde_json::from_str(line.trim()) {
                return Some(said);
            }
        }
    }
}
