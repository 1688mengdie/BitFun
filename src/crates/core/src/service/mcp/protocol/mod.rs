//! MCP protocol layer
//!
//! Implements the core protocol definitions of Model Context Protocol and JSON-RPC 2.0
//! communication.

pub mod jsonrpc;
pub mod transport;
pub mod transport_remote;
pub mod types;

pub use jsonrpc::*;
pub use transport::*;
pub use transport_remote::*;
pub use types::*;
