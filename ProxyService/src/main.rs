use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

// ===== DATA STRUCTURES =====

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct Resto {
    id: i64,
    width_mm: i64,
    height_mm: i64,
    thickness_mm: i64,
    material: String,
    notes: Option<String>,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AddRestoRequest {
    width_mm: i64,
    height_mm: i64,
    thickness_mm: i64,
    material: String,
    notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchRestoRequest {
    width_mm: i64,
    height_mm: i64,
    thickness_mm: i64,
    material: String,
}

// New struct for batch deletion
#[derive(Debug, Serialize, Deserialize)]
struct DeleteBatchRequest {
    ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    proxy_active: bool,
    main_server_active: bool,
    db_path: String,
    uptime_seconds: u64,
}

#[derive(Debug, Serialize)]
struct SyncStatus {
    last_sync: Option<String>,
    pending_changes: i64,
}

struct AppState {
    db: SqlitePool,
    main_server_url: String,
    start_time: std::time::Instant,
}

// ===== HANDLERS =====

async fn health_check(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let main_server_active = reqwest::get(&format!("{}/list", state.main_server_url))
        .await
        .is_ok();

    Json(HealthResponse {
        proxy_active: true,
        main_server_active,
        db_path: "data/proxy.db".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
    })
}

async fn sync_status(State(state): State<Arc<AppState>>) -> Result<Json<SyncStatus>, StatusCode> {
    let last_sync = sqlx::query_scalar::<_, String>(
        "SELECT value FROM sync_metadata WHERE key = 'last_sync_time' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let pending = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sync_queue WHERE synced = 0")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    Ok(Json(SyncStatus {
        last_sync,
        pending_changes: pending,
    }))
}

async fn list_restos(State(state): State<Arc<AppState>>) -> Result<Json<Vec<Resto>>, StatusCode> {
    // Try Main Server
    if let Ok(response) = reqwest::get(&format!("{}/list", state.main_server_url)).await {
        if let Ok(data) = response.json::<Vec<Resto>>().await {
            return Ok(Json(data));
        }
    }

    // Fallback to Local DB
    let restos = sqlx::query_as::<_, Resto>("SELECT * FROM restos ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(restos))
}

async fn add_resto(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddRestoRequest>,
) -> Result<Json<Resto>, StatusCode> {
    let client = reqwest::Client::new();

    // Try Main Server
    if let Ok(response) = client
        .post(&format!("{}/add", state.main_server_url))
        .json(&payload)
        .send()
        .await
    {
        if let Ok(resto) = response.json::<Resto>().await {
            // Update local cache
            let _ = save_to_local_db(&state.db, &resto).await;
            return Ok(Json(resto));
        }
    }

    // Offline Mode
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query_as::<_, Resto>(
        "INSERT INTO restos (width_mm, height_mm, thickness_mm, material, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         RETURNING *",
    )
    .bind(payload.width_mm)
    .bind(payload.height_mm)
    .bind(payload.thickness_mm)
    .bind(&payload.material)
    .bind(&payload.notes)
    .bind(&now)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _ = queue_sync_operation(&state.db, "INSERT", result.id).await;

    Ok(Json(result))
}

async fn remove_resto(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    let client = reqwest::Client::new();
    
    // Try Main Server
    if client
        .delete(&format!("{}/remove/{}", state.main_server_url, id))
        .send()
        .await
        .is_ok()
    {
        let _ = sqlx::query("DELETE FROM restos WHERE id = ?1")
            .bind(id)
            .execute(&state.db)
            .await;
        return Ok(StatusCode::OK);
    }

    // Offline Mode
    let rows = sqlx::query("DELETE FROM restos WHERE id = ?1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .rows_affected();

    if rows == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    let _ = queue_sync_operation(&state.db, "DELETE", id).await;
    Ok(StatusCode::OK)
}

// --- NEW HANDLER: PROXY DELETE BATCH ---
async fn proxy_delete_batch(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DeleteBatchRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = reqwest::Client::new();

    // 1. Forward to Main Server
    let response = client
        .post(&format!("{}/delete_batch", state.main_server_url))
        .json(&req)
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            // Success on main server -> Sync local DB (delete items from cache)
            for id in &req.ids {
                let _ = sqlx::query("DELETE FROM restos WHERE id = ?")
                    .bind(id)
                    .execute(&state.db)
                    .await;
            }
            // Return whatever the main server returned
            let data = resp.json::<serde_json::Value>().await.unwrap_or_default();
            Ok(Json(data))
        }
        _ => {
            // If main server is unreachable, we return error for now.
            // (Implementing offline batch delete queueing is complex, 
            // so we force online for batch ops to ensure consistency)
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

// --- UPDATED HANDLER: SEARCH (POST) ---
async fn search_resto(
    State(state): State<Arc<AppState>>,
    Json(params): Json<SearchRestoRequest>,
) -> Result<Json<Vec<Resto>>, StatusCode> {
    let client = reqwest::Client::new();

    // Try Main Server (POST)
    if let Ok(response) = client
        .post(&format!("{}/search", state.main_server_url))
        .json(&params)
        .send()
        .await
    {
        if let Ok(data) = response.json::<Vec<Resto>>().await {
            return Ok(Json(data));
        }
    }

    // Offline Mode: Local Search
    let restos = sqlx::query_as::<_, Resto>(
        "SELECT * FROM restos 
         WHERE width_mm >= ?1 
           AND height_mm >= ?2
           AND thickness_mm = ?3
           AND LOWER(material) = LOWER(?4)
         ORDER BY (width_mm * height_mm) ASC",
    )
    .bind(params.width_mm)
    .bind(params.height_mm)
    .bind(params.thickness_mm)
    .bind(&params.material)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(restos))
}

async fn proxy_optimize_cuts(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = reqwest::Client::new();
    let response = client
        .post(&format!("{}/optimize_cuts", state.main_server_url))
        .json(&payload)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !response.status().is_success() {
        return Err(StatusCode::from_u16(response.status().as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR));
    }

    let data = response
        .json::<serde_json::Value>()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(data))
}

// ===== DB HELPERS =====

async fn save_to_local_db(db: &SqlitePool, resto: &Resto) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO restos (id, width_mm, height_mm, thickness_mm, material, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
    )
    .bind(resto.id)
    .bind(resto.width_mm)
    .bind(resto.height_mm)
    .bind(resto.thickness_mm)
    .bind(&resto.material)
    .bind(&resto.notes)
    .bind(&resto.created_at)
    .execute(db)
    .await?;
    Ok(())
}

async fn queue_sync_operation(
    db: &SqlitePool,
    operation: &str,
    resto_id: i64,
) -> Result<(), sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO sync_queue (operation, resto_id, timestamp, synced)
         VALUES (?1, ?2, ?3, 0)",
    )
    .bind(operation)
    .bind(resto_id)
    .bind(&now)
    .execute(db)
    .await?;
    Ok(())
}

