//! Define an adapter to implement `std::io::Write` on `Sender<Bytes>`.
use actix_web::web::{Bytes, BytesMut};
use futures::channel::mpsc::Sender;
use futures::executor::block_on;
use futures::sink::SinkExt;
use std::io::{Error, ErrorKind, Result, Write};

/// Adapter to implement the `std::io::Write` trait on a `Sender<Bytes>` from a futures channel.
///
/// It uses an intermediate buffer to transfer packets.
pub struct Pipe {
    // Wrapping the sender in `Wait` makes it blocking, so we can implement blocking-style
    // io::Write over the async-style Sender.
    dest: Sender<std::result::Result<Bytes, ()>>,
    bytes: BytesMut,
}

impl Pipe {
    /// Wrap the given sender in a `Pipe`.
    pub fn new(destination: Sender<std::result::Result<Bytes, ()>>) -> Self {
        Pipe {
            dest: destination,
            bytes: BytesMut::new(),
        }
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        // This is the correct thing to do, but is not super important since the `Sink`
        // implementation of `Sender` just returns `Ok` without doing anything else.
        let _ = block_on(self.dest.close());
    }
}

impl Write for Pipe {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        // We are given a slice of bytes we do not own, so we must start by copying it.
        self.bytes.extend_from_slice(buf);

        // Then, take the buffer and send it in the channel.
        block_on(self.dest.send(Ok(self.bytes.split().into())))
            .map_err(|e| Error::new(ErrorKind::UnexpectedEof, e))?;

        // Return how much we sent - all of it.
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        block_on(self.dest.flush()).map_err(|e| Error::new(ErrorKind::UnexpectedEof, e))
    }
}
