//! Shared core for Weave: the OpenAI-compatible LLM client, the graph data
//! model, and the text → entities/relations extractor.
//!
//! Used by both `weave-api` (the web backend) and `weave-mcp` (the knowledge /
//! memory MCP server), so the two stay on one extraction implementation.

pub mod extract;
pub mod llm;
pub mod models;
pub mod subgraph;
