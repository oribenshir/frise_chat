#![feature(wait_until)]

use std::net::{TcpListener};
use std::{io, thread};
use std::sync::mpsc;

mod chat_room;
mod room_manager;
mod utilities;

use crate::room_manager::RoomManager;

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:80")?;
    let (tx, rx) = mpsc::channel();
    let manager_handler = thread::spawn(|| {
        let room_manager = RoomManager::new(rx, 16);
        if room_manager.activate().is_err(){
            println!("Error in room manager");
        }
    });

    // accept connections and process them serially
    for stream in listener.incoming() {
        match stream {
            Ok(connection) => {
                println!("New connection");
                tx.send(connection).unwrap();
            },
            Err(err) => println!("Error in incoming connection {}", err),
        }
    }

    // TODO: We should add a notification mechanism to close the server.
    manager_handler.join().expect_err("Failed to join manager handler");

    Ok(())
}