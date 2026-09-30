//! Action Cable for Axum: the `actioncable-v1-json` protocol server, channels, and the in-process
//! pub/sub that replaces Redis.
//!
//! The app builds a [`Server`] with its connection authenticator and channel classes, mounts
//! [`Server::router`] at `/cable`, and broadcasts through [`Server::broadcast`] and the Turbo
//! helpers in [`turbo`]. Everything here mirrors actioncable and turbo-rails at the versions in
//! `reference/Gemfile.lock`.
pub mod channel;
mod connection;
pub mod naming;
pub mod protocol;
pub mod pubsub;
mod server;
pub mod socket;
pub mod turbo;

pub use channel::{Channel, ChannelError, ChannelResult, EmptyChannel, Params, Subscription};
pub use server::{Authenticate, Config, ConnectRequest, Identified, Server, ServerBuilder};
