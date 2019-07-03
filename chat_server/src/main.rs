#![feature(wait_until)]

use std::net::{TcpListener};
use std::{io, thread};
use std::sync::mpsc;
use std::net::{TcpStream};

use utilities::work_token::Token;

mod chat_room;
mod room_manager;
mod utilities;

use crate::room_manager::RoomManager;

fn main() -> io::Result<()> {
    let token = Token::build();
    let room_manager_token = token.clone();
    let handler_token = token.clone();
    ctrlc::set_handler(move || {
        handler_token.cancel();
        // Release the listener
        TcpStream::connect("127.0.0.1:80").unwrap();
    }).expect("Error setting Ctrl-C handler");

    let listener = TcpListener::bind("127.0.0.1:80")?;
    let (tx, rx) = mpsc::channel();
    let manager_handler = thread::spawn(move || {
        let room_manager = RoomManager::new(rx, 16);
        if room_manager.activate(room_manager_token).is_err(){
            println!("Error in room manager");
        }
    });

    // Listener is not async, so it will wait during stop,
    // ideally we would want to turn it to async and use epoll (or relevant alternative)
    // This is behind the scope of this part of the project, so we will use a very ugly trick/workaround
    // When stopping the server, we will create a local connection from the server to itself, to release the listener, this can be done in the ctrlc handler.

    // accept connections and process them serially
    for stream in listener.incoming() {
        if token.canceled() {
            break
        }

        match stream {
            Ok(connection) => {
                println!("New connection");
                tx.send(connection).unwrap();
            },
            Err(err) => println!("Error in incoming connection {}", err),
        }
    }

    std::mem::drop(tx);
    std::mem::drop(listener);
    // Wait for the manager handler to be released
    let _result = manager_handler.join();

    Ok(())
}