//! Embeddings for GraphRAG retrieval.
//!
//! An [`Embedder`] turns text into a fixed-size vector stored in pgvector.
//! Production uses local [`FastembedEmbedder`] (ONNX, offline, private) when
//! the `embedding` cargo feature is enabled; a deterministic [`StubEmbedder`]
//! keeps the pipeline functional and testable without a model download.

use std::sync::Arc;

pub const EMBEDDING_DIMS: usize = 384;

/// Text → embedding vector.
pub trait Embedder: Send + Sync {
    fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
}

/// Deterministic, dependency-free embedder. Not semantically meaningful — it
/// only keeps the retrieval pipeline exercised in tests and feature-less
/// builds.
pub struct StubEmbedder;

impl Embedder for StubEmbedder {
    fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        use sha2::{Digest, Sha256};
        let mut vector = Vec::with_capacity(EMBEDDING_DIMS);
        for i in 0..EMBEDDING_DIMS {
            // Stable per-character-ish signal derived from the whole text.
            let seed = format!("{text}::{i}");
            let hash = Sha256::digest(seed.as_bytes());
            let value = hash[0] as f32 / 255.0;
            vector.push(value - 0.5);
        }
        // Deterministic normalization so cosine distance is meaningful-ish.
        let norm: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
        for v in &mut vector {
            *v /= norm;
        }
        Ok(vector)
    }
}

/// Local ONNX embeddings via fastembed (default `BAAI/bge-small-en-v1.5`,
/// 384 dims; model downloaded from Hugging Face on first use).
#[cfg(feature = "embedding")]
pub struct FastembedEmbedder {
    model: std::sync::Mutex<fastembed::TextEmbedding>,
}

#[cfg(feature = "embedding")]
impl FastembedEmbedder {
    pub fn new(_model_name: &str) -> anyhow::Result<Self> {
        let options = fastembed::TextInitOptions::new(fastembed::EmbeddingModel::BGESmallENV15);
        let model = fastembed::TextEmbedding::try_new(options)?;
        Ok(Self {
            model: std::sync::Mutex::new(model),
        })
    }
}

#[cfg(feature = "embedding")]
impl Embedder for FastembedEmbedder {
    fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut model = self
            .model
            .lock()
            .map_err(|_| anyhow::anyhow!("embedding model lock poisoned"))?;
        let docs = model.embed(vec![text.to_string()], None)?;
        docs.into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("embedding produced no vectors"))
    }
}

/// Build the embedder for the running binary: fastembed when the feature is
/// enabled (falling back to the stub if the model cannot load), otherwise the
/// stub.
pub fn build_embedder() -> Arc<dyn Embedder> {
    #[cfg(feature = "embedding")]
    {
        let model = std::env::var("WEAVE_MCP_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "BAAI/bge-small-en-v1.5".to_string());
        match FastembedEmbedder::new(&model) {
            Ok(embedder) => {
                tracing::info!(model = %model, "local embedding model loaded");
                return Arc::new(embedder);
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to load embedding model; using stub embedder");
            }
        }
    }
    tracing::warn!("stub embedder active (semantic similarity disabled)");
    Arc::new(StubEmbedder)
}
