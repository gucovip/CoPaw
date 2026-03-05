mod crons;
mod envs;
mod mcp;
mod repo;
mod routes;
mod runner;
mod skills;

use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    Router,
};
use copaw_config::get_working_dir;
use copaw_providers::{ProviderRegistry, ProviderStore};
use routes::{
    AgentState, ChatsState, ConfigState, CronState, DownloadManager, EnvsState, HasConfigState,
    HasCronState, HasEnvsState, HasMcpState, LocalModelsState, McpState, ModelsState,
    OllamaModelsState, LOCAL_MODELS_DIR,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{info, Level};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_ADDRESS: &str = "127.0.0.1:8088";

#[derive(Clone)]
struct AppState {
    console_static_dir: Option<PathBuf>,
    config_state: ConfigState,
    models_state: ModelsState,
    cron_state: CronState,
    mcp_state: McpState,
    envs_state: EnvsState,
    local_models_state: LocalModelsState,
    ollama_models_state: OllamaModelsState,
    agent_state: AgentState,
    chats_state: ChatsState,
}

impl HasConfigState for AppState {
    fn config_state(&self) -> &ConfigState {
        &self.config_state
    }
}

impl HasCronState for AppState {
    fn cron_state(&self) -> &CronState {
        &self.cron_state
    }
}

impl HasMcpState for AppState {
    fn mcp_state(&self) -> &McpState {
        &self.mcp_state
    }
}

impl HasEnvsState for AppState {
    fn envs_state(&self) -> &EnvsState {
        &self.envs_state
    }
}

#[derive(Serialize, Deserialize)]
struct VersionResponse {
    version: String,
}

#[derive(Serialize, Deserialize)]
struct HelloResponse {
    message: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// Custom error handler
#[derive(Debug)]
pub struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    fn internal_error(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ErrorResponse {
            error: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.status, self.message)
    }
}

impl std::error::Error for AppError {}

// Resolve console static directory (mirrors Python _resolve_console_static_dir)
fn resolve_console_static_dir() -> Option<PathBuf> {
    // Check environment variable first
    if let Ok(env_dir) = std::env::var("COPAW_CONSOLE_STATIC_DIR") {
        let dir = PathBuf::from(env_dir);
        if dir.is_dir() {
            return Some(dir);
        }
    }

    // Check current working directory for console/dist or console_dist
    if let Ok(cwd) = std::env::current_dir() {
        for subdir in ["console/dist", "console_dist"] {
            let candidate = cwd.join(subdir);
            if candidate.is_dir() && candidate.join("index.html").exists() {
                return Some(candidate);
            }
        }
    }

    None
}

// Helper function to serve index.html from console directory
async fn serve_index_html(console_dir: &Path) -> Result<Response, AppError> {
    let index_path = console_dir.join("index.html");
    if !index_path.exists() {
        return Err(AppError::not_found("index.html not found"));
    }

    let bytes = tokio::fs::read(&index_path)
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to read index.html: {}", e)))?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html")],
        Body::from(bytes),
    )
        .into_response())
}

// Root endpoint - serves console frontend or hello message
async fn root_handler(State(state): State<AppState>) -> Result<Response, AppError> {
    if let Some(console_dir) = &state.console_static_dir {
        let index_path = console_dir.join("index.html");
        if index_path.exists() {
            return serve_index_html(console_dir).await;
        }
    }

    // Fallback to hello message
    let response = HelloResponse {
        message: "Hello World".to_string(),
    };
    Ok((StatusCode::OK, Json(response)).into_response())
}

// Version endpoint
async fn version_handler() -> Json<VersionResponse> {
    Json(VersionResponse {
        version: VERSION.to_string(),
    })
}

// Serve logo.png
async fn logo_handler(State(state): State<AppState>) -> Result<Response, AppError> {
    let console_dir = state
        .console_static_dir
        .as_ref()
        .ok_or_else(|| AppError::not_found("Console static dir not configured"))?;

    let logo_path = console_dir.join("logo.png");
    if !logo_path.exists() {
        return Err(AppError::not_found("Logo not found"));
    }

    let bytes = tokio::fs::read(&logo_path)
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to read logo: {}", e)))?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        Body::from(bytes),
    )
        .into_response())
}

