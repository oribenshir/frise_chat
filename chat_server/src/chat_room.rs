use std::{
    io::{self},
    net::{TcpStream},
    sync::mpsc,
};

use tlv_message::message::{AsyncReader, AsyncWriter, Message};

pub struct ChatRoom {
    // All client currently in the chat room (Each client has a dedicate stream)
    stream_list : Vec<TcpStream>,
    // All the messages that are waiting to be sent to the streams
    message_queue: Vec<Message>
}

impl ChatRoom {
    pub fn new() -> ChatRoom {
        ChatRoom{
            stream_list : Vec::new(),
            message_queue : Vec::new()
        }
    }

    pub fn look_for_new_message(&mut self) {
        let stream_list = &mut self.stream_list;
        let message_queue = &mut self.message_queue;

        stream_list.iter_mut().for_each(|stream| {
            //let message = Message::from_reader(stream).expect("failed to read from connection");
            // TODO: we are going to change the finish interface. Therefore each stream might start with unready reader from previous run
            // We need to do 2 things:
            // 1. wrap TcpStream with a struct that contain a TcpStream and an Optional Reader
            // 2. Adjust this code:
            //      a. if no reader, create one else use the existing one
            //      b. read loop can stays the same
            //      c. finish should check for enum result, which can contain error as today, but in case everything went well, it might have either a message (everything went alright) or reader (no data was read)
            let mut receiver = AsyncReader::<Message>::new();
            while !receiver.done() {
                println!("async_read hopefully");
                receiver.async_read(stream);
            }

            let message = receiver.finish().expect("failed to read from connection");
            message_queue.push(message);
        });
    }

    pub fn broadcast_pending_messages(&mut self) {
        let stream_list = &mut self.stream_list;
        let message_queue = &mut self.message_queue;

        if !message_queue.is_empty() {
            println!("I've got a message to send to the room");
            stream_list.iter_mut().for_each(|mut stream| {
                while let Some(message) = message_queue.pop() {
                    println!("Sending {:?} to a client", std::str::from_utf8(message.data()).unwrap());
                    let mut writer = AsyncWriter::<Message>::new(message);
                    while !writer.done() {
                        println!("async_write hopefully");
                        writer.async_write(stream);
                    }
                    //message.into_writer(&mut stream).expect("Failed to write message");
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
        println!("looping");
        // Look for a new connection to the chat room
        let _ = receiver.try_recv().map(|stream| {
            println!("Received a new connection to the room");
            // TODO: Uncomment when message implement async read
            stream.set_nonblocking(true).expect("Failed to set socket as non blocking");
            //stream.set_read_timeout(Some(Duration::new(1, 0))).expect("Failed to set read timeout on socket");
            //stream.set_write_timeout(Some(Duration::new(1, 0))).expect("Failed to set read timeout on socket");
            chat_room.stream_list.push(stream);
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
