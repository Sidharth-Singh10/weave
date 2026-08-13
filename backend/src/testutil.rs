//! Test-only shared state (compiled away in normal builds).
//!
//! DB-backed integration tests share the dev database; a mutex serializes them
//! so parallel test runs do not corrupt each other's fixtures.

#[cfg(test)]
pub mod db_lock {
    use std::sync::Mutex;

    /// Acquire before touching the shared database in a test.
    pub static LOCK: Mutex<()> = Mutex::new(());
}
