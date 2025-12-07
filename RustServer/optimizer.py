import sys
import json
import os
import io
from contextlib import contextmanager

# 1. FORCE UTF-8
sys.stdin = io.TextIOWrapper(sys.stdin.buffer, encoding='utf-8')
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

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
    sys.stderr.write("Error: OR-Tools not installed. Run 'pip install ortools'\n")
    sys.exit(1)

def to_int(val):
    try:
        return int(float(val))
    except (ValueError, TypeError):
        return 0

def solve_packing(data):
    van = data.get('van', {})
    raw_items = data.get('items', [])

    # --- PRE-PROCESSING: SORTING FOR STABILITY ---
    # We sort by Area (Footprint) first, then Volume. 
    # Logic: Large flat items (bases) should be processed first to act as floors for others.
    indexed_items = []
    for idx, item in enumerate(raw_items):
        l, w, h = to_int(item.get('length_mm', 0)), to_int(item.get('width_mm', 0)), to_int(item.get('height_mm', 0))
        vol = l * w * h
        # Heuristic: Max face area (for stability)
        max_area = max(l*w, w*h, l*h)
        
        indexed_items.append({
            'original_idx': idx,
            'data': item,
            'vol': vol,
            'max_area': max_area,
            'sig': (l, w, h, to_int(item.get('weight_kg', 0)))
        })
    
    # Sort: Largest Area first, then Largest Volume
    indexed_items.sort(key=lambda x: (x['max_area'], x['vol']), reverse=True)

    model = cp_model.CpModel()

    # Dimensions
    VAN_L = to_int(van.get('length_mm', 0))
    VAN_W = to_int(van.get('width_mm', 0))
    VAN_H = to_int(van.get('height_mm', 0))
    
    # Wheel Wells (Obstacles)
    ww_h = to_int(van.get('wheel_well_height_mm', 0))
    ww_w = to_int(van.get('wheel_well_width_mm', 0))
    ww_start_val = van.get('wheel_well_start_x_mm')
    ww_start = to_int(ww_start_val) if ww_start_val is not None else VAN_L
    
    boxes = [] 

    # --- CREATE VARIABLES ---
    for i, item_wrapper in enumerate(indexed_items):
        item = item_wrapper['data']
        d_l, d_w, d_h = to_int(item.get('length_mm', 0)), to_int(item.get('width_mm', 0)), to_int(item.get('height_mm', 0))
        
        # Valid Orientations
        perms = [(d_l, d_w, d_h), (d_l, d_h, d_w), (d_w, d_l, d_h), (d_w, d_h, d_l), (d_h, d_l, d_w), (d_h, d_w, d_l)]
        valid_perms = sorted(list(set((l, w, h) for (l, w, h) in perms if l <= VAN_L and w <= VAN_W and h <= VAN_H)))
        
        is_packed = model.NewBoolVar(f'packed_{i}')
        
        if not valid_perms:
            model.Add(is_packed == 0)
            boxes.append({'is_packed': is_packed, 'idx': i, 'is_obstacle': False, 'wrapper': item_wrapper, 'x': model.NewConstant(0), 'y': model.NewConstant(0), 'z': model.NewConstant(0), 'xe': model.NewConstant(0), 'ye': model.NewConstant(0), 'ze': model.NewConstant(0)})
            continue

        # Coordinates
        x = model.NewIntVar(0, VAN_L, f'x_{i}')
        y = model.NewIntVar(0, VAN_H, f'y_{i}')
        z = model.NewIntVar(0, VAN_W, f'z_{i}')

        # Effective Dimensions
        eff_l = model.NewIntVar(0, max(d_l, d_w, d_h), f'len_{i}')
        eff_w = model.NewIntVar(0, max(d_l, d_w, d_h), f'wid_{i}')
        eff_h = model.NewIntVar(0, max(d_l, d_w, d_h), f'hgt_{i}')

        # Orientation Selection
        orient_bools = [model.NewBoolVar(f'rot_{i}_{k}') for k in range(len(valid_perms))]
        model.Add(sum(orient_bools) == 1).OnlyEnforceIf(is_packed)
        
        for k, (vl, vw, vh) in enumerate(valid_perms):
            model.Add(eff_l == vl).OnlyEnforceIf(orient_bools[k])
            model.Add(eff_w == vw).OnlyEnforceIf(orient_bools[k])
            model.Add(eff_h == vh).OnlyEnforceIf(orient_bools[k])

        # Rotation Allowed Constraint
        if not item.get('rotation_allowed', True):
            forced = False
            for k, (vl, vw, vh) in enumerate(valid_perms):
                # Only allow orientations where Height matches the original Height
                # This ensures "This Side Up" compliance
                if vh == d_h: 
                    model.Add(orient_bools[k] == 1).OnlyEnforceIf(is_packed)
                    forced = True
            if not forced: model.Add(is_packed == 0)

        # Ends
        xe, ye, ze = model.NewIntVar(0, VAN_L, f'xe_{i}'), model.NewIntVar(0, VAN_H, f'ye_{i}'), model.NewIntVar(0, VAN_W, f'ze_{i}')
        model.Add(xe == x + eff_l).OnlyEnforceIf(is_packed)
        model.Add(ye == y + eff_h).OnlyEnforceIf(is_packed)
        model.Add(ze == z + eff_w).OnlyEnforceIf(is_packed)

        # --- CENTER OF GRAVITY (Scaled by 2) ---
        # We need this to ensure the "middle" of the box rests on something.
        cx = model.NewIntVar(0, 2 * VAN_L, f'cx_{i}')
        cz = model.NewIntVar(0, 2 * VAN_W, f'cz_{i}')
        model.Add(cx == 2 * x + eff_l).OnlyEnforceIf(is_packed)
        model.Add(cz == 2 * z + eff_w).OnlyEnforceIf(is_packed)

        # Unpacked Cleanup
        for v in [x, y, z, eff_l, eff_w, eff_h]: model.Add(v == 0).OnlyEnforceIf(is_packed.Not())

        boxes.append({
            'idx': i, 'wrapper': item_wrapper, 'is_packed': is_packed,
            'x': x, 'y': y, 'z': z, 'xe': xe, 'ye': ye, 'ze': ze,
            'cx': cx, 'cz': cz, 'eff_h': eff_h, 'eff_l': eff_l, 'eff_w': eff_w,
            'is_obstacle': False
        })

    # --- OBSTACLES (Wheel Wells) ---
    if ww_h > 0 and ww_w > 0:
        for z_pos in [0, VAN_W - ww_w]:
            boxes.append({
                'is_obstacle': True, 'is_packed': model.NewConstant(1),
                'x': model.NewConstant(ww_start), 'xe': model.NewConstant(VAN_L),
                'y': model.NewConstant(0),        'ye': model.NewConstant(ww_h),
                'z': model.NewConstant(z_pos),    'ze': model.NewConstant(z_pos + ww_w)
            })

    # --- NON-OVERLAP ---
    for i in range(len(boxes)):
        for j in range(i + 1, len(boxes)):
            b1, b2 = boxes[i], boxes[j]
            if b1['is_obstacle'] and b2['is_obstacle']: continue

            left, right = model.NewBoolVar(f'{i}L{j}'), model.NewBoolVar(f'{i}R{j}')
            below, above = model.NewBoolVar(f'{i}B{j}'), model.NewBoolVar(f'{i}A{j}')
            front, back  = model.NewBoolVar(f'{i}F{j}'), model.NewBoolVar(f'{i}K{j}')

            model.Add(b1['xe'] <= b2['x']).OnlyEnforceIf(left)
            model.Add(b2['xe'] <= b1['x']).OnlyEnforceIf(right)
            model.Add(b1['ye'] <= b2['y']).OnlyEnforceIf(below)
            model.Add(b2['ye'] <= b1['y']).OnlyEnforceIf(above)
            model.Add(b1['ze'] <= b2['z']).OnlyEnforceIf(front)
            model.Add(b2['ze'] <= b1['z']).OnlyEnforceIf(back)

            model.AddBoolOr([left, right, below, above, front, back, b1['is_packed'].Not(), b2['is_packed'].Not()])

    # --- PHYSICAL STABILITY: CENTER OF GRAVITY SUPPORT ---
    # For every item i, if it is NOT on the floor (y > 0), it MUST be supported by another item j.
    # Support definition:
    # 1. j is directly below i (j_ye == i_y)
    # 2. i's center point (cx, cz) is strictly within j's footprint (j_x <= cx <= j_xe)
    
    for i in range(len(boxes)):
        if boxes[i]['is_obstacle']: continue
        b_i = boxes[i]
        
        # Helper: is_on_floor
        on_floor = model.NewBoolVar(f'{i}_on_floor')
        model.Add(b_i['y'] == 0).OnlyEnforceIf(on_floor)
        model.Add(b_i['y'] > 0).OnlyEnforceIf(on_floor.Not())
        
        supported_by_any = []
        
        for j in range(len(boxes)):
            if i == j: continue
            b_j = boxes[j]
            
            # Can j support i?
            supports = model.NewBoolVar(f'{j}_supports_{i}')
            
            # 1. Vertical contact
            model.Add(b_j['ye'] == b_i['y']).OnlyEnforceIf(supports)
            
            # 2. X Stability (Center of I is inside J)
            # cx is 2*center. bounds are x and xe. so 2*x <= cx <= 2*xe
            model.Add(b_j['x'] * 2 <= b_i['cx']).OnlyEnforceIf(supports)
            model.Add(b_i['cx'] <= b_j['xe'] * 2).OnlyEnforceIf(supports)
            
            # 3. Z Stability (Center of I is inside J)
            model.Add(b_j['z'] * 2 <= b_i['cz']).OnlyEnforceIf(supports)
            model.Add(b_i['cz'] <= b_j['ze'] * 2).OnlyEnforceIf(supports)
            
            # 4. J must be packed
            model.AddImplication(supports, b_j['is_packed'])
            
            supported_by_any.append(supports)
            
        # If packed, must be on floor OR supported by at least one object
        model.AddBoolOr([on_floor] + supported_by_any).OnlyEnforceIf(b_i['is_packed'])

    # --- OBJECTIVE FUNCTION ---
    obj_terms = []
    for box in boxes:
        if box['is_obstacle']: continue
        
        # 1. Maximize items packed (Highest Priority)
        obj_terms.append(box['is_packed'] * 1_000_000)
        
        # 2. Floor Priority (Heavy Y Penalty)
        # This prevents stacking unless absolutely necessary.
        obj_terms.append(box['y'] * -500)
        
        # 3. Wall Building (X Penalty)
        # Fill the back of the van first.
        obj_terms.append(box['x'] * -100)
        
        # 4. Compactness (Z Penalty)
        obj_terms.append(box['z'] * -10)

    model.Maximize(sum(obj_terms))

    # --- SOLVE ---
    solver = cp_model.CpSolver()
    # Increased time limit for stability calculations
    solver.parameters.max_time_in_seconds = 30.0 
    solver.parameters.num_search_workers = 8
    
    with suppress_stdout():
        status = solver.Solve(model)

    if status in (cp_model.OPTIMAL, cp_model.FEASIBLE):
        placed_items = []
        unplaced_items = []
        total_weight = 0.0
        used_volume = 0
        
        for box in boxes:
            if box.get('is_obstacle'): continue
            wrapper = box['wrapper']
            original = wrapper['data']
            
            if solver.Value(box['is_packed']):
                pl, pw, ph = solver.Value(box['eff_l']), solver.Value(box['eff_w']), solver.Value(box['eff_h'])
                orig_h, orig_w = to_int(original.get('height_mm', 0)), to_int(original.get('width_mm', 0))
                
                # Visual Rotation Logic
                rx, ry, rz = 0, 0, 0
                # If the effective height matches original width, we rotated X
                if ph == orig_w: rx = 90
                # If effective height matches original length, we rotated Y
                elif ph == to_int(original.get('length_mm', 0)): ry = 90

                placed_items.append({
                    "item": original,
                    "position": {"x": solver.Value(box['x']), "y": solver.Value(box['y']), "z": solver.Value(box['z'])},
                    "rotation": {"x": rx, "y": ry, "z": rz},
                    "placed_length": pl, "placed_width": pw, "placed_height": ph,
                    "level": 0
                })
                total_weight += float(original.get('weight_kg', 0) or 0)
                used_volume += (pl * pw * ph)
            else:
                unplaced_items.append(original)
        
        van_vol = VAN_L * VAN_W * VAN_H
        util = (used_volume / van_vol * 100) if van_vol > 0 else 0
            
        return {
            "success": True,
            "plan": {
                "items": placed_items,
                "total_weight": total_weight,
                "utilization_percent": util,
                "van_volume": van_vol,
                "used_volume": used_volume
            },
            "unplaced_items": unplaced_items,
            "warnings": [f"Items not packed: {len(unplaced_items)}"] if unplaced_items else []
        }
    else:
        return {"success": False, "plan": None, "warnings": [f"Optimization failed: {solver.StatusName(status)}"]}

if __name__ == "__main__":
    try:
        input_str = sys.stdin.read()
        if input_str:
            print(json.dumps(solve_packing(json.loads(input_str))))
    except Exception as e:
        print(json.dumps({"success": False, "warnings": [f"Python Error: {str(e)}"]}))