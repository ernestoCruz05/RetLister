use crate::{AppError, AppState, Leftover, MAX_DIMENSION, MAX_THICKNESS};
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

// ===== CUTTING OPTIMIZER STRUCTURES =====

#[derive(Deserialize, Serialize, Clone)]
pub struct CutRequest {
    pub width_mm: i64,
    pub height_mm: i64,
    pub thickness_mm: i64,
    pub material: String,
    pub quantity: i32,
}

#[derive(Deserialize)]
pub struct OptimizeCutsRequest {
    pub cuts: Vec<CutRequest>,
    #[serde(default = "default_kerf")]
    pub kerf_width_mm: i64,
    #[serde(default = "default_min_remainder")]
    pub min_remainder_width_mm: i64,
    #[serde(default = "default_min_remainder")]
    pub min_remainder_height_mm: i64,
}

fn default_kerf() -> i64 { 3 }
fn default_min_remainder() -> i64 { 100 }

#[derive(Serialize, Clone)]
pub struct PlacedCut {
    pub original_index: usize,
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    pub rotated: bool,
    pub material: String,
    pub thickness_mm: i64,
}

#[derive(Serialize)]
pub struct UsedPlank {
    pub resto_id: i64,
    pub width_mm: i64,
    pub height_mm: i64,
    pub thickness_mm: i64,
    pub material: String,
    pub cuts: Vec<PlacedCut>,
    pub waste_percent: f64,
    pub total_area_mm2: i64,
    pub used_area_mm2: i64,
}

#[derive(Serialize)]
pub struct OptimizeCutsResponse {
    pub success: bool,
    pub used_planks: Vec<UsedPlank>,
    pub unplaced_cuts: Vec<(usize, CutRequest)>,
    pub efficiency_percent: f64,
    pub total_cuts_placed: usize,
    pub total_cuts_requested: usize,
}

