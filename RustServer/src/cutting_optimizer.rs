use crate::{AppError, AppState, Leftover, MAX_DIMENSION, MAX_THICKNESS};
use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

const BEAM_WIDTH: usize = 5;
const FULL_SHEET_W: i64 = 2800;
const FULL_SHEET_H: i64 = 3000;

#[derive(Deserialize, Serialize, Clone)]
pub struct CutRequest {
    pub width_mm: i64,
    pub height_mm: i64,
    pub thickness_mm: i64,
    pub material: String,
    pub quantity: i32,
}

#[derive(Deserialize, Clone)]
pub struct OptimizeCutsRequest {
    pub cuts: Vec<CutRequest>,
    #[serde(default = "default_kerf")]
    pub kerf_width_mm: i64,
    #[serde(default = "default_min_remainder")]
    pub min_remainder_width_mm: i64,
    #[serde(default = "default_min_remainder")]
    pub min_remainder_height_mm: i64,
}

#[derive(Clone)]
struct OptimizationState {
    used_planks: Vec<UsedPlank>,
    free_rects_by_plank: HashMap<i64, Vec<FreeRect>>,
    unplaced_cuts: Vec<(usize, CutRequest)>,
    score: i64,
}

fn default_kerf() -> i64 {
    3
}
fn default_min_remainder() -> i64 {
    100
}

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

#[derive(Serialize, Clone)]
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

#[derive(Serialize, Clone)]
pub struct OptimizeCutsResponse {
    pub success: bool,
    pub efficiency_percent: f64,
    pub total_cuts_placed: usize,
    pub total_cuts_requested: usize,

    pub used_planks: Vec<UsedPlank>,
    pub unplaced_cuts: Vec<(usize, CutRequest)>,
}

