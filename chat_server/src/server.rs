use std::net::{TcpListener, TcpStream};
use std::io::Error;
use std::sync::mpsc;

use crate::utilities::work_token::Token;

/// A TCP Server
/// Listen to new connection and dispatch them to an handler down the stream
pub struct Server {
    listener : TcpListener,
    dispatcher : mpsc::Sender<TcpStream>,
}

impl Server {
    /// Create the server, provide an address to listen on, and an handler for the TcpStreams (via a channel)
    pub fn new<T>(address : T, dispatcher : mpsc::Sender<TcpStream>) -> Result<Server, Error>
        where T : AsRef<str> {
        Ok(Server {
            listener : TcpListener::bind(address.as_ref())?,
            dispatcher
        })
    }

    /// Will accept new connections as long as the token wasn't canceled
    pub fn accept_while_token_available(self, token : Token) {
        // Listener is not async, so it will wait during stop,
        // ideally we would want to turn it to async and use epoll (or relevant alternative)
        // This is behind the scope of this part of the project, so we will use a very ugly trick/workaround
        // When stopping the server, we will create a local connection from the server to itself, to release the listener, this can be done in the ctrlc handler.

        for stream in self.listener.incoming() {
            if token.canceled() {
                break
            }

            match stream {
                Ok(connection) => {
                    println!("New connection");
                    self.dispatcher.send(connection).unwrap();
                },
                Err(err) => println!("Error in incoming connection {}", err),
            }
        }

        // Manually drop the dispatcher to make sure it is being dropped before the listener (avoid errors on exit)
        std::mem::drop(self.dispatcher);
    }
}