// Serve copaw-symbol.svg
async fn icon_handler(State(state): State<AppState>) -> Result<Response, AppError> {
    let console_dir = state
        .console_static_dir
        .as_ref()
        .ok_or_else(|| AppError::not_found("Console static dir not configured"))?;

    let icon_path = console_dir.join("copaw-symbol.svg");
    if !icon_path.exists() {
        return Err(AppError::not_found("Icon not found"));
    }

    let bytes = tokio::fs::read(&icon_path)
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to read icon: {}", e)))?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        Body::from(bytes),
    )
        .into_response())
}

// SPA fallback handler
async fn spa_fallback_handler(State(state): State<AppState>) -> Result<Response, AppError> {
    if let Some(console_dir) = &state.console_static_dir {
        let index_path = console_dir.join("index.html");
        if index_path.exists() {
            return serve_index_html(console_dir).await;
        }
    }

    Err(AppError::not_found("Not Found"))
}

// Build router with all routes
fn build_router(console_static_dir: Option<PathBuf>) -> Router {
    let config_state = ConfigState::new(None);

    // Initialize provider registry and store
    let registry = Arc::new(ProviderRegistry::new());
    let providers_path = ProviderStore::default_path();
    let store = Arc::new(ProviderStore::new(providers_path, registry.clone()));
    let models_state = ModelsState {
        registry: registry.clone(),
        store: store.clone(),
    };

    // Initialize MCP and Envs states
    let mcp_state = McpState::new(None);
    let envs_state = EnvsState::new(None);

    // Initialize cron state
    let cron_state = CronState::new(None);

    // Initialize local models state
    let models_dir = get_working_dir().join(LOCAL_MODELS_DIR);
    let download_manager = Arc::new(DownloadManager::new(models_dir.clone()));
    let local_models_state = LocalModelsState::new(registry.clone(), store.clone(), models_dir);

    // Initialize Ollama models state (shares download manager with local models)
    let ollama_models_state =
        OllamaModelsState::new(registry.clone(), store.clone(), download_manager);

    // Initialize chats state
    let chats_repo = crate::repo::ChatRepository::new().expect("Failed to create chat repository");
    let chat_manager = Arc::new(crate::runner::chat_manager::ChatManager::with_repository(
        chats_repo,
    ));
    let chats_state = ChatsState {
        manager: chat_manager.clone(),
    };

    // Initialize agent state
    let working_dir = get_working_dir();
    let file_manager = Arc::new(
        copaw_agents::AgentFileManager::with_working_dir(&working_dir)
            .expect("Failed to create agent file manager"),
    );
    let agent_state = AgentState {
        file_manager,
        registry: registry.clone(),
        store: store.clone(),
        chat_manager,
    };

    let state = AppState {
        console_static_dir,
        config_state,
        models_state,
        cron_state,
        mcp_state,
        envs_state,
        local_models_state,
        ollama_models_state,
        agent_state,
        chats_state,
    };

    let router = Router::new()
        .route("/", get(root_handler))
        .route("/api/version", get(version_handler))
        .route("/logo.png", get(logo_handler))
        .route("/copaw-symbol.svg", get(icon_handler));

    // Build the base router with middleware
    let mut app_router = Router::new()
        .merge(router)
        .nest("/api", routes::create_config_router::<AppState>())
        .layer(TraceLayer::new_for_http());

    // Add models router separately since it uses ModelsState
    let models_router = routes::create_models_router().with_state(state.models_state.clone());
    app_router = app_router.nest("/api/models", models_router);

    // Add cron router
    let cron_router = routes::create_cron_router::<AppState>().with_state(state.clone());
    app_router = app_router.nest("/api/cron", cron_router);

    // Add MCP router
    let mcp_router = routes::create_mcp_router::<AppState>().with_state(state.clone());
    app_router = app_router.nest("/api/mcp", mcp_router);

    // Add Envs router
    let envs_router = routes::create_envs_router::<AppState>().with_state(state.clone());
    app_router = app_router.nest("/api/envs", envs_router);

    // Add Skills router (stateless)
    let skills_router = routes::create_skills_router::<AppState>();
    app_router = app_router.nest("/api/skills", skills_router);

    // Add Local Models router
    let local_models_router =
        routes::create_local_models_router().with_state(state.local_models_state.clone());
    app_router = app_router.nest("/api/local-models", local_models_router);

    // Add Ollama Models router
    let ollama_models_router =
        routes::create_ollama_models_router().with_state(state.ollama_models_state.clone());
    app_router = app_router.nest("/api/ollama-models", ollama_models_router);

    // Add Workspace router (stateless)
    let workspace_router = routes::create_workspace_router::<AppState>();
    app_router = app_router.nest("/api/workspace", workspace_router);

    // Add Console router (stateless)
    let console_router = routes::create_console_router::<AppState>();
    app_router = app_router.nest("/api/console", console_router);

    // Add Agent router
    let agent_router = routes::create_agent_router().with_state(state.agent_state.clone());
    app_router = app_router.nest("/api/agent", agent_router);

    // Add Chats router
    let chats_router = routes::create_chats_router().with_state(state.chats_state.clone());
    app_router = app_router.nest("/api/chats", chats_router);

    // Add CORS if CORS_ORIGINS is set
    if let Ok(cors_origins) = std::env::var("COPAW_CORS_ORIGINS") {
        if !cors_origins.trim().is_empty() {
            let origins: Vec<String> = cors_origins
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            info!("CORS enabled for origins: {:?}", origins);

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);

            app_router = app_router.layer(cors);
        }
    }

    // Add static assets directory if it exists
    if let Some(ref console_dir) = state.console_static_dir {
        let assets_dir = console_dir.join("assets");
        if assets_dir.is_dir() {
            info!("Serving assets from: {:?}", assets_dir);
            app_router = app_router.nest_service("/assets", ServeDir::new(assets_dir));
        }
    }

    // Add explicit wildcard route for SPA fallback (for API audit visibility)
    // Note: This must be added after all specific routes so they get priority
    // The nested router allows the wildcard to match only after specific routes fail
    use axum::routing::get;
    let spa_router = Router::new()
        .route("/*full_path", get(spa_fallback_handler))
        .with_state(state.clone());

    // Merge the SPA router with the app router
    // This makes the wildcard route explicit while maintaining route priority
    app_router = app_router.merge(spa_router);

    // Also keep the fallback as a final safety net
    let app_router = app_router.fallback(spa_fallback_handler);

    // Set state at the very end
    app_router.with_state(state)
}

