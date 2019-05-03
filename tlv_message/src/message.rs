use std::io::prelude::*;
use std::io;
use std::io::{Error, ErrorKind};
use std::io::Read;

use byteorder::{NetworkEndian, ByteOrder, ReadBytesExt, WriteBytesExt};
use std::net::TcpStream;

// TODO: If message will implement read trait directly, it will be really easy to compose it with stream (and iterator)
#[derive(Debug)]
pub struct Message {
    message_type : u16,
    length : u32,
    data : Vec<u8>
}

pub enum AsyncResult {
    Ready(io::Result<()>),
    NotReady(usize),
}

pub trait AsyncRead: std::io::Read {
    fn read_async(&mut self, buf: &mut [u8]) -> AsyncResult {
        match self.read(buf) {
            Ok(bytes) if bytes == 0 => AsyncResult::Ready(Ok(())),
            Ok(bytes) => AsyncResult::NotReady(bytes),
            Err(ref error) if error.kind() == std::io::ErrorKind::Interrupted => AsyncResult::NotReady(0),
            Err(error) => AsyncResult::Ready(Err(error))
        }
    }
}

//TODO: Ideally we want to wrap TcpStream and force it to be nonblock
impl AsyncRead for TcpStream { }

impl Message {
    pub fn new(message_type : u16, length : u32, data : Vec<u8>) -> Message {
        Message {
            message_type,
            length,
            data
        }
    }

    pub fn message_type(&self) -> u16 {
        self.message_type
    }

    pub fn length(&self) -> u32 {
        self.length
    }

    pub fn data(&self) -> &[u8] {
        &self.data[..]
    }

    pub fn from_reader<T : Read>(reader: &mut T) -> io::Result<Self> {
        let mut message = Message {
            message_type : 0,
            length : 0,
            data : Vec::new()
        };

        //TODO: We shouldn't enforce the endianess at this level. we can't assume we are reading from network
        message.message_type = reader.read_u16::<NetworkEndian>()?;
        message.length = reader.read_u32::<NetworkEndian>()?;
        message.data = vec![0; message.length as usize];

        let mut total_bytes_read = 0;

        while total_bytes_read < message.length as usize {
            total_bytes_read += reader.read(&mut message.data[total_bytes_read..]).or_else(|err| {
                if err.kind() == std::io::ErrorKind::Interrupted {
                    Ok(0)
                } else {
                    Err(err)
                }
            })?;
        };

        Ok(message)
    }

    pub fn into_writer<T : Write>(mut self, writer: &mut T) -> io::Result<()> {

        //TODO: We shouldn't enforce the endianess at this level. we can't assume we are reading from network
        writer.write_u16::<NetworkEndian>(self.message_type)?;
        writer.write_u32::<NetworkEndian>(self.length)?;

        let mut total_bytes_written = 0;

        //TODO: Handle a case of 0 bytes written (not in interrupted error case)
        while total_bytes_written < self.length as usize {
            total_bytes_written += writer.write(&mut self.data[total_bytes_written..]).or_else(|err| {
                if err.kind() == std::io::ErrorKind::Interrupted {
                    Ok(0)
                } else {
                    Err(err)
                }
            })?;
        };

        Ok(())

    }

    pub fn read_async<T : AsyncRead>(&mut self, reader : &mut T) -> AsyncResult {
        if self.message_type == 0 {
            let mut buffer: [u8; 2] = [0; 2];

            match reader.read_async(&mut buffer[self.data.len()..]) {
                AsyncResult::Ready(Err(e)) => return AsyncResult::Ready(Err(e)),
                AsyncResult::Ready(ref result) => {
                    if self.data.len() != 0 {
                        buffer.copy_from_slice(&self.data[..self.data.len()]);
                    }
                    self.message_type = NetworkEndian::read_u16(&buffer[..]);
                },
                AsyncResult::NotReady(bytes_read) if 0 < bytes_read => {
                    // We read only part of the message type, we have to store the part we read, but we don't have where
                    // Therefore we hack what we have, the message data, and store it temporarily there
                    let total_read = self.data.len() + bytes_read;
                    self.data.resize(total_read, 0);
                    self.data.copy_from_slice(&buffer[(total_read - bytes_read)..total_read]);
                    return AsyncResult::NotReady(bytes_read);
                },
                AsyncResult::NotReady(bytes_read) => return AsyncResult::NotReady(bytes_read),
            }

            // If we are here, we read the message type successfully, so we have to clear anything we temporarly store in the data
            self.data.clear();
        };
        if self.length == 0 {
            let mut buffer: [u8; 4] = [0; 4];

            match reader.read_async(&mut buffer[self.data.len()..]) {
                AsyncResult::Ready(Err(e)) => return AsyncResult::Ready(Err(e)),
                AsyncResult::Ready(_) => {
                    if self.data.len() != 0 {
                        buffer.copy_from_slice(&self.data[..self.data.len()]);
                    }
                    self.length = NetworkEndian::read_u32(&buffer[..]);
                },
                AsyncResult::NotReady(bytes_read) if 0 < bytes_read => {
                    // We read only part of the message type, we have to store the part we read, but we don't have where
                    // Therefore we hack what we have, the message data, and store it temporarily there
                    let total_read = self.data.len() + bytes_read;
                    self.data.resize(total_read, 0);
                    self.data.copy_from_slice(&buffer[(total_read - bytes_read)..total_read]);
                    return AsyncResult::NotReady(bytes_read);
                },
                AsyncResult::NotReady(bytes_read) => return AsyncResult::NotReady(bytes_read),
            }

            // If we are here, we read the message type successfully, so we have to clear anything we temporarily store in the data
            self.data.clear();
            // Resize the vector to the new length:
            self.data.resize(self.length as usize, 0);
            // Yet another ugly hack: We now need a way to count how many bytes we actually read into the data.
            // As we have the total bytes we need to read in the vector length, we can abuse the length field in order to do the book keeping
            self.length = 0;
        };

        match reader.read_async(&mut self.data[(self.length as usize)..]) {
            AsyncResult::Ready(Err(e)) => return AsyncResult::Ready(Err(e)),
            AsyncResult::Ready(_) => {
                // Check we have read all the data
                if self.data.len() != self.length as usize {
                    return AsyncResult::Ready(Err(Error::new(ErrorKind::UnexpectedEof, "Expected to read more bytes for message")))
                } else {
                    return AsyncResult::Ready(Ok(()))
                };
            },
            AsyncResult::NotReady(bytes_read) => {
                self.length += bytes_read as u32; // This is safe as the buffer length is limited to this size
                // Find where to save how many bytes we have read
                return AsyncResult::NotReady(bytes_read)
            },
        }
        AsyncResult::Ready(Ok(()))
    }

}