import sys
import json
import os
from contextlib import contextmanager

@contextmanager
def suppress_stdout():
    with open(os.devnull, "w") as devnull:
        old_stdout = sys.stdout
        sys.stdout = devnull
        try:
            yield
        finally:
            sys.stdout = old_stdout

try:
    from ortools.sat.python import cp_model
except ImportError:
    sys.stderr.write("Error: OR-Tools not installed.\n")
    sys.exit(1)

def solve_packing(data):
    van = data['van']
    items = data['items']
    
    model = cp_model.CpModel()

    # --- 1. SETUP VARIABLES ---
    VAN_L = int(van['length_mm'])
    VAN_W = int(van['width_mm'])
    VAN_H = int(van['height_mm'])
    
    # Wheel Wells
    ww_h = int(van.get('wheel_well_height_mm', 0) or 0)
    ww_w = int(van.get('wheel_well_width_mm', 0) or 0)
    ww_start = int(van.get('wheel_well_start_x_mm', VAN_L) or VAN_L)
    
    all_boxes = [] 
    item_indices = []

    # --- 2. ADD ITEMS (With "Is Packed" logic) ---
    for i, item in enumerate(items):
        d_l = int(item['length_mm'])
        d_w = int(item['width_mm'])
        d_h = int(item['height_mm'])
        
        # Determine if item fits at all (physically)
        perms = [
            (d_l, d_w, d_h), (d_l, d_h, d_w),
            (d_w, d_l, d_h), (d_w, d_h, d_l),
            (d_h, d_l, d_w), (d_h, d_w, d_l)
        ]
        valid_perms = sorted(list(set(
            (l, w, h) for (l, w, h) in perms
            if l <= VAN_L and w <= VAN_W and h <= VAN_H
        )))
        
        # Variable: Is this item packed?
        is_packed = model.NewBoolVar(f'is_packed_{i}')
        
        if not valid_perms:
            # Physically impossible to fit -> Force unpacked
            model.Add(is_packed == 0)
            # Create dummy vars to satisfy structure
            item_vars = {
                'x': model.NewConstant(0), 'y': model.NewConstant(0), 'z': model.NewConstant(0),
                'xe': model.NewConstant(0), 'ye': model.NewConstant(0), 'ze': model.NewConstant(0),
                'l': model.NewConstant(0), 'w': model.NewConstant(0), 'h': model.NewConstant(0),
                'is_packed': is_packed,
                'is_obstacle': False
            }
            all_boxes.append(item_vars)
            item_indices.append(i)
            continue

        # Orientation Variables
        orient_bools = [model.NewBoolVar(f'o_{i}_{k}') for k in range(len(valid_perms))]
        
        # If packed, must pick exactly one orientation
        model.Add(sum(orient_bools) == 1).OnlyEnforceIf(is_packed)
        
        # Rotation constraints
        if not item.get('rotation_allowed', True):
            forced_idx = -1
            for k, (l, w, h) in enumerate(valid_perms):
                if l == d_l and w == d_w and h == d_h:
                    forced_idx = k; break
            if forced_idx >= 0: 
                model.Add(orient_bools[forced_idx] == 1).OnlyEnforceIf(is_packed)
            else:
                model.Add(is_packed == 0) # Doesn't fit in required orientation

        # Effective Dimensions
        eff_l = model.NewIntVar(0, max(d_l,d_w,d_h), f'l_{i}')
        eff_w = model.NewIntVar(0, max(d_l,d_w,d_h), f'w_{i}')
        eff_h = model.NewIntVar(0, max(d_l,d_w,d_h), f'h_{i}')
        
        for k, (vl, vw, vh) in enumerate(valid_perms):
            model.Add(eff_l == vl).OnlyEnforceIf([is_packed, orient_bools[k]])
            model.Add(eff_w == vw).OnlyEnforceIf([is_packed, orient_bools[k]])
            model.Add(eff_h == vh).OnlyEnforceIf([is_packed, orient_bools[k]])

        # Position Variables
        # Note: If not packed, we force them to 0 to keep the solution clean
        x = model.NewIntVar(0, VAN_L, f'x_{i}')
        y = model.NewIntVar(0, VAN_H, f'y_{i}')
        z = model.NewIntVar(0, VAN_W, f'z_{i}')
        
        xe = model.NewIntVar(0, VAN_L, f'xe_{i}')
        ye = model.NewIntVar(0, VAN_H, f'ye_{i}')
        ze = model.NewIntVar(0, VAN_W, f'ze_{i}')
        
        # Geometry constraints (Active only if packed)
        model.Add(xe == x + eff_l).OnlyEnforceIf(is_packed)
        model.Add(ye == y + eff_h).OnlyEnforceIf(is_packed)
        model.Add(ze == z + eff_w).OnlyEnforceIf(is_packed)
        
        # If NOT packed, force dims/pos to 0
        model.Add(x == 0).OnlyEnforceIf(is_packed.Not())
        model.Add(y == 0).OnlyEnforceIf(is_packed.Not())
        model.Add(z == 0).OnlyEnforceIf(is_packed.Not())
        model.Add(eff_l == 0).OnlyEnforceIf(is_packed.Not())
        
        all_boxes.append({'x': x, 'y': y, 'z': z, 'xe': xe, 'ye': ye, 'ze': ze, 
                          'is_packed': is_packed, 'is_obstacle': False})
        item_indices.append(i)

    # --- 3. ADD WHEEL WELLS (Fixed Obstacles) ---
    if ww_h > 0 and ww_w > 0:
        # Left Well
        all_boxes.append({
            'x': model.NewConstant(ww_start), 'xe': model.NewConstant(VAN_L),
            'y': model.NewConstant(0),        'ye': model.NewConstant(ww_h),
            'z': model.NewConstant(0),        'ze': model.NewConstant(ww_w),
            'is_packed': model.NewConstant(1), 'is_obstacle': True
        })
        # Right Well
        all_boxes.append({
            'x': model.NewConstant(ww_start), 'xe': model.NewConstant(VAN_L),
            'y': model.NewConstant(0),        'ye': model.NewConstant(ww_h),
            'z': model.NewConstant(VAN_W - ww_w), 'ze': model.NewConstant(VAN_W),
            'is_packed': model.NewConstant(1), 'is_obstacle': True
        })

    # --- 4. NO OVERLAP (Pairwise) ---
    for i in range(len(all_boxes)):
        for j in range(i + 1, len(all_boxes)):
            b1 = all_boxes[i]
            b2 = all_boxes[j]
            
            # If either is NOT packed, no collision check needed
            # We enforce separation only if BOTH are packed
            
            left   = model.NewBoolVar(f'{i}_L_{j}') 
            right  = model.NewBoolVar(f'{i}_R_{j}')
            below  = model.NewBoolVar(f'{i}_B_{j}')
            above  = model.NewBoolVar(f'{i}_A_{j}')
            front  = model.NewBoolVar(f'{i}_F_{j}')
            back   = model.NewBoolVar(f'{i}_K_{j}')

            # Geometry definitions
            model.Add(b1['xe'] <= b2['x']).OnlyEnforceIf(left)
            model.Add(b2['xe'] <= b1['x']).OnlyEnforceIf(right)
            model.Add(b1['ye'] <= b2['y']).OnlyEnforceIf(below)
            model.Add(b2['ye'] <= b1['y']).OnlyEnforceIf(above)
            model.Add(b1['ze'] <= b2['z']).OnlyEnforceIf(front) # Z axis logic
            model.Add(b2['ze'] <= b1['z']).OnlyEnforceIf(back)

            # Constraint: Must be separated in at least one dimension IF both packed
            # Logic: (Not Packed 1) OR (Not Packed 2) OR (Separated)
            model.AddBoolOr([
                b1['is_packed'].Not(), 
                b2['is_packed'].Not(), 
                left, right, below, above, front, back
            ])

    # --- 5. OBJECTIVE: MAXIMIZE PACKED + GRAVITY ---
    
    # 1. Maximize Number of Packed Items (Most important!)
    # Weight: 1,000,000 per item
    packed_bonus = sum(all_boxes[idx]['is_packed'] for idx in item_indices)
    
    # 2. Minimize Coordinates (Gravity & Compactness)
    # Costs are negative in maximization
    position_costs = []
    for idx in item_indices:
        b = all_boxes[idx]
        # Only count cost if packed
        # Cost Y (Height) > X (Length) > Z (Width)
        # We use a temporary variable to hold the cost * is_packed
        
        # Scaling: Y*10000, X*100, Z*1
        # This keeps items low and to the back
        cost_y = model.NewIntVar(0, VAN_H * 10000, f'cy_{idx}')
        cost_x = model.NewIntVar(0, VAN_L * 100, f'cx_{idx}')
        
        model.Add(cost_y == b['y'] * 10000).OnlyEnforceIf(b['is_packed'])
        model.Add(cost_y == 0).OnlyEnforceIf(b['is_packed'].Not())
        
        model.Add(cost_x == b['x'] * 100).OnlyEnforceIf(b['is_packed'])
        model.Add(cost_x == 0).OnlyEnforceIf(b['is_packed'].Not())
        
        position_costs.append(cost_y)
        position_costs.append(cost_x)

    # Final Objective
    # Maximize (PackedCount * 1 Billion) - (Total Position Cost)
    # This ensures we NEVER sacrifice an item just to improve position
    model.Maximize(packed_bonus * 1000000000 - sum(position_costs))

    # --- 6. SOLVE ---
    solver = cp_model.CpSolver()
    solver.parameters.max_time_in_seconds = 10.0
    solver.parameters.log_search_progress = False 
    
    with suppress_stdout():
        status = solver.Solve(model)

    # --- 7. OUTPUT ---
    if status in (cp_model.OPTIMAL, cp_model.FEASIBLE):
        placed_items = []
        unplaced_items = []
        
        for idx in item_indices:
            v = all_boxes[idx]
            raw = items[idx]
            
            if solver.Value(v['is_packed']):
                fl = solver.Value(v['xe']) - solver.Value(v['x'])
                fw = solver.Value(v['ze']) - solver.Value(v['z'])
                fh = solver.Value(v['ye']) - solver.Value(v['y'])
                
                rx, ry = 0, 0
                if fh != int(raw['height_mm']): rx = 90
                elif fw != int(raw['width_mm']): ry = 90

                placed_items.append({
                    "item": raw,
                    "position": {
                        "x": solver.Value(v['x']),
                        "y": solver.Value(v['y']),
                        "z": solver.Value(v['z'])
                    },
                    "rotation": { "x": rx, "y": ry, "z": 0 },
                    "placed_length": fl,
                    "placed_width": fw,
                    "placed_height": fh,
                    "level": 0
                })
            else:
                unplaced_items.append(raw)
            
        return {
            "success": True,
            "plan": {
                "items": placed_items,
                "total_weight": 0.0, 
                "utilization_percent": 0.0,
                "van_volume": VAN_L * VAN_W * VAN_H,
                "used_volume": 0
            },
            "unplaced_items": unplaced_items, # Explicit list for multi-trip
            "warnings": [f"Items not packed: {len(unplaced_items)}"] if unplaced_items else []
        }
    else:
        return {
            "success": False,
            "plan": None,
            "warnings": [f"Optimization failed. Status: {solver.StatusName(status)}"]
        }

if __name__ == "__main__":
    try:
        input_str = sys.stdin.read()
        if input_str:
            print(json.dumps(solve_packing(json.loads(input_str))))
    except Exception as e:
        print(json.dumps({"success": False, "warnings": [f"Python Error: {str(e)}"]}))