use std::io::prelude::*;
use std::io;
use std::io::Read;
use std::borrow::Borrow;
use std::string::String;
use byteorder::{NetworkEndian, ByteOrder, ReadBytesExt, WriteBytesExt, NativeEndian};
use std::net::TcpStream;
use crate::message::Async::{NotReady, Ready};

//TODO: We assume we read/write data in network endianness, we should be able to support reading/writing it from/to native endianness as well
// TODO: If message will implement read trait directly, it will be really easy to compose it with stream (and iterator)
// TODO: We should be able to return not_ready for write. The client should be able to continue on partial message.
// TODO: For some weird reason, the reader won't read if there is only a small number of bytes in the stream.
#[derive(Default, Debug)]
pub struct Message {
    message_type : [u8;2],
    length : [u8;4],
    data : Vec<u8>
}

pub enum Async<T> {
    Ready(T),
    NotReady
}

pub enum AsyncResult<T,E> {
    Ok(T),
    Err(E)
}

pub trait ByteBuffer {
    fn get_data(&mut self, location : usize) -> &mut [u8];
    fn get_storage(&mut self, location : usize) -> &mut [u8];
}

pub trait AsyncRead: std::io::Read {
    fn partial_read_async(&mut self, buf: &mut [u8]) -> AsyncResult<Async<usize>, io::Error> {
        match self.read(buf) {
            Ok(bytes) => AsyncResult::Ok(Async::Ready(bytes)),
            Err(ref error) if error.kind() == std::io::ErrorKind::Interrupted => AsyncResult::Ok(Async::NotReady),
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => AsyncResult::Ok(Async::NotReady),
            Err(error) => AsyncResult::Err(error)
        }
    }
}

pub trait AsyncWrite: std::io::Write {
    fn partial_write_async(&mut self, buf: &mut [u8]) -> AsyncResult<Async<usize>, io::Error> {
        match self.write(buf) {
            Ok(bytes) => AsyncResult::Ok(Async::Ready(bytes)),
            Err(ref error) if error.kind() == std::io::ErrorKind::Interrupted => AsyncResult::Ok(Async::NotReady),
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => AsyncResult::Ok(Async::NotReady),
            Err(error) => AsyncResult::Err(error)
        }
    }
}

// AsyncReader/AsyncWriter tries to be as thread safe as possible.
// The read/write itself are as safe as T and underlying writer/reader allows it to be.
// In practice all functions but async_write/async_read will be thread safe
pub struct AsyncReader<T : ByteBuffer + Default> {
    buffer : T,
    bytes_read : usize,
    done : bool,
    ready : bool,
    error : Option<io::Error>
}

pub struct AsyncWriter<T : ByteBuffer + Default> {
    buffer : T,
    bytes_written : usize,
    done : bool,
    ready: bool,
    error : Option<io::Error>
}

pub enum AsyncReadResult<T>
    where T : ByteBuffer + Default {
    Ready(T),
    NotReady(AsyncReader<T>)
}

pub enum AsyncWriteResult<T>
    where T : ByteBuffer + Default {
    Ready,
    NotReady(AsyncWriter<T>)
}

