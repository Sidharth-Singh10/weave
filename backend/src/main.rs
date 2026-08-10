mod extract;
mod llm;
mod models;
mod organize;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use models::{GraphDelta, IngestRequest, OrganizeRequest, OrganizeResult, SearchRequest, SearchResult};
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct AppState {
    llm: std::sync::Arc<llm::OpenCodeClient>,
}

async fn health() -> &'static str {
    "ok"
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
async fn main() {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "weave_api=debug,tower_http=debug,info".into()),
        )
        .init();

    let llm = llm::OpenCodeClient::from_env();
    tracing::info!(
        mode = if llm.available() { "opencode" } else { "mock" },
        model = %llm.model,
        base_url = %llm.base_url,
        "extractor configured"
    );

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let state = AppState {
        llm: std::sync::Arc::new(llm),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/graph/ingest", post(ingest))
        .route("/api/graph/organize", post(organize_graph))
        .route("/api/graph/search", post(search_graph))
        .route("/api/status", get(llm_status))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .expect("failed to bind port 3001");

    tracing::info!("weave-api listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
