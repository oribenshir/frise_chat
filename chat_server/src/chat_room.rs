use std::{
    io::{self},
    net::{TcpStream},
    sync::mpsc,
};

use tlv_message::message::Message;

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
            let message = Message::from_reader(stream).expect("failed to read from connection");
            message_queue.push(message);
            /*
            let mut len_buffer: [u8;  mem::size_of::<usize>()] = [0;  mem::size_of::<usize>()];

            //TODO: This is not a real async, it will loop until all information is available
            stream.read_exact(len_buffer.as_mut()).expect("Failed to read message length");

            let mut read = 0;
            while read < mem::size_of::<usize>() {
                let _ = stream.read(&mut len_buffer.as_mut()[read..]).map(|x| {
                    println!("{}", x);
                    read += x;
                });
            }
            let length = NetworkEndian::read_u64(&len_buffer);
            //len_buffer.read_u64::<NetworkEndian>().expect("Failed to read message length");
            println!("Going to read message of length {}", length);

            // All elements can be initialized to the same value
            let mut buffer: [u8; 1024] = [0; 1024];
            let _ = stream.read(buffer.as_mut()).map(|x| {
                if x > 0 {
                    println!("msg {}", x);
                    let mut sent_buffer = Vec::with_capacity(x);
                    for i in 0..x {
                        sent_buffer.push(buffer[i]);
                    }
                    println!("reading new message from a client");
                    message_queue.push(sent_buffer);
                }
            });
            */
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
                    message.into_writer(&mut stream).expect("Failed to write message");
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
            // TODO: Uncomment when message implement async read
            //stream.set_nonblocking(true).expect("Failed to set socket as non blocking");
            //stream.set_read_timeout(Some(Duration::new(1, 0))).expect("Failed to set read timeout on socket");
            //stream.set_write_timeout(Some(Duration::new(1, 0))).expect("Failed to set read timeout on socket");
            chat_room.stream_list.push(stream);
        });
        // Look for a new message from any of the room's clients
        chat_room.look_for_new_message();
        // Broadcast received messages to all room's members
        chat_room.broadcast_pending_messages();
    }

    println!("Closing chat room");
    Ok(())
}
