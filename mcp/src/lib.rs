//! weave-mcp: a personal knowledge/memory MCP server library.
//!
//! Exposed as a library so integration tests can drive ingestion/storage
//! directly; the binary (`main.rs`) wires this up over the MCP stdio
//! transport.

pub mod claims;
pub mod audit;
pub mod config;
pub mod db;
pub mod embed;
pub mod entity;
pub mod files;
pub mod graph;
pub mod ingest;
pub mod models;
pub mod recall;
pub mod retrieval;
pub mod server;
pub mod store;
pub mod summary;
pub mod validate;
pub mod verify;
