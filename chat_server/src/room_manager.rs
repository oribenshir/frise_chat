use std::net::{TcpStream};
use std::{io};
use std::io::{BufRead, BufReader};
use threadpool::ThreadPool;
use std::sync::mpsc;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};

use crate::utilities::work_token::Token;
use crate::chat_room;

pub struct RoomManager {
    receiver : mpsc::Receiver<TcpStream>,
    pool : ThreadPool,
    num_threads : usize,
    room_list : HashMap<String, mpsc::Sender<TcpStream>>,
}

impl RoomManager {
    pub fn new(receiver : mpsc::Receiver<TcpStream>, num_threads : usize) -> RoomManager {
        RoomManager {
            receiver,
            pool : ThreadPool::new(num_threads),
            num_threads,
            room_list : HashMap::new()
        }
    }

    pub fn activate(mut self, cancellation_token : Token) -> io::Result<()> {
        //TODO: currently we wait on the mspc during cancellation, we need to find a way to overcome it
        while let Some(stream) = self.receiver.iter().next() {
            let connection_token = cancellation_token.clone();
            self.handle_connection(stream, connection_token)?;

            if cancellation_token.canceled() {
                break;
            }
        }

        println!("waiting for pool to finish");
        self.pool.join();
        println!("pool finished its job");
        Ok(())
    }

    fn handle_connection(&mut self, stream: TcpStream, cancellation_token : Token) -> io::Result<()> {
        println!("handling new connection");
        let room_name = self.get_room_name(&stream)?;

        let mut room_dispatch = self.room_list.get(&room_name);

        if room_dispatch.is_none() {
            println!("Room {} not found", room_name);
            room_dispatch = self.create_room(&room_name, cancellation_token);
        }

        room_dispatch.and_then(|rx| {
            println!("Dispatching new client to room {}", room_name);
            rx.send(stream).ok()
        }).ok_or(Error::new(ErrorKind::Other, "Failed to dispatch client to room"))
    }

    fn create_room(&mut self, room_name: &str, cancellation_token : Token) -> Option<&mpsc::Sender<TcpStream>> {
        if self.pool.active_count() >= self.num_threads {
            println!("Thread Pool is full");
            None
        } else {
            let (tx, rx) = mpsc::channel();
            self.pool.execute(move || {
                //TODO: Add an exit mechanism
                if chat_room::chat_room_handler(rx, cancellation_token).is_err() {
                    println!("Error in chat room handler");
                };
            });

            self.room_list.insert(room_name.to_string(), tx);
            self.room_list.get(room_name)
        }
    }

    fn get_room_name(&self, stream: &TcpStream) -> io::Result<String> {
        let mut reader = BufReader::new(stream);
        let mut message = String::new();
        reader.read_line(&mut message)?;
        Ok(message)
    }
}