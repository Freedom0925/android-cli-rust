//! CLI module for Android CLI
//!
//! Contains command definitions, context, and handlers

pub mod commands;
pub mod context;
pub mod handlers;

pub use commands::*;
pub use context::{Context, SysInfoService, get_channel_from_flags};
pub use handlers::*;