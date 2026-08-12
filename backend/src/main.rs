mod auth;
mod config;
mod db;
mod error;
mod extract;
mod llm;
mod models;
mod organize;
mod redis_store;
mod request_id;
mod state;

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{Method, StatusCode, header},
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use models::{
    GraphDelta, IngestRequest, LabelCommunityRequest, LabelCommunityResult, OrganizeRequest,
    OrganizeResult, SearchRequest, SearchResult,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::auth::oauth::{GoogleOidc, OidcProvider, UnconfiguredOidc};
use crate::config::Config;
use crate::request_id::RequestId;
use crate::state::AppState;

async fn health() -> &'static str {
    "ok"
}

async fn health_ready(State(state): State<AppState>) -> Result<&'static str, (StatusCode, String)> {
    db::ping(&state.db)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, format!("db: {e}")))?;
    state
        .redis
        .ping()
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, format!("redis: {e}")))?;
    Ok("ok")
}

async fn ingest(
    State(state): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> Result<Json<GraphDelta>, (StatusCode, String)> {
    if req.text.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "text must not be empty".into()));
    }
    let delta = extract::extract_delta(&state.llm, &req).await;
    Ok(Json(delta))
}

async fn organize_graph(
    State(state): State<AppState>,
    Json(req): Json<OrganizeRequest>,
) -> Json<OrganizeResult> {
    let result = organize::organize(&state.llm, &req).await;
    Json(result)
}

async fn search_graph(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Json<SearchResult> {
    let result = organize::search(&state.llm, &req).await;
    Json(result)
}

async fn label_community_graph(
    State(state): State<AppState>,
    Json(req): Json<LabelCommunityRequest>,
) -> Json<LabelCommunityResult> {
    let result = organize::label_community(&state.llm, &req).await;
    Json(result)
}

async fn llm_status(State(state): State<AppState>) -> impl IntoResponse {
    let models = state.llm.list_models().await;
    Json(serde_json::json!({
        "mode": if state.llm.available() { "opencode" } else { "mock" },
        "model": state.llm.model,
        "base_url": state.llm.base_url,
        "models": models,
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "weave_api=debug,tower_http=debug,info".into()),
        )
        .init();

    let config = Config::from_env();
    let llm = llm::OpenCodeClient::from_env();
    let db = db::connect(&config.database_url).await?;
    let redis = redis_store::Redis::connect(&config.redis_url).await?;

    // Build the OIDC provider. Discovery against Google is network-bound, so
    // bound it: on failure (or missing credentials) OAuth is unavailable but
    // the rest of the API keeps running.
    let (oidc, oauth_configured): (Arc<dyn OidcProvider>, bool) =
        if let (Some(id), Some(secret), Some(redirect)) = (
            config.google_client_id.clone(),
            config.google_client_secret.clone(),
            config.google_redirect_uri.clone(),
        ) {
            match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                GoogleOidc::new(&id, &secret, &redirect),
            )
            .await
            {
                Ok(Ok(provider)) => (Arc::new(provider), true),
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "Google OAuth discovery failed; OAuth disabled");
                    (Arc::new(UnconfiguredOidc), false)
                }
                Err(_) => {
                    tracing::warn!("Google OAuth discovery timed out; OAuth disabled");
                    (Arc::new(UnconfiguredOidc), false)
                }
            }
        } else {
            (Arc::new(UnconfiguredOidc), false)
        };

    tracing::info!(
        mode = if llm.available() { "opencode" } else { "mock" },
        model = %llm.model,
        base_url = %llm.base_url,
        auth_stub = config.auth_stub,
        oauth_configured = oauth_configured,
        "weave-api configured"
    );

    let state = AppState {
        config: Arc::new(config),
        llm: Arc::new(llm),
        db,
        redis,
        oidc,
    };

    // CORS is restricted to the real frontend origin(s); credentials are
    // allowed only with explicit origins, never a wildcard.
    let origins = state
        .config
        .allowed_origins
        .iter()
        .filter_map(|o| o.parse::<header::HeaderValue>().ok())
        .collect::<Vec<_>>();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            "x-request-id".parse().unwrap(),
        ])
        .allow_credentials(true)
        .max_age(std::time::Duration::from_secs(3600));

    // Structured HTTP logs, enriched with the current request id.
    let trace = TraceLayer::new_for_http().make_span_with(
        |request: &axum::http::Request<axum::body::Body>| {
            let request_id = request
                .extensions()
                .get::<RequestId>()
                .map(|r| r.0.clone())
                .unwrap_or_else(|| "-".into());
            tracing::info_span!(
                "http_request",
                method = %request.method(),
                uri = %request.uri(),
                request_id = %request_id,
            )
        },
    );

    let app = Router::new()
        .merge(auth::routes())
        .route("/health", get(health))
        .route("/health/ready", get(health_ready))
        .route("/api/graph/ingest", post(ingest))
        .route("/api/graph/organize", post(organize_graph))
        .route("/api/graph/search", post(search_graph))
        .route("/api/graph/label-community", post(label_community_graph))
        .route("/api/status", get(llm_status))
        .layer(DefaultBodyLimit::max(state.config.max_body_bytes))
        .layer(trace)
        .layer(cors)
        .layer(middleware::from_fn(request_id::layer))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .expect("failed to bind port 3001");

    tracing::info!("weave-api listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
