use axum::{
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, Pool, Sqlite};
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::process::Command;
use std::time::Duration;
use time::OffsetDateTime;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cutting_optimizer;
use cutting_optimizer::optimize_cuts;

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
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, details) = match self {
            AppError::Validation(msg) => {
                tracing::warn!(error = %msg, "Validation error");
                (
                    StatusCode::BAD_REQUEST,
                    "Validation error".to_string(),
                    Some(msg),
                )
            }
            AppError::NotFound(msg) => {
                tracing::debug!(error = %msg, "Resource not found");
                (StatusCode::NOT_FOUND, "Not found".to_string(), Some(msg))
            }
            AppError::Database(err) => {
                tracing::error!("Database error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                    None,
                )
            }
            AppError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal error".to_string(),
                    Some(msg),
                )
            }
        };

        let body = Json(ErrorResponse { error, details });
        (status, body).into_response()
    }
}

fn validate_dimensions(width: i64, height: i64, thickness: i64) -> Result<(), AppError> {
    if width < MIN_DIMENSION || width > MAX_DIMENSION {
        return Err(AppError::Validation(format!(
            "Width must be {}-{} mm",
            MIN_DIMENSION, MAX_DIMENSION
        )));
    }
    if height < MIN_DIMENSION || height > MAX_DIMENSION {
        return Err(AppError::Validation(format!(
            "Height must be {}-{} mm",
            MIN_DIMENSION, MAX_DIMENSION
        )));
    }
    if thickness < MIN_DIMENSION || thickness > MAX_THICKNESS {
        return Err(AppError::Validation(format!(
            "Thickness must be {}-{} mm",
            MIN_DIMENSION, MAX_THICKNESS
        )));
    }
    Ok(())
}

fn validate_van_dimensions(length: i64, width: i64, height: i64) -> Result<(), AppError> {
    if length < MIN_DIMENSION || length > MAX_DIMENSION {
        return Err(AppError::Validation(format!(
            "Length must be {}-{} mm",
            MIN_DIMENSION, MAX_DIMENSION
        )));
    }
    if width < MIN_DIMENSION || width > MAX_DIMENSION {
        return Err(AppError::Validation(format!(
            "Width must be {}-{} mm",
            MIN_DIMENSION, MAX_DIMENSION
        )));
    }
    if height < MIN_DIMENSION || height > MAX_DIMENSION {
        return Err(AppError::Validation(format!(
            "Height must be {}-{} mm",
            MIN_DIMENSION, MAX_DIMENSION
        )));
    }
    Ok(())
}

fn validate_material(material: &str) -> Result<(), AppError> {
    let len = material.len();
    if len < MIN_MATERIAL_LEN || len > MAX_MATERIAL_LEN {
        return Err(AppError::Validation(format!(
            "Material must be {}-{} chars",
            MIN_MATERIAL_LEN, MAX_MATERIAL_LEN
        )));
    }
    if material.trim().is_empty() {
        return Err(AppError::Validation("Material cannot be empty".to_string()));
    }
    Ok(())
}

