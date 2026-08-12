//! Shared application state, injected into every route and middleware.

use std::sync::Arc;

use crate::auth::oauth::OidcProvider;
use crate::config::Config;
use crate::llm::OpenCodeClient;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub llm: Arc<OpenCodeClient>,
    pub db: sqlx::PgPool,
    pub redis: crate::redis_store::Redis,
    pub oidc: Arc<dyn OidcProvider>,
}
