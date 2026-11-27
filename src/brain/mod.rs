//! Capa de cerebro: cliente Ollama y (más adelante) tools y chat.

#[allow(unused_imports)]
pub mod chat;
#[allow(unused_imports)]
pub mod client;
#[allow(unused_imports)]
pub mod intent;
#[allow(unused_imports)]
pub mod items;
#[allow(unused_imports)]
pub mod tools;
#[allow(unused_imports)]
pub mod worldstate;

#[allow(unused_imports)]
pub use chat::{ChatMessage, StreamMsg};
#[allow(unused_imports)]
pub use client::OllamaClient;
#[allow(unused_imports)]
pub use worldstate::{WorldState, spawn_fetcher};