#[derive(Clone, Debug)]
struct FreeRect {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

// ===== CUTTING OPTIMIZER IMPLEMENTATION =====

pub async fn optimize_cuts(
    State(state): State<AppState>,
    Json(req): Json<OptimizeCutsRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(cuts = %req.cuts.len(), "Optimizing cuts");
    
    // Expand cuts by quantity
    let mut all_cuts = Vec::new();
    for (idx, cut_req) in req.cuts.iter().enumerate() {
        // Validate dimensions
        if cut_req.width_mm < 10 || cut_req.width_mm > MAX_DIMENSION {
            return Err(AppError::Validation(format!("Cut {}: width must be 10-{}mm", idx, MAX_DIMENSION)));
        }
        if cut_req.height_mm < 10 || cut_req.height_mm > MAX_DIMENSION {
            return Err(AppError::Validation(format!("Cut {}: height must be 10-{}mm", idx, MAX_DIMENSION)));
        }
        if cut_req.thickness_mm < 1 || cut_req.thickness_mm > MAX_THICKNESS {
            return Err(AppError::Validation(format!("Cut {}: thickness must be 1-{}mm", idx, MAX_THICKNESS)));
        }
        if cut_req.quantity < 1 || cut_req.quantity > 1000 {
            return Err(AppError::Validation(format!("Cut {}: quantity must be 1-1000", idx)));
        }
        
        for _ in 0..cut_req.quantity {
            all_cuts.push((idx, cut_req.clone()));
        }
    }
    
    // Sort cuts by area descending (Best-Fit Decreasing)
    all_cuts.sort_by(|a, b| {
        let area_a = a.1.width_mm * a.1.height_mm;
        let area_b = b.1.width_mm * b.1.height_mm;
        area_b.cmp(&area_a)
    });
    
    // Get available inventory - sort by area DESCENDING so we try bigger planks first
    // This helps consolidate cuts onto fewer planks
    let inventory = sqlx::query_as::<_, Leftover>("SELECT * FROM leftovers WHERE width_mm >= 10 AND height_mm >= 10 ORDER BY width_mm * height_mm DESC")
        .fetch_all(&state.db)
        .await
        .map_err(AppError::Database)?;
    
    let mut used_planks: Vec<UsedPlank> = Vec::new();
    let mut unplaced_cuts: Vec<(usize, CutRequest)> = Vec::new();
    
    // Group inventory by (material, thickness)
    use std::collections::HashMap;
    let mut plank_pools: HashMap<(String, i64), Vec<(Leftover, Vec<FreeRect>)>> = HashMap::new();
    
    for plank in inventory.into_iter() {
        let key = (plank.material.to_lowercase(), plank.thickness_mm);
        let free_rects = vec![FreeRect { x: 0, y: 0, width: plank.width_mm, height: plank.height_mm }];
        plank_pools.entry(key).or_insert_with(Vec::new).push((plank, free_rects));
    }
    
    // Try to place each cut
    for (orig_idx, cut) in all_cuts {
        let key = (cut.material.to_lowercase(), cut.thickness_mm);
        
        tracing::debug!(
            cut_idx = orig_idx,
            width = cut.width_mm,
            height = cut.height_mm,
            material = %cut.material,
            "Attempting to place cut"
        );
        
        if let Some(planks) = plank_pools.get_mut(&key) {
            let mut placed = false;
            
            // Sort planks: prioritize those already in use (have cuts placed) 
            // First check: is this plank already being used?
            // Second check: bigger planks first
            planks.sort_by(|(plank_a, _), (plank_b, _)| {
                let a_used = used_planks.iter().any(|p| p.resto_id == plank_a.id);
                let b_used = used_planks.iter().any(|p| p.resto_id == plank_b.id);
                
                match (a_used, b_used) {
                    (true, false) => std::cmp::Ordering::Less,    // A is used, comes first
                    (false, true) => std::cmp::Ordering::Greater, // B is used, comes first
                    _ => {
                        // Both used or both unused, sort by area descending
                        let area_a = plank_a.width_mm * plank_a.height_mm;
                        let area_b = plank_b.width_mm * plank_b.height_mm;
                        area_b.cmp(&area_a)
                    }
                }
            });
            
            tracing::debug!(
                available_planks = planks.len(),
                "Sorted planks for placement"
            );
            
            // Try each plank in the pool
            for (plank, free_rects) in planks.iter_mut() {
                tracing::debug!(
                    plank_id = plank.id,
                    plank_dims = format!("{}x{}", plank.width_mm, plank.height_mm),
                    free_rects = free_rects.len(),
                    "Trying plank"
                );
                // Try both orientations
                let orientations = [
                    (cut.width_mm, cut.height_mm, false),
                    (cut.height_mm, cut.width_mm, true),
                ];
                
                for (w, h, rotated) in orientations {
                    // Find best-fit rectangle
                    let mut best_rect_idx: Option<usize> = None;
                    let mut best_waste = i64::MAX;
                    
                    for (i, rect) in free_rects.iter().enumerate() {
                        if rect.width >= w && rect.height >= h {
                            let waste = (rect.width * rect.height) - (w * h);
                            if waste < best_waste {
                                best_waste = waste;
                                best_rect_idx = Some(i);
                            }
                        }
                    }
                    
                    if let Some(rect_idx) = best_rect_idx {
                        let rect = free_rects.remove(rect_idx);
                        
                        let placed_cut = PlacedCut {
                            original_index: orig_idx,
                            x: rect.x,
                            y: rect.y,
                            width: w,
                            height: h,
                            rotated,
                            material: cut.material.clone(),
                            thickness_mm: cut.thickness_mm,
                        };
                        
                        // Split the rectangle (Guillotine)
                        let right_rect = FreeRect {
                            x: rect.x + w + req.kerf_width_mm,
                            y: rect.y,
                            width: rect.width.saturating_sub(w + req.kerf_width_mm),
                            height: h,
                        };
                        
                        let bottom_rect = FreeRect {
                            x: rect.x,
                            y: rect.y + h + req.kerf_width_mm,
                            width: rect.width,
                            height: rect.height.saturating_sub(h + req.kerf_width_mm),
                        };
                        
                        if right_rect.width >= req.min_remainder_width_mm && right_rect.height >= req.min_remainder_height_mm {
                            free_rects.push(right_rect);
                        }
                        if bottom_rect.width >= req.min_remainder_width_mm && bottom_rect.height >= req.min_remainder_height_mm {
                            free_rects.push(bottom_rect);
                        }
                        
                        // Add to used planks
                        if let Some(plank_entry) = used_planks.iter_mut().find(|p| p.resto_id == plank.id) {
                            plank_entry.cuts.push(placed_cut);
                            tracing::debug!(
                                plank_id = plank.id,
                                total_cuts = plank_entry.cuts.len(),
                                "Added cut to existing plank"
                            );
                        } else {
                            tracing::debug!(
                                plank_id = plank.id,
                                "Opening new plank"
                            );
                            used_planks.push(UsedPlank {
                                resto_id: plank.id,
                                width_mm: plank.width_mm,
                                height_mm: plank.height_mm,
                                thickness_mm: plank.thickness_mm,
                                material: plank.material.clone(),
                                cuts: vec![placed_cut],
                                waste_percent: 0.0,
                                total_area_mm2: plank.width_mm * plank.height_mm,
                                used_area_mm2: 0,
                            });
                        }
                        
                        placed = true;
                        break;
                    }
                }
                
                if placed {
                    break;
                }
            }
            
            if !placed {
                unplaced_cuts.push((orig_idx, cut));
            }
        } else {
            unplaced_cuts.push((orig_idx, cut));
        }
    }
    
    // Update stats for each plank
    for plank in &mut used_planks {
        let total = plank.total_area_mm2;
        let mut used = 0i64;
        
        for cut in &plank.cuts {
            used += cut.width * cut.height;
            used += (cut.width + cut.height) * req.kerf_width_mm;
        }
        
        plank.used_area_mm2 = used;
        plank.waste_percent = if total > 0 {
            ((total - used) as f64 / total as f64) * 100.0
        } else {
            0.0
        };
    }
    
    // Calculate efficiency
    let total_cuts_placed = req.cuts.iter().map(|c| c.quantity as usize).sum::<usize>() - unplaced_cuts.len();
    let total_area_used: i64 = used_planks.iter().map(|p| p.used_area_mm2).sum();
    let total_area_available: i64 = used_planks.iter().map(|p| p.total_area_mm2).sum();
    let efficiency = if total_area_available > 0 {
        (total_area_used as f64 / total_area_available as f64) * 100.0
    } else {
        0.0
    };
    
    Ok(Json(OptimizeCutsResponse {
        success: true,
        used_planks,
        unplaced_cuts,
        efficiency_percent: efficiency,
        total_cuts_placed,
        total_cuts_requested: req.cuts.iter().map(|c| c.quantity as usize).sum(),
    }))
}
