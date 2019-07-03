#![feature(wait_until)]

use std::net::{TcpListener};
use std::{io, thread};
use std::sync::mpsc;
use std::net::{TcpStream};

use utilities::work_token::Token;

mod chat_room;
mod room_manager;
mod utilities;
mod server;

use crate::room_manager::{RoomManager, RoomManagerHandler};

fn set_signal_handlers(token : Token) {
    ctrlc::set_handler(move || {
        token.cancel();
        // Connect to the server in order to release its blocking listener.
        // This is an ugly but practical workaround, as I don't want to introduce an OS dependent polling system (such as epoll)
        TcpStream::connect("127.0.0.1:80").unwrap();
    }).expect("Error setting Ctrl-C handler");
}

fn main() -> io::Result<()> {
    // Create token which will be used to shutdown the server cleanly
    let token = Token::build();
    // Setup a signal handler in order to allow shutting down the server with Ctrl+C
    set_signal_handlers(token.clone());
    // Create a channel to distribute TcpStreams from the server to the chat room manager
    let (tx, rx) = mpsc::channel();
    // Spin up the chat room handler in a new thread
    let manager_handler = RoomManagerHandler::spawn(rx, 16, token.clone());
    // Create the TCP server
    let server = server::Server::new("127.0.0.1:80", tx)?;
    // Listen for new connection as long as token is available
    server.accept_while_token_available(token.clone());
    // Wait for the chat room manager to close up cleanly
    let _result = manager_handler.join();
    Ok(())
}