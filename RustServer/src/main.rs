use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
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

// TODO: Big consideration, but maybe add cut grain direction later.

// Validation constants
const MIN_DIMENSION: i64 = 1;
const MAX_DIMENSION: i64 = 10000;
const MAX_THICKNESS: i64 = 1000;
const MIN_MATERIAL_LEN: usize = 1;
const MAX_MATERIAL_LEN: usize = 64;
const MAX_NOTES_LEN: usize = 256;

#[derive(Clone)]
struct AppState {
    db: Pool<Sqlite>,
}

// Custom error type with JSON response
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

// Validation helpers
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

    sqlx::migrate!("./Migrations").run(&db).await?;

    let state = AppState { db };

    // Leftover <=> Resto
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/add", post(add_resto))
        .route("/remove/:id", delete(remove_resto))
        .route("/update/:id", post(update_resto))
        .route("/search", get(search_resto))
        .route("/list", get(list_restos))
        .route("/stats", get(get_stats))
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