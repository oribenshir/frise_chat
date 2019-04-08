use std::io::prelude::*;
use std::io;

use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};

// TODO: If message will implement read trait directly, it will be really easy to compose it with stream (and iterator)
#[derive(Debug)]
pub struct Message {
    message_type : u16,
    length : u32,
    data : Vec<u8>
}

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

}