impl<T : ByteBuffer + Default> AsyncReader<T> {
    pub fn new() -> AsyncReader<T> {
        AsyncReader {
            buffer : T::default(),
            bytes_read : 0,
            done : false,
            ready : false,
            error : None
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    // Calling finish on unready object will panic
    // Should return an enum: T if ready, self if not ready, error if error
    pub fn finish(mut self) -> io::Result<AsyncReadResult<T>> {
        if !self.done {
            panic!("Called finish on message, but reading is not done");
        }

        if let Some(e) = self.error {
            return Err(e)
        }

        if self.ready {
            return Ok(AsyncReadResult::Ready(self.buffer))
        } else {
            // Clear the done status for the next run
            self.done = false;
            return Ok(AsyncReadResult::NotReady(self))
        }

    }

    pub fn async_read<R : AsyncRead>(&mut self, reader : &mut R) {
        let buffer = self.buffer.get_storage(self.bytes_read);
        if buffer.is_empty() {
            self.ready = true;
            self.done = true;
        } else {
            match reader.partial_read_async(buffer) {
                AsyncResult::Ok(Async::NotReady) => {
                    self.done = true;
                },
                AsyncResult::Ok(Async::Ready(0)) => {
                    self.done = true;
                    self.ready = true;
                },
                AsyncResult::Ok(Async::Ready(bytes)) => self.bytes_read += bytes,
                AsyncResult::Err(error) => {
                    self.error = Some(error);
                    self.done = true;
                }
            }
        }
    }
}

impl<T : ByteBuffer + Default> AsyncWriter<T> {
    pub fn new(buffer : T) -> AsyncWriter<T> {
        AsyncWriter {
            buffer,
            bytes_written : 0,
            done : false,
            ready: false,
            error : None
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    // Calling finish on unready object will panic
    pub fn finish(mut self) -> io::Result<AsyncWriteResult<T>> {
        if !self.done {
            panic!("Called finish on message, but writing is not done");
        }

        if let Some(e) = self.error {
            return Err(e)
        }

        if self.ready {
            return Ok(AsyncWriteResult::Ready)
        } else {
            // Clear the done status for the next run
            self.done = false;
            return Ok(AsyncWriteResult::NotReady(self))
        }
    }

    pub fn async_write<W : AsyncWrite>(&mut self, writer : &mut W) {
        let mut buffer = self.buffer.get_data(self.bytes_written);
        match writer.partial_write_async(buffer) {
            AsyncResult::Ok(Async::NotReady) => {
                self.done = true;
            },
            AsyncResult::Ok(Async::Ready(0)) => {
                self.done = true;
                self.ready = true;
            },
            AsyncResult::Ok(Async::Ready(bytes)) => self.bytes_written += bytes,
            AsyncResult::Err(error) => {
                self.error = Some(error);
                self.done = true;
            }
        }
    }
}

//TODO: Ideally we want to wrap TcpStream and force it to be nonblock
impl AsyncRead for TcpStream { }
impl AsyncWrite for TcpStream { }

impl ByteBuffer for Message {
    fn get_data(&mut self, location : usize) -> &mut [u8] {
        match location {
            0...1 => {
                &mut self.message_type[location..]
            },
            2...5 => {
                &mut self.length[(location - 2)..]
            },
            _ => {
                if location - 6 > self.length() as usize {
                    &mut []
                } else {
                    &mut self.data[(location - 6)..]
                }
            }
        }
    }

    fn get_storage(&mut self, location : usize) -> &mut [u8] {
        match location {
            0...1 => {
                &mut self.message_type[location..]
            },
            2...5 => {
                &mut self.length[(location - 2)..]
            },
            _ => {
                if location - 6 > self.length() as usize {
                    &mut[]
                } else {
                    &mut self.data[(location - 6)..]
                }
            }
        }
    }
}

//TODO: Create helper function for internal read/write (types/length/data)
impl Message {
    pub fn new(message_type: u16, length: u32, data: Vec<u8>) -> Message {
        let mut type_buffer :[u8; 2] = [0;2];
        NetworkEndian::write_u16(&mut type_buffer[..], message_type);

        let mut length_buffer :[u8; 4] = [0;4];
        NetworkEndian::write_u32(&mut length_buffer[..], length);

        Message {
            message_type : type_buffer,
            length : length_buffer,
            data
        }
    }

    pub fn message_type(&self) -> u16 {
        NetworkEndian::read_u16(&self.message_type[..])
    }

    pub fn length(&self) -> u32 {
        NetworkEndian::read_u32(&self.length[..])
    }

    pub fn data(&self) -> &[u8] {
        &self.data[..]
    }

    pub fn from_reader<T: Read>(reader: &mut T) -> io::Result<Self> {
        let mut message = Message {
            message_type: [0; 2],
            length: [0; 4],
            data: Vec::new()
        };

        // Read Type
        let mut total_bytes_read = 0;
        while total_bytes_read < 2 {
            total_bytes_read += reader.read(&mut message.message_type[total_bytes_read..]).or_else(|err| {
                if err.kind() == std::io::ErrorKind::Interrupted {
                    Ok(0)
                } else {
                    Err(err)
                }
            })?;
        };

        // Read Length
        total_bytes_read = 0;
        while total_bytes_read < 4 {
            total_bytes_read += reader.read(&mut message.length[total_bytes_read..]).or_else(|err| {
                if err.kind() == std::io::ErrorKind::Interrupted {
                    Ok(0)
                } else {
                    Err(err)
                }
            })?;
        };

        // Read Data
        total_bytes_read = 0;
        message.data = vec![0; message.length() as usize];
        while total_bytes_read < message.length() as usize {
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

    pub fn into_writer<T: Write>(mut self, writer: &mut T) -> io::Result<()> {

        let mut total_bytes_written = 0;

        //TODO: Handle a case of 0 bytes written, it usually mean we have some kind of failure (buffer run out), and we might never manage to get out of the loop.
        // This is true for all writes loop below

        // Write Type
        while total_bytes_written < 2 as usize {
            total_bytes_written += writer.write(&mut self.message_type[total_bytes_written..]).or_else(|err| {
                if err.kind() == std::io::ErrorKind::Interrupted {
                    Ok(0)
                } else {
                    Err(err)
                }
            })?;
        };

        // Write Length
        total_bytes_written = 0;
        while total_bytes_written < 4 as usize {
            total_bytes_written += writer.write(&mut self.length[total_bytes_written..]).or_else(|err| {
                if err.kind() == std::io::ErrorKind::Interrupted {
                    Ok(0)
                } else {
                    Err(err)
                }
            })?;
        };

        // Write Data
        total_bytes_written = 0;
        while total_bytes_written < self.length() as usize {
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
}
