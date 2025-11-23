use axum::{
    extract::{Path, Query, State, Request},
    http::{StatusCode, HeaderMap},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, Pool, Sqlite};
use std::net::SocketAddr;
use std::fs;
use std::time::Duration;
use time::OffsetDateTime;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{TraceLayer, DefaultMakeSpan, DefaultOnResponse};
use tower_http::LatencyUnit;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cutting_optimizer;
use cutting_optimizer::optimize_cuts;

// TODO: Big consideration, but maybe add cut grain direction later.

// Validation constants
const MIN_DIMENSION: i64 = 1;
const MAX_DIMENSION: i64 = 10000;
const MAX_THICKNESS: i64 = 1000;
const MIN_MATERIAL_LEN: usize = 1;
const MAX_MATERIAL_LEN: usize = 64;
const MAX_NOTES_LEN: usize = 256;
const SCHEMA_VERSION: i64 = 1;

#[derive(Clone)]
struct AppState {
    db: Pool<Sqlite>,
    auth_token: Option<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

enum AppError {
    Validation(String),
    NotFound(String),
    Database(sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, details) = match self {
            AppError::Validation(msg) => {
                tracing::warn!(error = %msg, "Validation error");
                (StatusCode::BAD_REQUEST, "Validation error".to_string(), Some(msg))
            },
            AppError::NotFound(msg) => {
                tracing::debug!(error = %msg, "Resource not found");
                (StatusCode::NOT_FOUND, "Not found".to_string(), Some(msg))
            },
            AppError::Database(err) => {
                tracing::error!("Database error: {}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), None)
            }
        };
        
        let body = Json(ErrorResponse { error, details });
        (status, body).into_response()
    }
}

fn validate_dimensions(width: i64, height: i64, thickness: i64) -> Result<(), AppError> {
    if width < MIN_DIMENSION || width > MAX_DIMENSION {
        return Err(AppError::Validation(format!("Width must be between {} and {} mm", MIN_DIMENSION, MAX_DIMENSION)));
    }
    if height < MIN_DIMENSION || height > MAX_DIMENSION {
        return Err(AppError::Validation(format!("Height must be between {} and {} mm", MIN_DIMENSION, MAX_DIMENSION)));
    }
    if thickness < MIN_DIMENSION || thickness > MAX_THICKNESS {
        return Err(AppError::Validation(format!("Thickness must be between {} and {} mm", MIN_DIMENSION, MAX_THICKNESS)));
    }
    Ok(())
}

fn validate_van_dimensions(length: i64, width: i64, height: i64) -> Result<(), AppError> {
    if length < MIN_DIMENSION || length > MAX_DIMENSION {
        return Err(AppError::Validation(format!("Length must be between {} and {} mm", MIN_DIMENSION, MAX_DIMENSION)));
    }
    if width < MIN_DIMENSION || width > MAX_DIMENSION {
        return Err(AppError::Validation(format!("Width must be between {} and {} mm", MIN_DIMENSION, MAX_DIMENSION)));
    }
    if height < MIN_DIMENSION || height > MAX_DIMENSION {
        return Err(AppError::Validation(format!("Height must be between {} and {} mm", MIN_DIMENSION, MAX_DIMENSION)));
    }
    Ok(())
}

fn validate_material(material: &str) -> Result<(), AppError> {
    let len = material.len();
    if len < MIN_MATERIAL_LEN || len > MAX_MATERIAL_LEN {
        return Err(AppError::Validation(format!("Material must be between {} and {} characters", MIN_MATERIAL_LEN, MAX_MATERIAL_LEN)));
    }
    if material.trim().is_empty() {
        return Err(AppError::Validation("Material cannot be empty or whitespace only".to_string()));
    }
    Ok(())
}

fn validate_notes(notes: &Option<String>) -> Result<(), AppError> {
    if let Some(n) = notes {
        if n.len() > MAX_NOTES_LEN {
            return Err(AppError::Validation(format!("Notes must not exceed {} characters", MAX_NOTES_LEN)));
        }
    }
    Ok(())
}

async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(required_token) = &state.auth_token else {
        return Ok(next.run(request).await);
    };
    
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..]; // Skip "Bearer "
            if token == required_token {
                Ok(next.run(request).await)
            } else {
                tracing::warn!("Authentication failed: invalid token");
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        Some(_) => {
            tracing::warn!("Authentication failed: malformed Authorization header");
            Err(StatusCode::UNAUTHORIZED)
        }
        None => {
            tracing::warn!("Authentication failed: missing Authorization header");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

#[derive(Serialize, FromRow)]
struct Leftover {
    id: i64,
    width_mm: i64,
    height_mm: i64,
    thickness_mm: i64,
    material: String,
    notes: Option<String>,
    created_at: String,
}

#[derive(Deserialize)]
struct AddLeftoverRequest {
    width_mm: i64,
    height_mm: i64,
    thickness_mm: i64,
    material: String,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct UpdateLeftoverRequest {
    width_mm: Option<i64>,
    height_mm: Option<i64>,
    thickness_mm: Option<i64>,
    material: Option<String>,
    notes: Option<String>,
}

#[derive(Serialize)]
struct AddLeftoverResponse {
    id: i64,
}

// Van and Cargo structures
#[derive(Serialize, FromRow)]
struct Van {
    id: i64,
    name: String,
    length_mm: i64,
    width_mm: i64,
    height_mm: i64,
    max_weight_kg: Option<i64>,
    wheel_well_height_mm: Option<i64>,
    wheel_well_width_mm: Option<i64>,
    wheel_well_start_x_mm: Option<i64>,
    active: bool,
    notes: Option<String>,
    created_at: String,
}

#[derive(Deserialize)]
struct AddVanRequest {
    name: String,
    length_mm: i64,
    width_mm: i64,
    height_mm: i64,
    max_weight_kg: Option<i64>,
    wheel_well_height_mm: Option<i64>,
    wheel_well_width_mm: Option<i64>,
    wheel_well_start_x_mm: Option<i64>,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct UpdateVanRequest {
    name: Option<String>,
    length_mm: Option<i64>,
    width_mm: Option<i64>,
    height_mm: Option<i64>,
    max_weight_kg: Option<i64>,
    wheel_well_height_mm: Option<i64>,
    wheel_well_width_mm: Option<i64>,
    wheel_well_start_x_mm: Option<i64>,
    notes: Option<String>,
    active: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone)]
struct CargoItem {
    description: String,
    length_mm: i64,
    width_mm: i64,
    height_mm: i64,
    weight_kg: f64,
    fragile: bool,
    rotation_allowed: bool,
    stackable: bool,
    color: Option<String>,
}

// Optimize request
#[derive(Deserialize)]
struct OptimizeRequest {
    van_id: i64,
    items: Vec<CargoItem>,
}

// Positioned item in 3D space
#[derive(Serialize, Clone)]
struct PositionedItem {
    item: CargoItem,
    position: Position3D,
    rotation: Rotation3D,
    level: i32,
}

#[derive(Serialize, Clone)]
struct Position3D {
    x: i64,
    y: i64,
    z: i64,
}

#[derive(Serialize, Clone)]
struct Rotation3D {
    x: i64, // 0, 90, 180, 270 degrees
    y: i64,
    z: i64,
}

#[derive(Serialize)]
struct OptimizeResponse {
    success: bool,
    plan: Option<LoadingPlan>,
    warnings: Vec<String>,
}

#[derive(Serialize, Clone)]
struct LoadingPlan {
    items: Vec<PositionedItem>,
    total_weight: f64,
    utilization_percent: f64,
    van_volume: i64,
    used_volume: i64,
}

#[derive(Serialize, FromRow)]
struct MaterialStats {
    material: String,
    count: i64,
    total_area_mm2: i64,
}

#[derive(Serialize, FromRow)]
struct ThicknessStats {
    thickness_mm: i64,
    count: i64,
}

#[derive(Serialize)]
struct StatsResponse {
    total_count: i64,
    total_area_mm2: i64,
    by_material: Vec<MaterialStats>,
    by_thickness: Vec<ThicknessStats>,
}

#[derive(Deserialize)]
struct SearchQuery {
    width_mm: i64,
    height_mm: i64,
    thickness_mm: i64,
    material: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("SERVER_LOG")
                .unwrap_or_else(|_| "retlister-server=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let _ = fs::create_dir_all("data");

    let db_path = std::env::current_dir()?.join("data").join("retlister.db");
    let connection_string = format!("sqlite://{}", db_path.display());
    
    tracing::info!("Connecting to database: {}", connection_string);

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&connection_string)
        .await?;

    // Configure SQLite for safety and performance
    sqlx::query("PRAGMA journal_mode=WAL;").execute(&db).await?;
    sqlx::query("PRAGMA foreign_keys=ON;").execute(&db).await?;
    sqlx::query("PRAGMA busy_timeout=5000;").execute(&db).await?; // 5 second timeout for locks

    // Check database integrity before proceeding
    tracing::info!("Running database integrity check...");
    let integrity_result: (String,) = sqlx::query_as("PRAGMA integrity_check;")
        .fetch_one(&db)
        .await?;
    
    if integrity_result.0 != "ok" {
        tracing::error!("Database integrity check failed: {}", integrity_result.0);
        anyhow::bail!(
            "Database corruption detected!\n\nIntegrity check result: {}\n\nAction required:\n\
             1. Stop the server\n\
             2. Restore from backup (see data/backups/)\n\
             3. If no backup exists, consider running: sqlite3 data/retlister.db '.recover' > recovered.sql",
            integrity_result.0
        );
    }
    tracing::info!("Database integrity check passed");

    sqlx::migrate!("./Migrations").run(&db).await?;
    
    // Verify schema version
    let current_version: Option<(i64,)> = sqlx::query_as(
        "SELECT version FROM schema_metadata ORDER BY version DESC LIMIT 1"
    )
    .fetch_optional(&db)
    .await?;
    
    match current_version {
        Some((version,)) => {
            if version != SCHEMA_VERSION {
                tracing::warn!(
                    "Schema version mismatch: expected {}, found {}. Migrations may be needed.",
                    SCHEMA_VERSION, version
                );
            } else {
                tracing::info!("Schema version {} verified", version);
            }
        }
        None => {
            // First run, insert schema version
            sqlx::query("INSERT INTO schema_metadata (version, applied_at) VALUES (?1, CURRENT_TIMESTAMP)")
                .bind(SCHEMA_VERSION)
                .execute(&db)
                .await
                .ok(); // Ignore error if table doesn't exist yet (will be created by migration)
            tracing::info!("Initialized schema version {}", SCHEMA_VERSION);
        }
    }

    let auth_token = std::env::var("AUTH_TOKEN").ok();
    if let Some(ref token) = auth_token {
        tracing::info!("Authentication enabled (token length: {})", token.len());
        tracing::warn!("🔒 All endpoints except /health require Bearer token authentication");
    } else {
        tracing::warn!("⚠️  Authentication disabled - set AUTH_TOKEN env var to enable");
    }

    let state = AppState { db, auth_token };

    // Leftover <=> Resto
    let app = Router::new()
        .route("/health", get(health_check))  // Public endpoint
        .route("/ready", get(readiness_check))
        .route("/add", post(add_resto))
        .route("/remove/:id", delete(remove_resto))
        .route("/delete_batch", post(delete_batch))
        .route("/update/:id", post(update_resto))
        .route("/search", get(search_resto))
        .route("/list", get(list_restos))
        .route("/stats", get(get_stats))
        // Van management
        .route("/vans", get(list_vans).post(add_van))
        .route("/vans/:id", get(get_van).post(update_van).delete(delete_van))
        // Optimize loading (cargo items sent in request body)
        .route("/optimize", post(optimize_loading))
        // Optimize cutting (cutting list sent in request body)
        .route("/optimize_cuts", post(optimize_cuts))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware))  // Apply auth to all routes above
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                        .on_response(DefaultOnResponse::new().level(tracing::Level::INFO).latency_unit(LatencyUnit::Millis))
                )
                .layer(TimeoutLayer::new(Duration::from_secs(30))) // 30s request timeout
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods(Any)
                        .allow_headers(Any),
                )
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("Socket on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}

async fn add_resto(
    State(state): State<AppState>,
    Json(payload): Json<AddLeftoverRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Comprehensive validation
    validate_dimensions(payload.width_mm, payload.height_mm, payload.thickness_mm)?;
    validate_material(&payload.material)?;
    validate_notes(&payload.notes)?;

    let created_at = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();

    let result = sqlx::query(
        r#"
        INSERT INTO leftovers (width_mm, height_mm, thickness_mm, material, notes, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(payload.width_mm)
    .bind(payload.height_mm)
    .bind(payload.thickness_mm)
    .bind(&payload.material)
    .bind(&payload.notes)
    .bind(created_at)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;

    let id = result.last_insert_rowid();
    
    tracing::info!(
        id = id,
        width = payload.width_mm,
        height = payload.height_mm,
        thickness = payload.thickness_mm,
        material = %payload.material,
        "Added new resto"
    );

    Ok((StatusCode::CREATED, Json(AddLeftoverResponse { id })))
}

async fn remove_resto(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let res = sqlx::query("DELETE FROM leftovers WHERE id = ?1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;

    if res.rows_affected() == 0 {
        tracing::warn!(id = id, "Attempted to remove non-existent resto");
        return Err(AppError::NotFound(format!("No resto with id {}", id)));
    }

    tracing::info!(id = id, "Removed resto");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct DeleteBatchRequest {
    ids: Vec<i64>,
}

async fn delete_batch(
    State(state): State<AppState>,
    Json(req): Json<DeleteBatchRequest>,
) -> Result<impl IntoResponse, AppError> {
    if req.ids.is_empty() {
        return Ok(Json("No IDs provided".to_string()));
    }

    let query = format!(
        "DELETE FROM leftovers WHERE id IN ({})",
        req.ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ")
    );

    let mut q = sqlx::query(&query);
    for id in req.ids {
        q = q.bind(id);
    }

    let result = q.execute(&state.db).await.map_err(AppError::Database)?;
    
    tracing::info!("Batch deleted {} records", result.rows_affected());
    Ok(Json(format!("Deleted {} records", result.rows_affected())))
}

// TODO: Maybe not a full depth search, what if the worker just wants all the restos from material X and thickness Y?
// For now full depth search, but i could see a worker wanting to search my material only and see what thicknesses are available 
// to get an idea of what to cut & available sizes, i should bring this up later
// TODO: Pagination too, just in case the database gets huge (unlikely but i might enterprise this later)
// Also TODO: Limit maximum results, might overload the server and the worker with the information
// Why this query give so much work, i HATE IT!
// Also TODO: Order the query results (might be better on the worker side but still)
// Maybe order by closeness of size? Other parameters are useless but maybe big bonus if description matching?
// so much to think about
async fn search_resto(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Validate search parameters
    validate_dimensions(params.width_mm, params.height_mm, params.thickness_mm)?;
    validate_material(&params.material)?;

    let required_area = params.width_mm * params.height_mm;

    let candidates: Vec<Leftover> = sqlx::query_as(
        r#"
        SELECT id, width_mm, height_mm, thickness_mm, material, notes, created_at
        FROM leftovers
        WHERE width_mm >= ?1
          AND height_mm >= ?2
          AND thickness_mm = ?3
          AND LOWER(material) = LOWER(?4)
        "#,
    )
    .bind(params.width_mm)
    .bind(params.height_mm)
    .bind(params.thickness_mm)
    .bind(&params.material)
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Database)?;

    tracing::debug!(
        width = params.width_mm,
        height = params.height_mm,
        thickness = params.thickness_mm,
        material = %params.material,
        candidates = candidates.len(),
        "Search executed"
    );

    if candidates.is_empty() {
        return Err(AppError::NotFound(format!("No matching restos found for material '{}' with dimensions {}x{}x{} mm", params.material, params.width_mm, params.height_mm, params.thickness_mm)));
    }

    let best_match = candidates
        .into_iter()
        .min_by_key(|leftover| {
            let leftover_area = leftover.width_mm * leftover.height_mm;
            leftover_area - required_area
        })
        .unwrap();

    tracing::info!(
        search_dimensions = format!("{}x{}x{}", params.width_mm, params.height_mm, params.thickness_mm),
        match_id = best_match.id,
        match_dimensions = format!("{}x{}x{}", best_match.width_mm, best_match.height_mm, best_match.thickness_mm),
        "Found best match"
    );

    Ok(Json(best_match))
}

async fn list_restos(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let restos: Vec<Leftover> = sqlx::query_as(
        r#"
        SELECT id, width_mm, height_mm, thickness_mm, material, notes, created_at
        FROM leftovers
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Database)?;

    tracing::debug!(count = restos.len(), "Listed restos");
    Ok(Json(restos))
}

async fn update_resto(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateLeftoverRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Check if exists first
    let existing: Option<Leftover> = sqlx::query_as(
        "SELECT id, width_mm, height_mm, thickness_mm, material, notes, created_at FROM leftovers WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(AppError::Database)?;

    let resto = existing.ok_or_else(|| AppError::NotFound(format!("Resto with id {} not found", id)))?;

    // Apply updates with defaults from existing
    let new_width = payload.width_mm.unwrap_or(resto.width_mm);
    let new_height = payload.height_mm.unwrap_or(resto.height_mm);
    let new_thickness = payload.thickness_mm.unwrap_or(resto.thickness_mm);
    let new_material = payload.material.unwrap_or(resto.material);
    let new_notes = payload.notes.or(resto.notes);

    // Validate final values
    validate_dimensions(new_width, new_height, new_thickness)?;
    validate_material(&new_material)?;
    validate_notes(&new_notes)?;

    sqlx::query(
        r#"
        UPDATE leftovers
        SET width_mm = ?1, height_mm = ?2, thickness_mm = ?3, material = ?4, notes = ?5
        WHERE id = ?6
        "#,
    )
    .bind(new_width)
    .bind(new_height)
    .bind(new_thickness)
    .bind(&new_material)
    .bind(&new_notes)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;

    tracing::info!(
        id = id,
        dimensions = format!("{}x{}x{}", new_width, new_height, new_thickness),
        material = %new_material,
        "Updated resto"
    );

    Ok(StatusCode::OK)
}

async fn get_stats(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    // Total count and area
    let total: (i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*), COALESCE(SUM(width_mm * height_mm), 0)
        FROM leftovers
        "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(AppError::Database)?;

    let (total_count, total_area_mm2) = total;

    // By material
    let by_material: Vec<MaterialStats> = sqlx::query_as(
        r#"
        SELECT material, COUNT(*) as count, SUM(width_mm * height_mm) as total_area_mm2
        FROM leftovers
        GROUP BY material
        ORDER BY count DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Database)?;

    // By thickness
    let by_thickness: Vec<ThicknessStats> = sqlx::query_as(
        r#"
        SELECT thickness_mm, COUNT(*) as count
        FROM leftovers
        GROUP BY thickness_mm
        ORDER BY thickness_mm
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Database)?;

    tracing::debug!(
        total_count = total_count,
        total_area_mm2 = total_area_mm2,
        material_groups = by_material.len(),
        thickness_groups = by_thickness.len(),
        "Retrieved stats"
    );

    Ok(Json(StatsResponse {
        total_count,
        total_area_mm2,
        by_material,
        by_thickness,
    }))
}

// ===== VAN ENDPOINTS =====

async fn list_vans(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    tracing::info!("Listing vans");
    let vans = sqlx::query_as::<_, Van>("SELECT * FROM vans WHERE active = 1 ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?;
    
    Ok(Json(vans))
}

async fn get_van(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(id = %id, "Getting van");
    let van = sqlx::query_as::<_, Van>("SELECT * FROM vans WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Van {} not found", id)))?;
    
    Ok(Json(van))
}

async fn add_van(
    State(state): State<AppState>,
    Json(req): Json<AddVanRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(name = %req.name, "Adding van");
    
    // Validate
    if req.name.trim().is_empty() || req.name.len() > 128 {
        return Err(AppError::Validation("Name must be 1-128 characters".to_string()));
    }
    validate_van_dimensions(req.length_mm, req.width_mm, req.height_mm)?;
    
    if let Some(weight) = req.max_weight_kg {
        if weight <= 0 || weight > 10000 {
            return Err(AppError::Validation("Max weight must be 1-10000 kg".to_string()));
        }
    }
    if let Some(wh) = req.wheel_well_height_mm {
        if wh < 0 || wh > req.height_mm {
            return Err(AppError::Validation("Wheel well height must be 0 to van height".to_string()));
        }
    }
    if let Some(ww) = req.wheel_well_width_mm {
        if ww < 0 || ww > req.width_mm / 2 {
            return Err(AppError::Validation("Wheel well width must be 0 to half van width".to_string()));
        }
    }
    if let Some(ws) = req.wheel_well_start_x_mm {
        if ws < 0 || ws > req.length_mm {
            return Err(AppError::Validation("Wheel well start must be 0 to van length".to_string()));
        }
    }
    validate_notes(&req.notes)?;
    
    let result = sqlx::query(
        "INSERT INTO vans (name, length_mm, width_mm, height_mm, max_weight_kg, wheel_well_height_mm, wheel_well_width_mm, wheel_well_start_x_mm, notes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&req.name)
    .bind(req.length_mm)
    .bind(req.width_mm)
    .bind(req.height_mm)
    .bind(req.max_weight_kg)
    .bind(req.wheel_well_height_mm)
    .bind(req.wheel_well_width_mm)
    .bind(req.wheel_well_start_x_mm)
    .bind(&req.notes)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;
    
    Ok(Json(serde_json::json!({"id": result.last_insert_rowid()})))
}

async fn update_van(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateVanRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(id = %id, "Updating van");
    
    // Check exists
    let existing = sqlx::query_as::<_, Van>("SELECT * FROM vans WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Van {} not found", id)))?;
    
    // Validate updates
    if let Some(ref name) = req.name {
        if name.trim().is_empty() || name.len() > 128 {
            return Err(AppError::Validation("Name must be 1-128 characters".to_string()));
        }
    }
    
    let length = req.length_mm.unwrap_or(existing.length_mm);
    let width = req.width_mm.unwrap_or(existing.width_mm);
    let height = req.height_mm.unwrap_or(existing.height_mm);
    validate_van_dimensions(length, width, height)?;
    
    if let Some(weight) = req.max_weight_kg {
        if weight <= 0 || weight > 10000 {
            return Err(AppError::Validation("Max weight must be 1-10000 kg".to_string()));
        }
    }
    
    if let Some(wh) = req.wheel_well_height_mm {
        if wh < 0 || wh > height {
            return Err(AppError::Validation("Wheel well height must be 0 to van height".to_string()));
        }
    }
    if let Some(ww) = req.wheel_well_width_mm {
        if ww < 0 || ww > width / 2 {
            return Err(AppError::Validation("Wheel well width must be 0 to half van width".to_string()));
        }
    }
    if let Some(ws) = req.wheel_well_start_x_mm {
        if ws < 0 || ws > length {
            return Err(AppError::Validation("Wheel well start must be 0 to van length".to_string()));
        }
    }
    
    sqlx::query(
        "UPDATE vans SET name = COALESCE(?, name), length_mm = COALESCE(?, length_mm), 
         width_mm = COALESCE(?, width_mm), height_mm = COALESCE(?, height_mm), 
         max_weight_kg = COALESCE(?, max_weight_kg), wheel_well_height_mm = COALESCE(?, wheel_well_height_mm),
         wheel_well_width_mm = COALESCE(?, wheel_well_width_mm), wheel_well_start_x_mm = COALESCE(?, wheel_well_start_x_mm),
         notes = COALESCE(?, notes), active = COALESCE(?, active) WHERE id = ?"
    )
    .bind(&req.name)
    .bind(req.length_mm)
    .bind(req.width_mm)
    .bind(req.height_mm)
    .bind(req.max_weight_kg)
    .bind(req.wheel_well_height_mm)
    .bind(req.wheel_well_width_mm)
    .bind(req.wheel_well_start_x_mm)
    .bind(&req.notes)
    .bind(req.active)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(AppError::Database)?;
    
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_van(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(id = %id, "Soft deleting van");
    
    let rows = sqlx::query("UPDATE vans SET active = 0 WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?
        .rows_affected();
    
    if rows == 0 {
        return Err(AppError::NotFound(format!("Van {} not found", id)));
    }
    
    Ok(StatusCode::NO_CONTENT)
}

// ===== CARGO ENDPOINTS (REMOVED - cargo is in-memory only) =====
// Cargo items are now sent directly in the /optimize request body

// ===== 3D BIN PACKING ALGORITHM =====

#[derive(Clone, Debug)]
struct FreeSpace {
    x: i64,
    y: i64,
    z: i64,
    length: i64,
    width: i64,
    height: i64,
}

impl FreeSpace {
    fn volume(&self) -> i64 {
        self.length * self.width * self.height
    }
    
    fn can_fit(&self, item_l: i64, item_w: i64, item_h: i64) -> bool {
        item_l <= self.length && item_w <= self.width && item_h <= self.height
    }
}

fn is_in_wheel_well(x: i64, y: i64, z: i64, length: i64, width: i64, height: i64, van: &Van) -> bool {
    let wheel_start = van.wheel_well_start_x_mm.unwrap_or(van.length_mm + 1);
    let wheel_height = van.wheel_well_height_mm.unwrap_or(0);
    let wheel_width = van.wheel_well_width_mm.unwrap_or(0);
    
    if wheel_height == 0 || wheel_width == 0 {
        return false; // No wheel wells
    }
    
    // Check if item overlaps with wheel well zones
    let item_bottom = y;
    let _item_top = y + height;
    let item_front = x;
    let _item_back = x + length;
    let item_left = z;
    let item_right = z + width;
    
    // Wheel wells only exist from wheel_start_x onwards and up to wheel_height from floor
    if item_front >= wheel_start && item_bottom < wheel_height {
        // Check left wheel well
        if item_left < wheel_width {
            return true;
        }
        // Check right wheel well
        if item_right > van.width_mm - wheel_width {
            return true;
        }
    }
    
    false
}

fn pack_items_3d(items: &[CargoItem], van: &Van, warnings: &mut Vec<String>) -> Vec<PositionedItem> {
    let mut positioned = Vec::new();
    let mut free_spaces = vec![FreeSpace {
        x: 0,
        y: 0,
        z: 0,
        length: van.length_mm,
        width: van.width_mm,
        height: van.height_mm,
    }];
    
    // Sort items: fragile first (go on top), then heavy (bottom), then largest volume
    let mut sorted_items: Vec<(usize, &CargoItem)> = items.iter().enumerate().collect();
    sorted_items.sort_by(|a, b| {
        // Priority 1: Non-fragile first (they go bottom)
        match (a.1.fragile, b.1.fragile) {
            (false, true) => return std::cmp::Ordering::Less,
            (true, false) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        // Priority 2: Heavy first
        let weight_cmp = b.1.weight_kg.partial_cmp(&a.1.weight_kg).unwrap_or(std::cmp::Ordering::Equal);
        if weight_cmp != std::cmp::Ordering::Equal {
            return weight_cmp;
        }
        // Priority 3: Largest volume first
        let vol_a = a.1.length_mm * a.1.width_mm * a.1.height_mm;
        let vol_b = b.1.length_mm * b.1.width_mm * b.1.height_mm;
        vol_b.cmp(&vol_a)
    });
    
    for (_idx, item) in sorted_items {
        let orientations = if item.rotation_allowed {
            vec![
                (item.length_mm, item.width_mm, item.height_mm, 0),
                (item.length_mm, item.height_mm, item.width_mm, 90),
                (item.width_mm, item.length_mm, item.height_mm, 90),
                (item.width_mm, item.height_mm, item.length_mm, 0),
                (item.height_mm, item.length_mm, item.width_mm, 90),
                (item.height_mm, item.width_mm, item.length_mm, 90),
            ]
        } else {
            vec![(item.length_mm, item.width_mm, item.height_mm, 0)]
        };
        
        let mut best_fit: Option<(usize, i64, i64, i64, i64, i64, i64, i64)> = None;
        let mut best_waste = i64::MAX;
        
        for (space_idx, space) in free_spaces.iter().enumerate() {
            for (l, w, h, rot) in &orientations {
                if space.can_fit(*l, *w, *h) {
                    // Check wheel well collision
                    if is_in_wheel_well(space.x, space.y, space.z, *l, *w, *h, van) {
                        continue;
                    }
                    
                    let waste = space.volume() - (l * w * h);
                    if waste < best_waste {
                        best_waste = waste;
                        best_fit = Some((space_idx, space.x, space.y, space.z, *l, *w, *h, *rot));
                    }
                }
            }
        }
        
        if let Some((space_idx, x, y, z, l, w, h, rot)) = best_fit {
            positioned.push(PositionedItem {
                item: item.clone(),
                position: Position3D { x, y, z },
                rotation: Rotation3D { x: 0, y: rot, z: 0 },
                level: (y / 500) as i32,
            });
            
            // Remove used space and create new free spaces (Guillotine split)
            let used_space = free_spaces.remove(space_idx);
            
            // Split along length (X axis)
            if used_space.length > l {
                free_spaces.push(FreeSpace {
                    x: used_space.x + l,
                    y: used_space.y,
                    z: used_space.z,
                    length: used_space.length - l,
                    width: used_space.width,
                    height: used_space.height,
                });
            }
            
            // Split along width (Z axis)
            if used_space.width > w {
                free_spaces.push(FreeSpace {
                    x: used_space.x,
                    y: used_space.y,
                    z: used_space.z + w,
                    length: l,
                    width: used_space.width - w,
                    height: used_space.height,
                });
            }
            
            // Split along height (Y axis)
            if used_space.height > h {
                free_spaces.push(FreeSpace {
                    x: used_space.x,
                    y: used_space.y + h,
                    z: used_space.z,
                    length: l,
                    width: w,
                    height: used_space.height - h,
                });
            }
            
            // Merge overlapping spaces (simplified - just sort by volume)
            free_spaces.sort_by(|a, b| b.volume().cmp(&a.volume()));
        } else {
            warnings.push(format!("Item '{}' could not be placed", item.description));
        }
    }
    
    positioned
}

// ===== OPTIMIZE ENDPOINT =====

async fn optimize_loading(
    State(state): State<AppState>,
    Json(req): Json<OptimizeRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(van_id = %req.van_id, items = %req.items.len(), "Optimizing load");
    
    // Get van details
    let van = sqlx::query_as::<_, Van>("SELECT * FROM vans WHERE id = ? AND active = 1")
        .bind(req.van_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Van {} not found or inactive", req.van_id)))?;
    
    // Validate items
    let mut warnings = Vec::new();
    let mut total_weight = 0.0;
    
    for item in &req.items {
        if item.description.trim().is_empty() {
            return Err(AppError::Validation("Item description cannot be empty".to_string()));
        }
        if item.length_mm < 10 || item.length_mm > 5000 {
            return Err(AppError::Validation(format!("Item '{}': length must be 10-5000mm", item.description)));
        }
        if item.width_mm < 10 || item.width_mm > 5000 {
            return Err(AppError::Validation(format!("Item '{}': width must be 10-5000mm", item.description)));
        }
        if item.height_mm < 10 || item.height_mm > 5000 {
            return Err(AppError::Validation(format!("Item '{}': height must be 10-5000mm", item.description)));
        }
        if item.weight_kg <= 0.0 || item.weight_kg > 500.0 {
            return Err(AppError::Validation(format!("Item '{}': weight must be 0.1-500kg", item.description)));
        }
        
        total_weight += item.weight_kg;
        
        // Warnings for oversized items
        if item.length_mm > van.length_mm || item.width_mm > van.width_mm || item.height_mm > van.height_mm {
            warnings.push(format!("Item '{}' may not fit ({}×{}×{}mm exceeds van dimensions)", 
                item.description, item.length_mm, item.width_mm, item.height_mm));
        }
    }
    
    // Check weight limit
    if let Some(max_weight) = van.max_weight_kg {
        if total_weight > max_weight as f64 {
            warnings.push(format!("Total weight ({:.1}kg) exceeds van capacity ({}kg)", total_weight, max_weight));
        }
    }
    
    if req.items.is_empty() {
        return Ok(Json(OptimizeResponse {
            success: false,
            plan: None,
            warnings: vec!["No items to load".to_string()],
        }));
    }
    
    // TODO: Run 3D bin packing algorithm here
    let van_volume = van.length_mm * van.width_mm * van.height_mm;
    
    // Run 3D bin packing
    let positioned_items = pack_items_3d(&req.items, &van, &mut warnings);
    
    let used_volume: i64 = positioned_items.iter()
        .map(|pi| pi.item.length_mm * pi.item.width_mm * pi.item.height_mm)
        .sum();
    
    let utilization = (used_volume as f64 / van_volume as f64 * 100.0).min(100.0);
    
    let plan = LoadingPlan {
        items: positioned_items,
        total_weight,
        utilization_percent: utilization,
        van_volume,
        used_volume,
    };
    
    Ok(Json(OptimizeResponse {
        success: true,
        plan: Some(plan),
        warnings,
    }))
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
}

#[derive(Serialize)]
struct ReadinessResponse {
    status: String,
    database: String,
}

async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

async fn readiness_check(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    // Quick DB query to verify connection
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;
    
    Ok(Json(ReadinessResponse {
        status: "ready".to_string(),
        database: "connected".to_string(),
    }))
}