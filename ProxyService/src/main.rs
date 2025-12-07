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
use std::time::Duration;
use tokio::time::interval;
use tower_http::cors::{Any, CorsLayer};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
    auth_token: String,
    start_time: std::time::Instant,
}

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
    let client = reqwest::Client::new();

    if let Ok(response) = client
        .get(&format!("{}/list", state.main_server_url))
        .bearer_auth(&state.auth_token)
        .header(reqwest::header::USER_AGENT, "RetListerProxy/1.0")
        .send()
        .await
    {
        if let Ok(data) = response.json::<Vec<Resto>>().await {
            let db = state.db.clone();
            let restos_clone = data.clone();
            tokio::spawn(async move {
                if let Err(e) = warm_cache(&db, &restos_clone).await {
                    tracing::warn!("Cache warming failed: {}", e);
                }
            });
            return Ok(Json(data));
        }
    }

    let restos = sqlx::query_as::<_, Resto>("SELECT * FROM restos ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(restos))
}

async fn warm_cache(db: &SqlitePool, restos: &[Resto]) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;

    sqlx::query(
        "DELETE FROM restos WHERE id NOT IN (SELECT resto_id FROM sync_queue WHERE synced = 0)",
    )
    .execute(&mut *tx)
    .await?;

    for resto in restos {
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
        .execute(&mut *tx)
        .await?;
    }

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT OR REPLACE INTO sync_metadata (key, value) VALUES ('last_cache_warm', ?1)")
        .bind(&now)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    tracing::debug!("Cache warmed with {} restos", restos.len());
    Ok(())
}
async fn add_resto(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddRestoRequest>,
) -> Result<Json<Resto>, StatusCode> {
    let client = reqwest::Client::new();

    if let Ok(response) = client
        .post(&format!("{}/add", state.main_server_url))
        .bearer_auth(&state.auth_token)
        .header(reqwest::header::USER_AGENT, "RetListerProxy/1.0")
        .json(&payload)
        .send()
        .await
    {
        if let Ok(resto) = response.json::<Resto>().await {
            let _ = save_to_local_db(&state.db, &resto).await;
            return Ok(Json(resto));
        }
    }

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

    if client
        .delete(&format!("{}/remove/{}", state.main_server_url, id))
        .bearer_auth(&state.auth_token)
        .header(reqwest::header::USER_AGENT, "RetListerProxy/1.0")
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

async fn proxy_delete_batch(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DeleteBatchRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let client = reqwest::Client::new();

    let response = client
        .post(&format!("{}/delete_batch", state.main_server_url))
        .json(&req)
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            for id in &req.ids {
                let _ = sqlx::query("DELETE FROM restos WHERE id = ?")
                    .bind(id)
                    .execute(&state.db)
                    .await;
            }
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

async fn search_resto(
    State(state): State<Arc<AppState>>,
    Json(params): Json<SearchRestoRequest>,
) -> Result<Json<Vec<Resto>>, StatusCode> {
    let client = reqwest::Client::new();

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
        .bearer_auth(&state.auth_token)
        .header(reqwest::header::USER_AGENT, "RetListerProxy/1.0")
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
            payload TEXT,
            timestamp TEXT NOT NULL,
            synced INTEGER DEFAULT 0,
            retry_count INTEGER DEFAULT 0,
            last_error TEXT
        )",
    )
    .execute(&pool)
    .await?;

    let _ = sqlx::query("ALTER TABLE sync_queue ADD COLUMN payload TEXT")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE sync_queue ADD COLUMN retry_count INTEGER DEFAULT 0")
        .execute(&pool)
        .await;
    let _ = sqlx::query("ALTER TABLE sync_queue ADD COLUMN last_error TEXT")
        .execute(&pool)
        .await;

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

async fn sync_loop(state: Arc<AppState>) {
    let mut ticker = interval(Duration::from_secs(30));

    loop {
        ticker.tick().await;

        let client = reqwest::Client::new();
        let health_check = client
            .get(&format!("{}/health", state.main_server_url))
            .bearer_auth(&state.auth_token)
            .header(reqwest::header::USER_AGENT, "RetListerProxy/1.0")
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        if health_check.is_err() || !health_check.unwrap().status().is_success() {
            tracing::debug!("Main server unreachable, skipping sync");
            continue;
        }

        let pending: Vec<SyncQueueItem> = match sqlx::query_as(
            "SELECT id, operation, resto_id, payload, timestamp, retry_count 
             FROM sync_queue 
             WHERE synced = 0 AND retry_count < 3
             ORDER BY timestamp ASC 
             LIMIT 50",
        )
        .fetch_all(&state.db)
        .await
        {
            Ok(items) => items,
            Err(e) => {
                tracing::error!("Failed to fetch sync queue: {}", e);
                continue;
            }
        };

        if pending.is_empty() {
            continue;
        }

        tracing::info!("Processing {} pending sync operations", pending.len());

        for item in pending {
            let result = process_sync_item(&state, &client, &item).await;

            match result {
                Ok(()) => {
                    let _ = sqlx::query("UPDATE sync_queue SET synced = 1 WHERE id = ?1")
                        .bind(item.id)
                        .execute(&state.db)
                        .await;
                    tracing::debug!(
                        "Synced operation {} for resto {}",
                        item.operation,
                        item.resto_id
                    );
                }
                Err(e) => {
                    let _ = sqlx::query(
                        "UPDATE sync_queue SET retry_count = retry_count + 1, last_error = ?1 WHERE id = ?2"
                    )
                    .bind(e.to_string())
                    .bind(item.id)
                    .execute(&state.db)
                    .await;
                    tracing::warn!(
                        "Sync failed for {} resto {}: {}",
                        item.operation,
                        item.resto_id,
                        e
                    );
                }
            }
        }

        let now = chrono::Utc::now().to_rfc3339();
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO sync_metadata (key, value) VALUES ('last_sync_time', ?1)",
        )
        .bind(&now)
        .execute(&state.db)
        .await;

        let _ = sqlx::query(
            "DELETE FROM sync_queue WHERE synced = 1 AND id NOT IN (
                SELECT id FROM sync_queue WHERE synced = 1 ORDER BY timestamp DESC LIMIT 1000
            )",
        )
        .execute(&state.db)
        .await;
    }
}