#[derive(Clone, Debug)]
struct FreeRect {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

pub async fn optimize_cuts(
    State(state): State<AppState>,
    Json(req): Json<OptimizeCutsRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(cuts = %req.cuts.len(), "Optimizing cuts");

    for (idx, cut_req) in req.cuts.iter().enumerate() {
        if cut_req.width_mm < 10 || cut_req.width_mm > MAX_DIMENSION {
            return Err(AppError::Validation(format!(
                "Cut {}: width must be 10-{}mm",
                idx, MAX_DIMENSION
            )));
        }
        if cut_req.height_mm < 10 || cut_req.height_mm > MAX_DIMENSION {
            return Err(AppError::Validation(format!(
                "Cut {}: height must be 10-{}mm",
                idx, MAX_DIMENSION
            )));
        }
        if cut_req.thickness_mm < 1 || cut_req.thickness_mm > MAX_THICKNESS {
            return Err(AppError::Validation(format!(
                "Cut {}: thickness must be 1-{}mm",
                idx, MAX_THICKNESS
            )));
        }
        if cut_req.quantity < 1 || cut_req.quantity > 1000 {
            return Err(AppError::Validation(format!(
                "Cut {}: quantity must be 1-1000",
                idx
            )));
        }
    }

    let inventory = sqlx::query_as::<_, Leftover>(
        "SELECT * FROM leftovers WHERE width_mm >= 10 AND height_mm >= 10 ORDER BY width_mm * height_mm ASC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(AppError::Database)?;

    let result = tokio::task::spawn_blocking(move || {
        let (req_material, req_thickness) = if let Some(first) = req.cuts.first() {
            (first.material.clone(), first.thickness_mm)
        } else {
            ("Unknown".to_string(), 18)
        };

        let mut current_inventory = inventory.clone();
        let mut response = run_optimization_sync(req.clone(), current_inventory.clone());

        let mut extra_sheets = 0;
        const MAX_EXTRA_SHEETS: i64 = 10;

        while !response.unplaced_cuts.is_empty() && extra_sheets < MAX_EXTRA_SHEETS {
            extra_sheets += 1;

            let new_sheet = Leftover {
                id: -extra_sheets,
                width_mm: FULL_SHEET_W,
                height_mm: FULL_SHEET_H,
                thickness_mm: req_thickness,
                material: req_material.clone(),
                notes: Some("New Full Sheet".to_string()),
                created_at: String::new(),
            };

            current_inventory.push(new_sheet);

            tracing::info!(
                "Unplaced items found. Adding Virtual Sheet #{}",
                extra_sheets
            );
            response = run_optimization_sync(req.clone(), current_inventory.clone());
        }

        response
    })
    .await
    .map_err(|e| AppError::Validation(format!("Optimization task failed: {}", e)))?;

    Ok(Json(result))
}

fn run_optimization_sync_asd(
    req: OptimizeCutsRequest,
    inventory: Vec<Leftover>,
) -> OptimizeCutsResponse {
    let mut all_cuts = Vec::new();
    for (idx, cut_req) in req.cuts.iter().enumerate() {
        for _ in 0..cut_req.quantity {
            all_cuts.push((idx, cut_req.clone()));
        }
    }

    all_cuts.sort_by(|a, b| {
        let area_a = a.1.width_mm * a.1.height_mm;
        let area_b = b.1.width_mm * b.1.height_mm;
        match area_b.cmp(&area_a) {
            Ordering::Equal => {
                let max_dim_a = a.1.width_mm.max(a.1.height_mm);
                let max_dim_b = b.1.width_mm.max(b.1.height_mm);
                max_dim_b.cmp(&max_dim_a)
            }
            other => other,
        }
    });

    let mut used_planks: Vec<UsedPlank> = Vec::new();
    let mut unplaced_cuts: Vec<(usize, CutRequest)> = Vec::new();

    let mut plank_pools: HashMap<(String, i64), Vec<(Leftover, Vec<FreeRect>)>> = HashMap::new();

    for plank in inventory.into_iter() {
        let key = (plank.material.to_lowercase(), plank.thickness_mm);
        let free_rects = vec![FreeRect {
            x: 0,
            y: 0,
            width: plank.width_mm,
            height: plank.height_mm,
        }];
        plank_pools
            .entry(key)
            .or_insert_with(Vec::new)
            .push((plank, free_rects));
    }

    for (orig_idx, cut) in all_cuts {
        let key = (cut.material.to_lowercase(), cut.thickness_mm);

        if let Some(planks) = plank_pools.get_mut(&key) {
            let mut placed = false;

            planks.sort_by(|(plank_a, _), (plank_b, _)| {
                let a_used = used_planks.iter().any(|p| p.resto_id == plank_a.id);
                let b_used = used_planks.iter().any(|p| p.resto_id == plank_b.id);

                match (a_used, b_used) {
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    _ => {
                        let area_a = plank_a.width_mm * plank_a.height_mm;
                        let area_b = plank_b.width_mm * plank_b.height_mm;
                        area_a.cmp(&area_b)
                    }
                }
            });

            println!("Cut {}x{} - Sorted planks:", cut.width_mm, cut.height_mm);
            for (i, (plank, _)) in planks.iter().take(5).enumerate() {
                println!(
                    "  {}. Plank ID={} {}x{}mm ({}mm²)",
                    i + 1,
                    plank.id,
                    plank.width_mm,
                    plank.height_mm,
                    plank.width_mm * plank.height_mm
                );
            }

            for (plank, free_rects) in planks.iter_mut() {
                if free_rects.is_empty() {
                    continue;
                }

                let orientations = [
                    (cut.width_mm, cut.height_mm, false),
                    (cut.height_mm, cut.width_mm, true),
                ];

                let mut best_rect_idx: Option<usize> = None;
                let mut best_waste = i64::MAX;
                let mut chosen_orientation = (0, 0, false);

                for (w, h, rotated) in orientations {
                    for (i, rect) in free_rects.iter().enumerate() {
                        if rect.width >= w && rect.height >= h {
                            let waste = (rect.width * rect.height) - (w * h);
                            if waste < best_waste {
                                best_waste = waste;
                                best_rect_idx = Some(i);
                                chosen_orientation = (w, h, rotated);
                            }
                        }
                    }
                }

                if let Some(rect_idx) = best_rect_idx {
                    let (w, h, rotated) = chosen_orientation;
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

                    let kerf = req.kerf_width_mm;
                    let remain_w = rect.width.saturating_sub(w + kerf);
                    let remain_h = rect.height.saturating_sub(h + kerf);

                    let bottom_area_h_split = rect.width * remain_h;
                    let right_area_v_split = remain_w * rect.height;

                    let (new_right, new_bottom) = if bottom_area_h_split >= right_area_v_split {
                        (
                            FreeRect {
                                x: rect.x + w + kerf,
                                y: rect.y,
                                width: remain_w,
                                height: h,
                            },
                            FreeRect {
                                x: rect.x,
                                y: rect.y + h + kerf,
                                width: rect.width,
                                height: remain_h,
                            },
                        )
                    } else {
                        (
                            FreeRect {
                                x: rect.x + w + kerf,
                                y: rect.y,
                                width: remain_w,
                                height: rect.height,
                            },
                            FreeRect {
                                x: rect.x,
                                y: rect.y + h + kerf,
                                width: w,
                                height: remain_h,
                            },
                        )
                    };

                    let min_useful_size = 30;
                    if new_right.width >= min_useful_size && new_right.height >= min_useful_size {
                        free_rects.push(new_right);
                    }
                    if new_bottom.width >= min_useful_size && new_bottom.height >= min_useful_size {
                        free_rects.push(new_bottom);
                    }

                    if let Some(plank_entry) =
                        used_planks.iter_mut().find(|p| p.resto_id == plank.id)
                    {
                        plank_entry.cuts.push(placed_cut);
                    } else {
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

            if !placed {
                unplaced_cuts.push((orig_idx, cut));
            }
        } else {
            unplaced_cuts.push((orig_idx, cut));
        }
    }

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

    let total_cuts_placed =
        req.cuts.iter().map(|c| c.quantity as usize).sum::<usize>() - unplaced_cuts.len();
    let total_area_used: i64 = used_planks.iter().map(|p| p.used_area_mm2).sum();
    let total_area_available: i64 = used_planks.iter().map(|p| p.total_area_mm2).sum();
    let efficiency = if total_area_available > 0 {
        (total_area_used as f64 / total_area_available as f64) * 100.0
    } else {
        0.0
    };

    OptimizeCutsResponse {
        success: true,
        used_planks,
        unplaced_cuts,
        efficiency_percent: efficiency,
        total_cuts_placed,
        total_cuts_requested: req.cuts.iter().map(|c| c.quantity as usize).sum(),
    }
}

impl OptimizationState {
    fn new(inventory: &Vec<Leftover>) -> Self {
        let mut free_rects = HashMap::new();
        for plank in inventory {
            free_rects.insert(
                plank.id,
                vec![FreeRect {
                    x: 0,
                    y: 0,
                    width: plank.width_mm,
                    height: plank.height_mm,
                }],
            );
        }

        Self {
            used_planks: Vec::new(),
            free_rects_by_plank: free_rects,
            unplaced_cuts: Vec::new(),
            score: 0,
        }
    }

    fn calculate_efficiency(&self, total_area_available: i64) -> f64 {
        let used_area: i64 = self.used_planks.iter().map(|p| p.used_area_mm2).sum();
        if total_area_available > 0 {
            (used_area as f64 / total_area_available as f64) * 100.0
        } else {
            0.0
        }
    }
}

fn find_all_placements(
    state: &OptimizationState,
    cut: &CutRequest,
    inventory: &Vec<Leftover>,
    req: &OptimizeCutsRequest,
) -> Vec<(i64, usize, bool)> {
    let mut options = Vec::new();
    let mat_key = cut.material.to_lowercase();

    for plank in inventory {
        if plank.material.to_lowercase() != mat_key || plank.thickness_mm != cut.thickness_mm {
            continue;
        }

        if let Some(free_rects) = state.free_rects_by_plank.get(&plank.id) {
            for (i, rect) in free_rects.iter().enumerate() {
                if rect.width >= cut.width_mm && rect.height >= cut.height_mm {
                    options.push((plank.id, i, false));
                }
                if rect.width >= cut.height_mm && rect.height >= cut.width_mm {
                    options.push((plank.id, i, true));
                }
            }
        }
    }

    options.sort_by(|a, b| {
        let a_used = state.used_planks.iter().any(|p| p.resto_id == a.0);
        let b_used = state.used_planks.iter().any(|p| p.resto_id == b.0);

        match (a_used, b_used) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => {
                let a_plank = inventory.iter().find(|p| p.id == a.0).unwrap();
                let b_plank = inventory.iter().find(|p| p.id == b.0).unwrap();
                let area_a = a_plank.width_mm * a_plank.height_mm;
                let area_b = b_plank.width_mm * b_plank.height_mm;
                area_a.cmp(&area_b)
            }
        }
    });

    options.into_iter().take(3).collect()
}

fn apply_cut(
    state: &mut OptimizationState,
    cut: &CutRequest,
    plank_id: i64,
    rect_idx: usize,
    rotated: bool,
    req: &OptimizeCutsRequest,
    inventory: &Vec<Leftover>,
    orig_idx: usize,
) {
    let free_rects = state.free_rects_by_plank.get_mut(&plank_id).unwrap();
    let rect = free_rects.remove(rect_idx);

    let w = if rotated { cut.height_mm } else { cut.width_mm };
    let h = if rotated { cut.width_mm } else { cut.height_mm };

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

    if let Some(existing) = state
        .used_planks
        .iter_mut()
        .find(|p| p.resto_id == plank_id)
    {
        existing.cuts.push(placed_cut);
        existing.used_area_mm2 += (w * h) + ((w + h) * req.kerf_width_mm);
    } else {
        let raw_plank = inventory.iter().find(|p| p.id == plank_id).unwrap();
        state.used_planks.push(UsedPlank {
            resto_id: plank_id,
            width_mm: raw_plank.width_mm,
            height_mm: raw_plank.height_mm,
            thickness_mm: raw_plank.thickness_mm,
            material: raw_plank.material.clone(),
            cuts: vec![placed_cut],
            waste_percent: 0.0,
            total_area_mm2: raw_plank.width_mm * raw_plank.height_mm,
            used_area_mm2: (w * h) + ((w + h) * req.kerf_width_mm),
        });
    }

    let kerf = req.kerf_width_mm;
    let remain_right_w = rect.width - w - kerf;
    let remain_bottom_h = rect.height - h - kerf;

    let (new_right, new_bottom) = if remain_right_w * rect.height > rect.width * remain_bottom_h {
        (
            FreeRect {
                x: rect.x + w + kerf,
                y: rect.y,
                width: remain_right_w,
                height: rect.height,
            },
            FreeRect {
                x: rect.x,
                y: rect.y + h + kerf,
                width: w,
                height: remain_bottom_h,
            },
        )
    } else {
        (
            FreeRect {
                x: rect.x + w + kerf,
                y: rect.y,
                width: remain_right_w,
                height: h,
            },
            FreeRect {
                x: rect.x,
                y: rect.y + h + kerf,
                width: rect.width,
                height: remain_bottom_h,
            },
        )
    };

    if new_right.width >= 30 && new_right.height >= 30 {
        free_rects.push(new_right);
    }
    if new_bottom.width >= 30 && new_bottom.height >= 30 {
        free_rects.push(new_bottom);
    }
}

fn run_optimization_sync(
    req: OptimizeCutsRequest,
    inventory: Vec<Leftover>,
) -> OptimizeCutsResponse {
    let mut all_cuts = Vec::new();
    for (idx, cut_req) in req.cuts.iter().enumerate() {
        for _ in 0..cut_req.quantity {
            all_cuts.push((idx, cut_req.clone()));
        }
    }
    all_cuts.sort_by(|a, b| (b.1.width_mm * b.1.height_mm).cmp(&(a.1.width_mm * a.1.height_mm)));

    let initial_state = OptimizationState::new(&inventory);
    let mut beam = vec![initial_state];

    for (orig_idx, cut) in all_cuts {
        let mut next_generation_candidates = Vec::new();

        for state in beam {
            let candidates = find_all_placements(&state, &cut, &inventory, &req);

            if candidates.is_empty() {
                let mut new_state = state.clone();
                new_state.unplaced_cuts.push((orig_idx, cut.clone()));
                next_generation_candidates.push(new_state);
            } else {
                for (plank_id, rect_idx, rotated) in candidates {
                    let mut new_state = state.clone();
                    apply_cut(
                        &mut new_state,
                        &cut,
                        plank_id,
                        rect_idx,
                        rotated,
                        &req,
                        &inventory,
                        orig_idx,
                    );
                    next_generation_candidates.push(new_state);
                }
            }
        }

        next_generation_candidates.sort_by(|a, b| {
            let unplaced_cmp = a.unplaced_cuts.len().cmp(&b.unplaced_cuts.len());
            if unplaced_cmp != Ordering::Equal {
                return unplaced_cmp;
            }
            let eff_a = a.calculate_efficiency(1);
            let eff_b = b.calculate_efficiency(1);
            eff_b.partial_cmp(&eff_a).unwrap_or(Ordering::Equal)
        });

        beam = next_generation_candidates
            .into_iter()
            .take(BEAM_WIDTH)
            .collect();
    }

    let best_state = beam.first().unwrap().clone();

    let total_area_available: i64 = inventory
        .iter()
        .filter(|p| best_state.used_planks.iter().any(|u| u.resto_id == p.id))
        .map(|p| p.width_mm * p.height_mm)
        .sum();

    let unplaced_count = best_state.unplaced_cuts.len();
    let final_efficiency = best_state.calculate_efficiency(total_area_available);

    OptimizeCutsResponse {
        success: true,
        used_planks: best_state.used_planks,
        unplaced_cuts: best_state.unplaced_cuts,
        efficiency_percent: final_efficiency,
        total_cuts_placed: req.cuts.iter().map(|c| c.quantity as usize).sum::<usize>()
            - unplaced_count,
        total_cuts_requested: req.cuts.iter().map(|c| c.quantity as usize).sum(),
    }
}