fn validate_notes(notes: &Option<String>) -> Result<(), AppError> {
    if let Some(n) = notes {
        if n.len() > MAX_NOTES_LEN {
            return Err(AppError::Validation(format!(
                "Notes max {} chars",
                MAX_NOTES_LEN
            )));
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

    let auth_header = headers.get("Authorization").and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if token == required_token {
                Ok(next.run(request).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

#[derive(Serialize, FromRow, Clone)]
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

#[derive(Serialize, FromRow, Clone, Debug)]
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

#[derive(Deserialize, Serialize, Clone, Debug)]
struct CargoItem {
    description: String,
    length_mm: i64,
    width_mm: i64,
    height_mm: i64,
    #[serde(default = "default_weight")]
    weight_kg: f64,
    #[serde(default)]
    fragile: bool,
    #[serde(default = "default_true")]
    rotation_allowed: bool,
    #[serde(default = "default_true")]
    stackable: bool,
    #[serde(default)]
    color: Option<String>,
}

fn default_weight() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct OptimizeRequest {
    van_id: i64,
    items: Vec<CargoItem>,
}

#[derive(Serialize, Deserialize, Clone)]
struct PositionedItem {
    item: CargoItem,
    position: Position3D,
    rotation: Rotation3D,
    level: i32,
    placed_length: i64,
    placed_width: i64,
    placed_height: i64,
}

#[derive(Serialize, Deserialize, Clone)]
struct Position3D {
    x: i64,
    y: i64,
    z: i64,
}

#[derive(Serialize, Deserialize, Clone)]
struct Rotation3D {
    x: i64,
    y: i64,
    z: i64,
}

#[derive(Serialize, Deserialize)]
struct OptimizeResponse {
    success: bool,
    plan: Option<LoadingPlan>,
    #[serde(default)]
    unplaced_items: Vec<CargoItem>,
    warnings: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
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

    sqlx::query("PRAGMA journal_mode=WAL;").execute(&db).await?;
    sqlx::query("PRAGMA foreign_keys=ON;").execute(&db).await?;
    sqlx::query("PRAGMA busy_timeout=5000;")
        .execute(&db)
        .await?;

    let integrity_result: (String,) = sqlx::query_as("PRAGMA integrity_check;")
        .fetch_one(&db)
        .await?;
    if integrity_result.0 != "ok" {
        anyhow::bail!("Database corruption detected!");
    }

    sqlx::migrate!("./Migrations").run(&db).await?;

    let current_version: Option<(i64,)> =
        sqlx::query_as("SELECT version FROM schema_metadata ORDER BY version DESC LIMIT 1")
            .fetch_optional(&db)
            .await?;
    if current_version.is_none() {
        sqlx::query(
            "INSERT INTO schema_metadata (version, applied_at) VALUES (?1, CURRENT_TIMESTAMP)",
        )
        .bind(SCHEMA_VERSION)
        .execute(&db)
        .await
        .ok();
    }

    let auth_token = std::env::var("AUTH_TOKEN").ok();
    let state = AppState { db, auth_token };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/add", post(add_resto))
        .route("/remove/:id", delete(remove_resto))
        .route("/delete_batch", post(delete_batch))
        .route("/update/:id", post(update_resto))
        .route("/search", get(search_resto))
        .route("/list", get(list_restos))
        .route("/stats", get(get_stats))
        .route("/vans", get(list_vans).post(add_van))
        .route(
            "/vans/:id",
            get(get_van).post(update_van).delete(delete_van),
        )
        .route("/optimize", post(optimize_loading))
        .route("/optimize_cuts", post(optimize_cuts))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                        .on_response(
                            DefaultOnResponse::new()
                                .level(tracing::Level::INFO)
                                .latency_unit(LatencyUnit::Millis),
                        ),
                )
                .layer(TimeoutLayer::new(Duration::from_secs(120)))
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods(Any)
                        .allow_headers(Any),
                ),
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
    validate_dimensions(payload.width_mm, payload.height_mm, payload.thickness_mm)?;
    validate_material(&payload.material)?;
    validate_notes(&payload.notes)?;

    let created_at = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();

    let result = sqlx::query("INSERT INTO leftovers (width_mm, height_mm, thickness_mm, material, notes, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
        .bind(payload.width_mm).bind(payload.height_mm).bind(payload.thickness_mm).bind(&payload.material).bind(&payload.notes).bind(created_at)
        .execute(&state.db).await.map_err(AppError::Database)?;

    Ok((
        StatusCode::CREATED,
        Json(AddLeftoverResponse {
            id: result.last_insert_rowid(),
        }),
    ))
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
        return Err(AppError::NotFound(format!("No resto with id {}", id)));
    }

    tracing::info!(id = id, "Removed resto");
    Ok(Json(serde_json::json!({ "success": true, "id": id })))
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
    Ok(Json(format!("Deleted {} records", result.rows_affected())))
}

async fn search_resto(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, AppError> {
    validate_dimensions(params.width_mm, params.height_mm, params.thickness_mm)?;
    validate_material(&params.material)?;
    let required_area = params.width_mm * params.height_mm;
    let candidates: Vec<Leftover> = sqlx::query_as(
        "SELECT id, width_mm, height_mm, thickness_mm, material, notes, created_at FROM leftovers WHERE width_mm >= ?1 AND height_mm >= ?2 AND thickness_mm = ?3 AND LOWER(material) = LOWER(?4)"
    )
    .bind(params.width_mm).bind(params.height_mm).bind(params.thickness_mm).bind(&params.material)
    .fetch_all(&state.db).await.map_err(AppError::Database)?;

    if candidates.is_empty() {
        return Err(AppError::NotFound(format!("No matching restos found")));
    }
    let best_match = candidates
        .into_iter()
        .min_by_key(|l| (l.width_mm * l.height_mm) - required_area)
        .unwrap();
    Ok(Json(best_match))
}

async fn list_restos(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let restos: Vec<Leftover> = sqlx::query_as("SELECT id, width_mm, height_mm, thickness_mm, material, notes, created_at FROM leftovers ORDER BY created_at DESC")
        .fetch_all(&state.db).await.map_err(AppError::Database)?;
    Ok(Json(restos))
}

async fn update_resto(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateLeftoverRequest>,
) -> Result<impl IntoResponse, AppError> {
    let existing: Option<Leftover> = sqlx::query_as("SELECT * FROM leftovers WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?;
    let resto = existing.ok_or_else(|| AppError::NotFound(format!("Resto {} not found", id)))?;

    let new_width = payload.width_mm.unwrap_or(resto.width_mm);
    let new_height = payload.height_mm.unwrap_or(resto.height_mm);
    let new_thickness = payload.thickness_mm.unwrap_or(resto.thickness_mm);
    let new_material = payload.material.unwrap_or(resto.material);
    let new_notes = payload.notes.or(resto.notes);

    validate_dimensions(new_width, new_height, new_thickness)?;
    validate_material(&new_material)?;
    validate_notes(&new_notes)?;

    sqlx::query("UPDATE leftovers SET width_mm = ?1, height_mm = ?2, thickness_mm = ?3, material = ?4, notes = ?5 WHERE id = ?6")
        .bind(new_width).bind(new_height).bind(new_thickness).bind(&new_material).bind(&new_notes).bind(id)
        .execute(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(
        serde_json::json!({ "success": true, "id": id, "message": "Updated successfully" }),
    ))
}

async fn get_stats(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let (total_count, total_area_mm2): (i64, i64) =
        sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(width_mm * height_mm), 0) FROM leftovers")
            .fetch_one(&state.db)
            .await
            .map_err(AppError::Database)?;
    let by_material: Vec<MaterialStats> = sqlx::query_as("SELECT material, COUNT(*) as count, SUM(width_mm * height_mm) as total_area_mm2 FROM leftovers GROUP BY material ORDER BY count DESC")
        .fetch_all(&state.db).await.map_err(AppError::Database)?;
    let by_thickness: Vec<ThicknessStats> = sqlx::query_as("SELECT thickness_mm, COUNT(*) as count FROM leftovers GROUP BY thickness_mm ORDER BY thickness_mm")
        .fetch_all(&state.db).await.map_err(AppError::Database)?;
    Ok(Json(StatsResponse {
        total_count,
        total_area_mm2,
        by_material,
        by_thickness,
    }))
}

async fn list_vans(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let vans =
        sqlx::query_as::<_, Van>("SELECT * FROM vans WHERE active = 1 ORDER BY created_at DESC")
            .fetch_all(&state.db)
            .await
            .map_err(AppError::Database)?;
    Ok(Json(vans))
}

async fn get_van(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
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
    if req.name.trim().is_empty() || req.name.len() > 128 {
        return Err(AppError::Validation("Invalid name".into()));
    }
    validate_van_dimensions(req.length_mm, req.width_mm, req.height_mm)?;
    validate_notes(&req.notes)?;

    let result = sqlx::query("INSERT INTO vans (name, length_mm, width_mm, height_mm, max_weight_kg, wheel_well_height_mm, wheel_well_width_mm, wheel_well_start_x_mm, notes) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&req.name).bind(req.length_mm).bind(req.width_mm).bind(req.height_mm).bind(req.max_weight_kg)
        .bind(req.wheel_well_height_mm).bind(req.wheel_well_width_mm).bind(req.wheel_well_start_x_mm).bind(&req.notes)
        .execute(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(serde_json::json!({"id": result.last_insert_rowid()})))
}

async fn update_van(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateVanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let existing = sqlx::query_as::<_, Van>("SELECT * FROM vans WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Van {} not found", id)))?;

    sqlx::query("UPDATE vans SET name = COALESCE(?, name), length_mm = COALESCE(?, length_mm), width_mm = COALESCE(?, width_mm), height_mm = COALESCE(?, height_mm), max_weight_kg = COALESCE(?, max_weight_kg), wheel_well_height_mm = COALESCE(?, wheel_well_height_mm), wheel_well_width_mm = COALESCE(?, wheel_well_width_mm), wheel_well_start_x_mm = COALESCE(?, wheel_well_start_x_mm), notes = COALESCE(?, notes), active = COALESCE(?, active) WHERE id = ?")
        .bind(&req.name).bind(req.length_mm).bind(req.width_mm).bind(req.height_mm).bind(req.max_weight_kg)
        .bind(req.wheel_well_height_mm).bind(req.wheel_well_width_mm).bind(req.wheel_well_start_x_mm)
        .bind(&req.notes).bind(req.active).bind(id)
        .execute(&state.db).await.map_err(AppError::Database)?;

    Ok(Json(serde_json::json!({ "success": true })))
}

async fn delete_van(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let rows = sqlx::query("UPDATE vans SET active = 0 WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?
        .rows_affected();
    if rows == 0 {
        return Err(AppError::NotFound(format!("Van {} not found", id)));
    }
    Ok(Json(serde_json::json!({ "success": true, "id": id })))
}

async fn optimize_loading(
    State(state): State<AppState>,
    Json(req): Json<OptimizeRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(van_id = %req.van_id, items = %req.items.len(), "Optimizing load via Python Sidecar");

    let van = sqlx::query_as::<_, Van>("SELECT * FROM vans WHERE id = ? AND active = 1")
        .bind(req.van_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound(format!("Van {} not found or inactive", req.van_id)))?;

    let input_data = serde_json::json!({
        "van": van,
        "items": req.items
    });

    let input_str = serde_json::to_string(&input_data)
        .map_err(|e| AppError::Internal(format!("JSON serialization error: {}", e)))?;

    let python_result = tokio::task::spawn_blocking(move || {
        let (cmd, args) = if cfg!(target_os = "windows") {
            ("py", vec!["-3.12", "optimizer.py"])
        } else {
            ("python3", vec!["optimizer.py"])
        };
        let mut child = std::process::Command::new(cmd)
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn python process: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input_str.as_bytes())
                .map_err(|e| format!("Failed to write to python stdin: {}", e))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to read python output: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Python optimizer failed: {}", stderr));
        }

        Ok(output.stdout)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Task spawn error: {}", e)))?;

    let output_bytes = python_result.map_err(AppError::Internal)?;

    let response: OptimizeResponse = serde_json::from_slice(&output_bytes).map_err(|e| {
        AppError::Internal(format!(
            "Failed to parse Python response: {}. Output was: {:?}",
            e,
            String::from_utf8_lossy(&output_bytes)
        ))
    })?;

    Ok(Json(response))
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
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(AppError::Database)?;
    Ok(Json(ReadinessResponse {
        status: "ready".to_string(),
        database: "connected".to_string(),
    }))
}