#[derive(sqlx::FromRow)]
struct SyncQueueItem {
    id: i64,
    operation: String,
    resto_id: i64,
    payload: Option<String>,
    timestamp: String,
    retry_count: i32,
}

async fn process_sync_item(
    state: &Arc<AppState>,
    client: &reqwest::Client,
    item: &SyncQueueItem,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match item.operation.as_str() {
        "INSERT" => {
            let resto: Option<Resto> = sqlx::query_as("SELECT * FROM restos WHERE id = ?1")
                .bind(item.resto_id)
                .fetch_optional(&state.db)
                .await?;

            if let Some(r) = resto {
                let payload = AddRestoRequest {
                    width_mm: r.width_mm,
                    height_mm: r.height_mm,
                    thickness_mm: r.thickness_mm,
                    material: r.material,
                    notes: r.notes,
                };

                let response = client
                    .post(&format!("{}/add", state.main_server_url))
                    .bearer_auth(&state.auth_token)
                    .header(reqwest::header::USER_AGENT, "RetListerProxy/1.0")
                    .json(&payload)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    return Err(format!("Server returned {}", response.status()).into());
                }

                if let Ok(new_resto) = response.json::<Resto>().await {
                    if new_resto.id != item.resto_id {
                        sqlx::query("UPDATE restos SET id = ?1 WHERE id = ?2")
                            .bind(new_resto.id)
                            .bind(item.resto_id)
                            .execute(&state.db)
                            .await?;
                    }
                }
            }
        }
        "DELETE" => {
            let response = client
                .delete(&format!(
                    "{}/remove/{}",
                    state.main_server_url, item.resto_id
                ))
                .bearer_auth(&state.auth_token)
                .header(reqwest::header::USER_AGENT, "RetListerProxy/1.0")
                .send()
                .await?;

            if !response.status().is_success()
                && response.status() != reqwest::StatusCode::NOT_FOUND
            {
                return Err(format!("Server returned {}", response.status()).into());
            }
        }
        _ => {
            tracing::warn!("Unknown sync operation: {}", item.operation);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("proxy_service=info".parse().unwrap()),
        )
        .init();

    let db = init_db().await.expect("Failed to initialize database");

    let auth_token = std::env::var("RETLISTER_API_TOKEN")
        .expect("RETLISTER_API_TOKEN environment variable must be set");

    let state = Arc::new(AppState {
        db,
        main_server_url: "https://api.faky.dev".to_string(),
        auth_token,
        start_time: std::time::Instant::now(),
    });

    let sync_state = Arc::clone(&state);
    tokio::spawn(async move {
        sync_loop(sync_state).await;
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
        .route("/delete_batch", post(proxy_delete_batch)) // NEW
        .route("/search", post(search_resto)) // CHANGED to POST
        .route("/optimize_cuts", post(proxy_optimize_cuts))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8001")
        .await
        .expect("Failed to bind to port 8001");

    println!("   Proxy Service running on http://0.0.0.0:8001");
    println!("   Main server: https://api.faky.dev");
    println!("   Background sync: enabled (every 30s)");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}
