use std::net::{TcpStream};
use std::io::prelude::*;
use std::io;
use std::mem;
use std::io::{BufReader, BufWriter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use tlv_message::message::Message;

fn main() -> io::Result<()> {
    let reading = Arc::new(AtomicBool::new(true));
    let r = reading.clone();
    let r2 = reading.clone();

    println!("Creating new control handler");
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let rstream = TcpStream::connect("127.0.0.1:80")?;
    println!("Connected to chat server");
    let wstream = rstream.try_clone()?;
    let mut reader = BufReader::new(rstream);
    let mut writer = BufWriter::new(wstream);

    let mut buffer = String::new();

    println!("Creating new reader from connection");

    let handler = thread::spawn(move || {
        let mut line = String::new();
        while r2.load(Ordering::SeqCst) {
            println!("Reading from connection");
            let message = Message::from_reader(&mut reader).expect("failed to read from connection");
            let result = message.data().read_to_string(&mut line).expect("failed to read data from message");
            println!("Room: {}", line);
            line.clear();
        }
    });

    let mut first = true;
    println!("Start client loop");
    while reading.load(Ordering::SeqCst) {
        println!("Reading from console");
        io::stdin().read_line(&mut buffer)?;
        if buffer.as_str().eq("stop\n") {
            println!("Stopping");
            reading.store(false, Ordering::SeqCst);
        } else {
            if !first {
                println!("Writing to chat {} bytes: {}", buffer.len(), buffer);
                let message = Message::new(1, buffer.len() as u32, buffer.clone().into_bytes());
                message.into_writer(writer.by_ref())?;
                /*
                let mut bs = [0u8; mem::size_of::<usize>()];
                bs.as_mut()
                    .write_u64::<NetworkEndian>(buffer.len() as u64)?;
                writer.by_ref().write(&bs)?;
                */
            } else {
                first = false;
                writer.write(buffer.as_bytes())?;
            }
            println!("flush");
            writer.flush()?;
        }

        buffer.clear();
    }
    handler.join().expect_err("Error joining thread ");
    println!("done");
    Ok(())
}