async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    std::fs::create_dir_all("data").map_err(|e| sqlx::Error::Io(e))?;

    let pool = SqlitePool::connect("sqlite:data/proxy.db").await?;

    sqlx::query("PRAGMA journal_mode = WAL;")
        .execute(&pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS restos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            width_mm INTEGER NOT NULL,
            height_mm INTEGER NOT NULL,
            thickness_mm INTEGER NOT NULL,
            material TEXT NOT NULL,
            notes TEXT,
            created_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sync_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            operation TEXT NOT NULL,
            resto_id INTEGER NOT NULL,
            timestamp TEXT NOT NULL,
            synced INTEGER DEFAULT 0
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sync_metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

// ===== MAIN =====

#[tokio::main]
async fn main() {
    // Initialize tracing if you want logs
    // tracing_subscriber::fmt::init();

    let db = init_db().await.expect("Failed to initialize database");

    let state = Arc::new(AppState {
        db,
        main_server_url: "http://localhost:8000".to_string(),
        start_time: std::time::Instant::now(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/sync/status", get(sync_status))
        .route("/list", get(list_restos))
        .route("/add", post(add_resto))
        .route("/remove/:id", delete(remove_resto))
        // --- ROUTES UPDATED HERE ---
        .route("/delete_batch", post(proxy_delete_batch)) // NEW
        .route("/search", post(search_resto)) // CHANGED to POST
        .route("/optimize_cuts", post(proxy_optimize_cuts))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8001")
        .await
        .expect("Failed to bind to port 8001");

    println!("🔄 Proxy Service running on http://0.0.0.0:8001");
    println!("   Main server: http://localhost:8000");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}