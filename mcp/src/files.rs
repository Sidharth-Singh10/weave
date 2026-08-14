//! On-disk blob store for documents (text, PDFs, images, audio, …).
//!
//! Files are written under the configured data dir keyed by SHA-256 of their
//! contents; the database stores the key + metadata, not the bytes.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Compute the storage key (hex SHA-256) for file bytes.
pub fn key_for_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn data_path(data_dir: &str, key: &str) -> PathBuf {
    Path::new(data_dir).join(key)
}

/// Persist bytes, returning the storage key.
pub fn store_bytes(data_dir: &str, bytes: &[u8]) -> anyhow::Result<String> {
    let key = key_for_bytes(bytes);
    let path = data_path(data_dir, &key);
    if !path.exists() {
        std::fs::create_dir_all(data_dir)?;
        std::fs::write(&path, bytes)?;
    }
    Ok(key)
}

/// Read bytes back by storage key (used by retrieval/resource reads).
#[allow(dead_code)]
pub fn read_bytes(data_dir: &str, key: &str) -> anyhow::Result<Vec<u8>> {
    let path = data_path(data_dir, key);
    std::fs::read(path).map_err(Into::into)
}

/// Best-effort text extraction from common text-ish files.
pub fn extract_text(filename: &str, bytes: &[u8]) -> Option<String> {
    let lower = filename.to_lowercase();
    let is_text = matches!(
        lower.rsplit('.').next(),
        Some(
            "txt"
                | "md"
                | "markdown"
                | "json"
                | "yml"
                | "yaml"
                | "toml"
                | "csv"
                | "rs"
                | "py"
                | "ts"
                | "js"
                | "html"
                | "css"
        )
    );
    if is_text {
        String::from_utf8(bytes.to_vec()).ok()
    } else {
        None
    }
}

/// Guess a MIME type from a filename extension.
pub fn guess_mime(filename: &str) -> &'static str {
    match filename.to_lowercase().rsplit('.').next() {
        Some("txt" | "md" | "markdown") => "text/plain",
        Some("json") => "application/json",
        Some("csv") => "text/csv",
        Some("html") => "text/html",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("m4a") => "audio/mp4",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_stable_hex() {
        let k = key_for_bytes(b"hello");
        assert_eq!(k.len(), 64);
        assert_eq!(k, key_for_bytes(b"hello"));
        assert_ne!(k, key_for_bytes(b"world"));
    }

    #[test]
    fn stores_and_reads_bytes() {
        let dir = std::env::temp_dir().join(format!("weave-mcp-test-{}", uuid::Uuid::new_v4()));
        let key = store_bytes(dir.to_str().unwrap(), b"payload").unwrap();
        assert_eq!(read_bytes(dir.to_str().unwrap(), &key).unwrap(), b"payload");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extracts_text_and_guesses_mime() {
        assert_eq!(extract_text("note.md", b"# Hi").as_deref(), Some("# Hi"));
        assert_eq!(extract_text("image.png", b"\x89PNG"), None);
        assert_eq!(guess_mime("photo.png"), "image/png");
        assert_eq!(guess_mime("audio.mp3"), "audio/mpeg");
    }
}
