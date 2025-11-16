use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, FromRow, Pool, Sqlite};
use std::net::SocketAddr;
use std::fs;
use time::OffsetDateTime;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// TODO: Big consideration, but maybe add cut grain direction later.

#[derive(Clone)]
struct AppState {
    db: Pool<Sqlite>,
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

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite://data/retlister.db")
        .await?;

    let _ = sqlx::query("PRAGMA journal_mode=WAL;").execute(&db).await;
    let _ = sqlx::query("PRAGMA foreign_keys=ON;").execute(&db).await;

    sqlx::migrate!().run(&db).await?;

    let state = AppState { db };

    // Leftover <=> Resto
    let app = Router::new()
        .route("/add", post(add_resto))
        .route("/remove/:id", delete(remove_resto))
        .route("/update/:id", post(update_resto))
        .route("/search", get(search_resto))
        .route("/list", get(list_restos))
        .route("/stats", get(get_stats))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    tracing::info!("Socket on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}

async fn add_resto(
    State(state): State<AppState>,
    Json(payload): Json<AddLeftoverRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if payload.width_mm <= 0
        || payload.height_mm <= 0
        || payload.thickness_mm <= 0
        || payload.material.is_empty()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid input, check:
 > Dimensions must be positive numbers
 > Material must not be empty"
                .into(),
        ));
    }

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
    .map_err(internal_error)?;

    let id = result.last_insert_rowid();

    Ok((StatusCode::CREATED, Json(AddLeftoverResponse { id })))
}

async fn remove_resto(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let res = sqlx::query("DELETE FROM leftovers WHERE id = ?1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(internal_error)?;

    if res.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, format!("No resto with id {id}")));
    }

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
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if params.width_mm <= 0
        || params.height_mm <= 0
        || params.thickness_mm <= 0
        || params.material.is_empty()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid search parameters, check:
 > Dimensions must be positive numbers
 > Material must not be empty"
                .into(),
        ));
    }

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
    .map_err(internal_error)?;

    if candidates.is_empty() {
        return Err((StatusCode::NOT_FOUND, "No matching restos found".into()));
    }

    let best_match = candidates
        .into_iter()
        .min_by_key(|leftover| {
            let leftover_area = leftover.width_mm * leftover.height_mm;
            leftover_area - required_area
        })
        .unwrap();

    Ok(Json(best_match))
}

async fn list_restos(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let restos: Vec<Leftover> = sqlx::query_as(
        r#"
        SELECT id, width_mm, height_mm, thickness_mm, material, notes, created_at
        FROM leftovers
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    Ok(Json(restos))
}

async fn update_resto(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateLeftoverRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Validate if provided
    if let Some(w) = payload.width_mm {
        if w <= 0 {
            return Err((StatusCode::BAD_REQUEST, "Width must be positive".into()));
        }
    }
    if let Some(h) = payload.height_mm {
        if h <= 0 {
            return Err((StatusCode::BAD_REQUEST, "Height must be positive".into()));
        }
    }
    if let Some(t) = payload.thickness_mm {
        if t <= 0 {
            return Err((StatusCode::BAD_REQUEST, "Thickness must be positive".into()));
        }
    }
    if let Some(ref m) = payload.material {
        if m.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "Material cannot be empty".into()));
        }
    }

    // Check if exists
    let existing: Option<Leftover> = sqlx::query_as(
        "SELECT id, width_mm, height_mm, thickness_mm, material, notes, created_at FROM leftovers WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?;

    if existing.is_none() {
        return Err((StatusCode::NOT_FOUND, "Resto not found".into()));
    }

    let resto = existing.unwrap();

    // Apply updates
    let new_width = payload.width_mm.unwrap_or(resto.width_mm);
    let new_height = payload.height_mm.unwrap_or(resto.height_mm);
    let new_thickness = payload.thickness_mm.unwrap_or(resto.thickness_mm);
    let new_material = payload.material.unwrap_or(resto.material);
    let new_notes = payload.notes.or(resto.notes);

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
    .map_err(internal_error)?;

    Ok(StatusCode::OK)
}

async fn get_stats(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Total count and area
    let total: (i64, i64) = sqlx::query_as(
        r#"
        SELECT COUNT(*), COALESCE(SUM(width_mm * height_mm), 0)
        FROM leftovers
        "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(internal_error)?;

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
    .map_err(internal_error)?;

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
    .map_err(internal_error)?;

    Ok(Json(StatsResponse {
        total_count,
        total_area_mm2,
        by_material,
        by_thickness,
    }))
}

fn internal_error<E: std::fmt::Display>(err: E) -> (StatusCode, String) {
    tracing::error!("Internal server error: {err}");
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into())
}