//! weave-mcp: a personal knowledge/memory MCP server library.
//!
//! Exposed as a library so integration tests can drive ingestion/storage
//! directly; the binary (`main.rs`) wires this up over the MCP stdio
//! transport.

pub mod config;
pub mod db;
pub mod embed;
pub mod files;
pub mod graph;
pub mod ingest;
pub mod models;
pub mod recall;
pub mod server;
pub mod store;
pub mod summary;
