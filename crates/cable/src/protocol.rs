//! Action Cable wire format (`ActionCable::INTERNAL` in actioncable/lib/action_cable.rb).
//!
//! Frames are Ruby hashes run through `ActiveSupport::JSON.encode`, so key order is the order
//! the hash literal is written in `Connection::Base` and `Channel::Base`.
use rails_compat::json;

/// Offered in `Sec-WebSocket-Protocol`; the first one the client lists that's in here wins
/// (websocket-driver's hybi negotiation walks the client's list, not the server's).
pub const PROTOCOLS: [&str; 2] = ["actioncable-v1-json", "actioncable-unsupported"];

pub const DEFAULT_MOUNT_PATH: &str = "/cable";

/// `ActionCable::Server::Connections::BEAT_INTERVAL`, in seconds.
pub const BEAT_INTERVAL: u64 = 3;

/// `ActionCable::INTERNAL[:disconnect_reasons]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    Unauthorized,
    InvalidRequest,
    ServerRestart,
    Remote,
}

impl DisconnectReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::InvalidRequest => "invalid_request",
            Self::ServerRestart => "server_restart",
            Self::Remote => "remote",
        }
    }
}

/// `{"type":"welcome"}`
pub fn welcome() -> String {
    r#"{"type":"welcome"}"#.to_string()
}

/// `{"type":"ping","message":<unix seconds>}` (`Connection::Base#beat`).
pub fn ping(unix_seconds: i64) -> String {
    format!(r#"{{"type":"ping","message":{unix_seconds}}}"#)
}

/// `{"type":"disconnect","reason":...,"reconnect":...}` (`Connection::Base#close`). `reason` is
/// `nil` when Rails calls `close` without one, and `reconnect` is whatever JSON value the remote
/// disconnect carried (it's `message.fetch("reconnect", true)`, unvalidated).
pub fn disconnect(reason: Option<DisconnectReason>, reconnect: &serde_json::Value) -> String {
    let reason = reason.map_or("null".to_string(), |r| format!("\"{}\"", r.as_str()));
    format!(r#"{{"type":"disconnect","reason":{reason},"reconnect":{}}}"#, json::encode(reconnect))
}

/// `{"identifier":...,"type":"confirm_subscription"}`
pub fn confirmation(identifier: &str) -> String {
    format!(r#"{{"identifier":{},"type":"confirm_subscription"}}"#, json::encode(identifier))
}

/// `{"identifier":...,"type":"reject_subscription"}`
pub fn rejection(identifier: &str) -> String {
    format!(r#"{{"identifier":{},"type":"reject_subscription"}}"#, json::encode(identifier))
}

/// `{"identifier":...,"message":...}` where `encoded_message` is already Active Support JSON.
/// Rails decodes each broadcast and re-encodes it inside this hash; passing the encoded payload
/// through is equivalent and saves the round trip per subscriber.
pub fn message(encoded_identifier: &str, encoded_message: &str) -> String {
    format!(r#"{{"identifier":{encoded_identifier},"message":{encoded_message}}}"#)
}