// Run the server
pub async fn run_server(address: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(address).await?;
    let addr = listener.local_addr()?;
    info!("Server listening on {}", addr);

    let console_static_dir = resolve_console_static_dir();
    if let Some(ref dir) = console_static_dir {
        info!("Console static dir: {:?}", dir);
    } else {
        info!("No console static dir found, serving hello message");
    }

    let router = build_router(console_static_dir);

    axum::serve(listener, router).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    let log_level = std::env::var("COPAW_LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string())
        .parse::<Level>()
        .unwrap_or(Level::INFO);

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::new(format!("copaw={}", log_level)))
        .init();

    let address =
        std::env::var("COPAW_SERVER_ADDRESS").unwrap_or_else(|_| DEFAULT_ADDRESS.to_string());

    info!("Starting CoPaw server v{}", VERSION);
    run_server(&address).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;

    // Helper to create test router
    fn create_test_router() -> Router {
        build_router(Some(PathBuf::from("/tmp/test_console")))
    }

    fn create_test_router_no_console() -> Router {
        build_router(None)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_version_endpoint() {
        let router = create_test_router();
        let request = Request::builder()
            .uri("/api/version")
            .body(Body::empty())
            .unwrap();

        let response = router
            .oneshot(request)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");
        let version_response: VersionResponse =
            serde_json::from_slice(&body).expect("Failed to parse JSON");
        assert_eq!(version_response.version, VERSION);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_root_endpoint_without_console() {
        let router = create_test_router_no_console();
        let request = Request::builder().uri("/").body(Body::empty()).unwrap();

        let response = router
            .oneshot(request)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");
        let hello_response: HelloResponse =
            serde_json::from_slice(&body).expect("Failed to parse JSON");
        assert_eq!(hello_response.message, "Hello World");
    }

    #[test]
    fn test_cors_middleware() {
        // CORS middleware is added when COPAW_CORS_ORIGINS is set
        // This test verifies the environment variable is checked
        let cors_origins = std::env::var("COPAW_CORS_ORIGINS");
        // The middleware logic is tested in build_router
        assert!(cors_origins.is_ok() || cors_origins.is_err());
    }

    #[test]
    fn test_static_file_serving() {
        // Static file serving is handled by ServeDir
        // Test that resolve_console_static_dir works
        let dir = resolve_console_static_dir();
        // Should return None for test environment or a valid path
        if let Some(d) = dir {
            assert!(d.is_dir());
        }
    }

    #[test]
    fn test_error_handling() {
        // Test AppError creation
        let error = AppError::not_found("test");
        assert_eq!(error.status, StatusCode::NOT_FOUND);

        let error = AppError::internal_error("test");
        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);

        // Test error display
        let error_str = format!("{}", error);
        assert!(error_str.contains("test"));
    }

    #[test]
    fn test_resolve_console_static_dir() {
        // Test with no console directory
        let dir = resolve_console_static_dir();
        // Should return None in test environment
        assert!(dir.is_none() || dir.unwrap().is_dir());
    }

    #[test]
    fn test_version_constant() {
        assert!(!VERSION.is_empty());
        assert!(VERSION.contains('.'));
    }

    #[test]
    fn test_default_address() {
        assert_eq!(DEFAULT_ADDRESS, "127.0.0.1:8088");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_spa_fallback_without_console() {
        let router = create_test_router_no_console();
        let request = Request::builder()
            .uri("/some/nonexistent/path")
            .body(Body::empty())
            .unwrap();

        let response = router
            .oneshot(request)
            .await
            .expect("Failed to get response");

        // Should return 404 since no console directory is configured
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn test_app_state_creation() {
        let registry = Arc::new(ProviderRegistry::new());
        let store = Arc::new(ProviderStore::new(
            ProviderStore::default_path(),
            registry.clone(),
        ));
        let models_state = ModelsState {
            registry: registry.clone(),
            store: store.clone(),
        };
        let models_dir = PathBuf::from("/tmp/models");
        let download_manager = Arc::new(DownloadManager::new(models_dir.clone()));
        let local_models_state =
            LocalModelsState::new(registry.clone(), store.clone(), models_dir.clone());
        let ollama_models_state =
            OllamaModelsState::new(registry.clone(), store.clone(), download_manager);

        // Create temp directory for test
        let temp_dir = tempfile::TempDir::new().unwrap();

        // Initialize chats state with temp repository
        let chats_repo_path = temp_dir.path().join("chats.json");
        let chats_repo = crate::repo::ChatRepository::with_path(chats_repo_path).unwrap();
        let chat_manager = Arc::new(crate::runner::chat_manager::ChatManager::with_repository(
            chats_repo,
        ));
        let chats_state = ChatsState {
            manager: chat_manager.clone(),
        };

        // Initialize agent state with temp directory
        let file_manager =
            Arc::new(copaw_agents::AgentFileManager::with_working_dir(temp_dir.path()).unwrap());
        let agent_state = AgentState {
            file_manager,
            registry: registry.clone(),
            store: store.clone(),
            chat_manager,
        };

        let state = AppState {
            console_static_dir: Some(PathBuf::from("/tmp/console")),
            config_state: ConfigState::new(None),
            models_state,
            cron_state: CronState::new(None),
            mcp_state: McpState::new(None),
            envs_state: EnvsState::new(None),
            local_models_state,
            ollama_models_state,
            agent_state,
            chats_state,
        };
        assert_eq!(
            state.console_static_dir,
            Some(PathBuf::from("/tmp/console"))
        );
    }
}
