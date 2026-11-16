use serde::{Serialize, de::DeserializeOwned};

fn config() -> bincode::config::Configuration {
    bincode::config::standard()
}

pub struct Encoder {
    output_buffer: Vec<u8>,
}

impl Encoder {
    pub fn new() -> Encoder {
        Self { output_buffer: Vec::new() }
    }

    pub fn encode<M: Serialize>(&mut self, message: M) -> &[u8] {
        self.output_buffer.clear();
        bincode::serde::encode_into_std_write(&message, &mut self.output_buffer, config()).unwrap();
        &self.output_buffer
    }
}

pub fn decode<M: DeserializeOwned>(data_message: &[u8]) -> Option<M> {
    bincode::serde::decode_from_slice::<M, _>(data_message, config()).ok().map(|(message, _)| message)
}
