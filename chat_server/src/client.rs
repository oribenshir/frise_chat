use std::net::TcpStream;
use tlv_message::message::{AsyncReader, AsyncWriter, Message, AsyncWriteResult, AsyncReadResult};

pub struct ClientStream {
    stream : TcpStream,
    async_reader : Option<AsyncReader<Message>>,
    message_queue : Vec<AsyncWriter<Message>>
}

impl ClientStream {
    pub fn build(stream : TcpStream) -> ClientStream {
        ClientStream {
            stream,
            async_reader : None,
            message_queue : Vec::new()
        }
    }

    pub fn write_messages_to_stream(&mut self, mut messages: Vec<AsyncWriter<Message>>) {
        // Queue all messages in our message queue. This is done as we want to handle messages from previous calls first
        self.queue_messages(messages);

        let mut incomplete_message_queue =  Vec::<AsyncWriter<Message>>::new();

        while let Some(mut message) = self.message_queue.pop() {
            // Write as much as we can without sleeping
            while !message.done() {
                message.async_write(&mut self.stream);
            }

            // Check everything went as expected
            // TODO: at the moment just crash, later on we will want to remove stream from the room as we are out of sync
            let result = message.finish().expect("failed to write message to client");

            // Check the result, if we didn't send all of the message we will want to continue it on the next loop
            if let AsyncWriteResult::NotReady(message) = result {
                incomplete_message_queue.push(message);
            }
        }

        self.message_queue = incomplete_message_queue;
    }

    fn read_async_from_stream(&mut self) {
        let receiver = self.async_reader.get_or_insert_with(|| { AsyncReader::<Message>::new() });
        while !receiver.done() {
            receiver.async_read(&mut self.stream);
        }
    }

    pub fn read_message(&mut self) -> Option<Message> {
        self.read_async_from_stream();

        let mut async_reader = self.async_reader.take();
        let mut result = async_reader.unwrap() // We know it is safe, as we just made sure to put a value inside of inside read_async_from_stream
            .finish().expect("failed to read from connection");

        match result {
            AsyncReadResult::NotReady(async_reader) => {
                self.async_reader = Some(async_reader);
                None
            },
            AsyncReadResult::Ready(message) => {
                //message_queue.push(AsyncWriter::<Message>::new(message));
                Some(message)
            }
        }
    }

    fn queue_messages(&mut self, mut messages: Vec<AsyncWriter<Message>>) {
        self.message_queue.append(&mut messages);
    }
}