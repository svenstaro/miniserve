use bytes::{Bytes, BytesMut};
use futures::sink::{Sink, Wait};
use futures::sync::mpsc::Sender;
use std::io::{Error, ErrorKind, Result, Write};

pub struct Pipe {
    dest: Wait<Sender<Bytes>>,
    bytes: BytesMut,
}

impl Pipe {
    pub fn new(destination: Sender<Bytes>) -> Self {
        Pipe {
            dest: destination.wait(),
            bytes: BytesMut::new(),
        }
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        let _ = self.dest.close();
    }
}

impl Write for Pipe {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.bytes.extend_from_slice(buf);
        match self.dest.send(self.bytes.take().into()) {
            Ok(_) => Ok(buf.len()),
            Err(e) => Err(Error::new(ErrorKind::UnexpectedEof, e)),
        }
    }

    fn flush(&mut self) -> Result<()> {
        self.dest
            .flush()
            .map_err(|e| Error::new(ErrorKind::UnexpectedEof, e))
    }
}
