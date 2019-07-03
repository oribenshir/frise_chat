use std::{
    io::{self},
    net::{TcpStream},
    sync::mpsc,
};

use tlv_message::message::{AsyncReader, AsyncWriter, Message, AsyncReadResult, AsyncWriteResult};
use crate::utilities::work_token::Token;
use crate::client::ClientStream;

pub struct ChatRoom {
    // All client currently in the chat room (Each client has a dedicate stream)
    stream_list : Vec<ClientStream>,
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
            if let Some(message) = stream.read_message() {
                message_queue.push(AsyncWriter::<Message>::new(message));
            }
        });
    }

    pub fn broadcast_pending_messages(&mut self) {
        // Loop over through all streams, clone pending messages to stream, and distribute them
        for stream in &mut self.stream_list {
            stream.write_messages_to_stream(self.message_queue.clone());
        }
        // All messages are now in the streams internal queues, so we can clear this queue
        self.message_queue.clear();
    }
}

pub fn chat_room_handler(receiver: mpsc::Receiver<TcpStream>, cancellation_token : Token) -> io::Result<()> {
    println!("Opening new chat room");

    let mut chat_room = ChatRoom::new();

    loop {
        if cancellation_token.canceled() {
            break;
        }
        // Look for a new connection to the chat room
        let _ = receiver.try_recv().map(|stream| {
            println!("Received a new connection to the room");
            stream.set_nonblocking(true).expect("Failed to set socket as non blocking");
            chat_room.stream_list.push(ClientStream::build(stream));
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
