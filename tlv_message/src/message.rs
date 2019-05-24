use std::io::prelude::*;
use std::io;
use std::io::Read;
use std::borrow::Borrow;
use std::string::String;
use byteorder::{NetworkEndian, ByteOrder, ReadBytesExt, WriteBytesExt};
use std::net::TcpStream;
use crate::message::Async::{NotReady, Ready};

// TODO: If message will implement read trait directly, it will be really easy to compose it with stream (and iterator)
// TODO: We should be able to return not_ready. The client should be able to continue on partial message.
#[derive(Default, Debug)]
pub struct Message {
    message_type : u16,
    length : u32,
    data : Vec<u8>
}

pub enum DataWrapper<'a> {
    Empty,
    /*
        For Integers use the Small option.
        You can specify only parts of the data is valid:
        a. For smaller integers (u16,u32 etc..)
        b. For partial int information (e.g. you have written only parts of the information)
    */
    Small([u8;8], usize, usize),

    LargeBorrowed(&'a mut[u8]),
    //LargeOwned(Vec<u8>)
}

impl<'a> DataWrapper<'a> {
    pub fn as_buffer(&mut self) -> &mut[u8] {
        match self {
            DataWrapper::Empty => &mut[],
            DataWrapper::Small(data, from, to) => &mut data[*from..*to],
            DataWrapper::LargeBorrowed(data) => data
        }
    }

    pub fn from_u16(data : u16) -> DataWrapper<'a> {
        let mut buffer :[u8; 8] = [0;8];
        NetworkEndian::write_u16(&mut buffer[..], data);
        DataWrapper::Small(buffer, 0, std::mem::size_of::<u16>())
    }

    pub fn from_partial_u16(data : u16, from : usize, to : usize) -> DataWrapper<'a> {
        if to > 8 || from > to {
            panic!("Invalid range given");
        }
        let mut buffer :[u8; 8] = [0;8];
        NetworkEndian::write_u16(&mut buffer[..], data);
        DataWrapper::Small(buffer, from, to)
    }

    pub fn from_u32(data : u32) -> DataWrapper<'a> {
        let mut buffer :[u8; 8] = [0;8];
        NetworkEndian::write_u32(&mut buffer[..], data);
        DataWrapper::Small(buffer, 0, std::mem::size_of::<u32>())
    }

    pub fn from_partial_u32(data : u32, from : usize, to : usize) -> DataWrapper<'a> {
        if to > 8 || from > to {
            panic!("Invalid range given");
        }

        let mut buffer :[u8; 8] = [0;8];
        NetworkEndian::write_u32(&mut buffer[..], data);
        DataWrapper::Small(buffer, from, to)
    }
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
    fn get_data(&mut self, location : usize) -> DataWrapper;
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
    error : Option<io::Error>
}

pub struct AsyncWriter<T : ByteBuffer + Default> {
    buffer : T,
    bytes_written : usize,
    done : bool,
    error : Option<io::Error>
}

impl<T : ByteBuffer + Default> AsyncReader<T> {
    pub fn new() -> AsyncReader<T> {
        AsyncReader {
            buffer : T::default(),
            bytes_read : 0,
            done : false,
            error : None
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    // Calling finish on unready object will panic
    // Should return an enum: T if ready, self if not ready, error if error
    pub fn finish(self) -> io::Result<T> {
        if !self.done {
            panic!("Called finish on message, but reading is not done");
        }

        match self.error {
            Some(e) => Err(e),
            None => Ok(self.buffer)
        }
    }

    pub fn async_read<R : AsyncRead>(&mut self, reader : &mut R) {
        let buffer = self.buffer.get_storage(self.bytes_read);
        match reader.partial_read_async(buffer) {
            AsyncResult::Ok(Async::NotReady) => {
                self.done = true;
                ()
            },
            AsyncResult::Ok(Async::Ready(0)) => self.done = true,
            AsyncResult::Ok(Async::Ready(bytes)) => self.bytes_read += bytes,
            AsyncResult::Ok(_) => (), // To make the linter relax
            AsyncResult::Err(error) => {
                self.error = Some(error);
                self.done = true;
            }
        }
    }
}
/*
let receiver = AsyncReader<Message>::new();
while !receiver.done() {
    receiver.async_read();
}

let message = receiver.finish()?;

let sender = AsyncWriter<Message>::new(message);
while !sender.done() {
    sender.async_write();
}

sender.result()?;
*/

impl<T : ByteBuffer + Default> AsyncWriter<T> {
    pub fn new(buffer : T) -> AsyncWriter<T> {
        AsyncWriter {
            buffer,
            bytes_written : 0,
            done : false,
            error : None
        }
    }

    pub fn done(&self) -> bool {
        self.done
    }

    // Calling finish on unready object will panic
    pub fn finish(self) -> io::Result<(T)> {
        if !self.done {
            panic!("Called finish on message, but reading is not done");
        }

        match self.error {
            Some(e) => Err(e),
            None => Ok(self.buffer)
        }
    }

    pub fn async_write<W : AsyncWrite>(&mut self, writer : &mut W) {
        let mut data = self.buffer.get_data(self.bytes_written);
        let buffer = data.as_buffer();
        match writer.partial_write_async(buffer) {
            AsyncResult::Ok(Async::NotReady) => {
                self.done = true;
                ()
            },
            AsyncResult::Ok(Async::Ready(0)) => self.done = true,
            AsyncResult::Ok(Async::Ready(bytes)) => self.bytes_written += bytes,
            AsyncResult::Ok(_) => (), // To make the linter relax
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
    fn get_data(&mut self, location : usize) -> DataWrapper {
        match location {
            0...1 => {
                DataWrapper::from_partial_u16(self.message_type, location, std::mem::size_of::<u16>())
            },
            2...5 => {
                DataWrapper::from_partial_u32(self.length, location - 2, std::mem::size_of::<u32>())
            },
            _ => {
                if location - 6 > self.length as usize {
                    DataWrapper::Empty
                } else {
                    DataWrapper::LargeBorrowed(&mut self.data[(location - 6)..])
                }
            }
        }
    }

    fn get_storage(&mut self, location : usize) -> &mut [u8] {
        match location {
            0 => {
                self.data.resize(2, 0);
                &mut self.data[location..]
            },
            1 => {
                &mut self.data[location..]
            },
            2 => {
                self.message_type = NetworkEndian::read_u16(&self.data[..]);
                self.data.resize(4,0);
                &mut self.data[(location - 2)..]
            },
            3...5 => {
                &mut self.data[(location - 2)..]
            } ,
            6 => {
                self.length = NetworkEndian::read_u32(&self.data[..]);
                self.data.clear();
                self.data.resize(self.length as usize,0);
                &mut self.data[(location - 6)..]
            },
            _ => {
                if location - 6 > self.length as usize {
                    &mut[]
                } else {
                    &mut self.data[(location - 6)..]
                }
            }
        }
    }
}

impl Message {
    pub fn new(message_type: u16, length: u32, data: Vec<u8>) -> Message {
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

    pub fn from_reader<T: Read>(reader: &mut T) -> io::Result<Self> {
        let mut message = Message {
            message_type: 0,
            length: 0,
            data: Vec::new()
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

    pub fn into_writer<T: Write>(mut self, writer: &mut T) -> io::Result<()> {

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
}