//! MCP server configuration, loaded once from the environment.

#[derive(Clone, Debug)]
pub struct Config {
    /// PostgreSQL URL for the `weave_mcp` database (created on first run).
    pub database_url: String,
    /// Directory where file blobs are stored.
    pub data_dir: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("WEAVE_MCP_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| "postgres://weave:weave@localhost:5432/weave_mcp".to_string());
        let data_dir = std::env::var("WEAVE_MCP_DATA_DIR").unwrap_or_else(|_| "./data".to_string());

        Ok(Self {
            database_url,
            data_dir,
        })
    }
}
