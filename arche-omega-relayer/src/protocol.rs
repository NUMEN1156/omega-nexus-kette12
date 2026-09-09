use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum RelayMessage {
    Handshake { node_id: String, role: String },
    Heartbeat { timestamp: u64 },
    Payload { topic: String, data: Vec<u8> },
    Ack { status: String },
}
