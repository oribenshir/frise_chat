use std::{
    io::{self},
    net::{TcpStream},
    sync::mpsc,
};

use tlv_message::message::{AsyncReader, AsyncWriter, Message, AsyncReadResult, AsyncWriteResult};

pub struct Stream {
    stream : TcpStream,
    async_reader : Option<AsyncReader<Message>>
}

pub struct ChatRoom {
    // All client currently in the chat room (Each client has a dedicate stream)
    stream_list : Vec<Stream>,
    // All the messages that are waiting to be sent to the streams
    message_queue: Vec<AsyncWriter<Message>>
}

impl ChatRoom {
    pub fn new() -> ChatRoom {
        ChatRoom {
            stream_list : Vec::new(),
            message_queue : Vec::new()
        }
    }

    pub fn look_for_new_message(&mut self) {
        let stream_list = &mut self.stream_list;
        let message_queue = &mut self.message_queue;

        stream_list.iter_mut().for_each(|stream| {
            {
                let receiver = stream.async_reader.get_or_insert_with(|| { AsyncReader::<Message>::new() });
                while !receiver.done() {
                    println!("async_read hopefully");
                    receiver.async_read(&mut stream.stream);
                }
            }

            let mut async_reader = stream.async_reader.take();
            let mut result = async_reader.unwrap() // We know it is safe, as we just made sure to put a value inside of it
                .finish().expect("failed to read from connection");

            match result {
                AsyncReadResult::NotReady(async_reader) => {
                    stream.async_reader = Some(async_reader);
                },
                AsyncReadResult::Ready(message) => {
                    message_queue.push(AsyncWriter::<Message>::new(message));
                }
            }
        });
    }

    pub fn broadcast_pending_messages(&mut self) {
        let stream_list = &mut self.stream_list;
        let message_queue = &mut self.message_queue;

        if !message_queue.is_empty() {
            println!("I've got a message to send to the room");
            stream_list.iter_mut().for_each(|mut stream| {
                while let Some(mut writer) = message_queue.pop() {
                    while !writer.done() {
                        println!("async_write hopefully");
                        writer.async_write(&mut stream.stream);
                    }

                    let result = writer.finish().expect("failed to write message to client");
                    if let AsyncWriteResult::NotReady(next_writer) = result {
                        message_queue.push(next_writer);
                    }
                }
            });
            // We've sent all the messages in the queue, we can clear it now
            message_queue.clear();
        }
    }
}

pub fn chat_room_handler(receiver: mpsc::Receiver<TcpStream>) -> io::Result<()> {
    println!("Opening new chat room");

    let mut chat_room = ChatRoom::new();

    loop {
        // Look for a new connection to the chat room
        let _ = receiver.try_recv().map(|stream| {
            println!("Received a new connection to the room");
            stream.set_nonblocking(true).expect("Failed to set socket as non blocking");
            chat_room.stream_list.push(Stream{stream, async_reader : None });
        });
        println!("looking for new messages");
        // Look for a new message from any of the room's clients
        chat_room.look_for_new_message();
        println!("broadcasting pending messages");
        // Broadcast received messages to all room's members
        chat_room.broadcast_pending_messages();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    println!("Closing chat room");
    Ok(())
}
