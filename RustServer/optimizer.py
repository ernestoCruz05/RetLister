import sys
import json
import os
import io
from contextlib import contextmanager

# 1. FORÇAR UTF-8 (Vital para Windows)
sys.stdin = io.TextIOWrapper(sys.stdin.buffer, encoding='utf-8')
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

# 2. SUPRIMIR LOGS
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
    """Converte seguramente qualquer número (float/str) para int"""
    try:
        return int(float(val))
    except (ValueError, TypeError):
        return 0

def solve_packing(data):
    van = data.get('van', {})
    items = data.get('items', [])
    
    model = cp_model.CpModel()

    # Setup Dimensões
    VAN_L = to_int(van.get('length_mm', 0))
    VAN_W = to_int(van.get('width_mm', 0))
    VAN_H = to_int(van.get('height_mm', 0))
    
    # Wheel Wells
    ww_h = to_int(van.get('wheel_well_height_mm', 0))
    ww_w = to_int(van.get('wheel_well_width_mm', 0))
    ww_start_val = van.get('wheel_well_start_x_mm')
    ww_start = to_int(ww_start_val) if ww_start_val is not None else VAN_L
    
    all_boxes = [] 
    item_indices = []

    # --- ADD ITEMS ---
    for i, item in enumerate(items):
        d_l = to_int(item.get('length_mm', 0))
        d_w = to_int(item.get('width_mm', 0))
        d_h = to_int(item.get('height_mm', 0))
        
        # Valid Orientations
        perms = [
            (d_l, d_w, d_h), (d_l, d_h, d_w), (d_w, d_l, d_h), 
            (d_w, d_h, d_l), (d_h, d_l, d_w), (d_h, d_w, d_l)
        ]
        
        # Filtrar o que cabe fisicamente
        valid_perms = sorted(list(set(
            (l, w, h) for (l, w, h) in perms
            if l <= VAN_L and w <= VAN_W and h <= VAN_H
        )))
        
        is_packed = model.NewBoolVar(f'is_packed_{i}')
        
        if not valid_perms:
            model.Add(is_packed == 0)
            # Dummy vars
            all_boxes.append({
                'x': model.NewConstant(0), 'y': model.NewConstant(0), 'z': model.NewConstant(0),
                'xe': model.NewConstant(0), 'ye': model.NewConstant(0), 'ze': model.NewConstant(0),
                'l': model.NewConstant(0), 'w': model.NewConstant(0), 'h': model.NewConstant(0),
                'is_packed': is_packed, 'is_obstacle': False
            })
            item_indices.append(i)
            continue

        # Orientation Selection
        orient_bools = [model.NewBoolVar(f'o_{i}_{k}') for k in range(len(valid_perms))]
        model.Add(sum(orient_bools) == 1).OnlyEnforceIf(is_packed)
        
        # Rotation constraints
        if not item.get('rotation_allowed', True):
            forced_idx = -1
            for k, (l, w, h) in enumerate(valid_perms):
                if l == d_l and w == d_w and h == d_h: forced_idx = k; break
            if forced_idx >= 0: model.Add(orient_bools[forced_idx] == 1).OnlyEnforceIf(is_packed)
            else: model.Add(is_packed == 0)

        # Effective Dimensions
        max_dim = max(d_l, d_w, d_h)
        eff_l = model.NewIntVar(0, max_dim, f'l_{i}')
        eff_w = model.NewIntVar(0, max_dim, f'w_{i}')
        eff_h = model.NewIntVar(0, max_dim, f'h_{i}')
        
        for k, (vl, vw, vh) in enumerate(valid_perms):
            model.Add(eff_l == vl).OnlyEnforceIf([is_packed, orient_bools[k]])
            model.Add(eff_w == vw).OnlyEnforceIf([is_packed, orient_bools[k]])
            model.Add(eff_h == vh).OnlyEnforceIf([is_packed, orient_bools[k]])

        # Position Variables
        x = model.NewIntVar(0, VAN_L, f'x_{i}')
        y = model.NewIntVar(0, VAN_H, f'y_{i}')
        z = model.NewIntVar(0, VAN_W, f'z_{i}')
        
        xe = model.NewIntVar(0, VAN_L, f'xe_{i}')
        ye = model.NewIntVar(0, VAN_H, f'ye_{i}')
        ze = model.NewIntVar(0, VAN_W, f'ze_{i}')
        
        # Geometry Logic
        model.Add(xe == x + eff_l).OnlyEnforceIf(is_packed)
        model.Add(ye == y + eff_h).OnlyEnforceIf(is_packed)
        model.Add(ze == z + eff_w).OnlyEnforceIf(is_packed)
        
        # Reset if not packed
        for v in [x, y, z, eff_l, eff_w, eff_h]:
            model.Add(v == 0).OnlyEnforceIf(is_packed.Not())
            
        all_boxes.append({
            'x': x, 'y': y, 'z': z, 'xe': xe, 'ye': ye, 'ze': ze, 
            'l': eff_l, 'w': eff_w, 'h': eff_h, 
            'is_packed': is_packed, 'is_obstacle': False
        })
        item_indices.append(i)

    # --- ADD OBSTACLES (Wheel Wells) ---
    if ww_h > 0 and ww_w > 0:
        # Left
        all_boxes.append({
            'x': model.NewConstant(ww_start), 'xe': model.NewConstant(VAN_L),
            'y': model.NewConstant(0), 'ye': model.NewConstant(ww_h),
            'z': model.NewConstant(0), 'ze': model.NewConstant(ww_w),
            'is_packed': model.NewConstant(1), 'is_obstacle': True
        })
        # Right
        all_boxes.append({
            'x': model.NewConstant(ww_start), 'xe': model.NewConstant(VAN_L),
            'y': model.NewConstant(0), 'ye': model.NewConstant(ww_h),
            'z': model.NewConstant(VAN_W - ww_w), 'ze': model.NewConstant(VAN_W),
            'is_packed': model.NewConstant(1), 'is_obstacle': True
        })

    # --- NO OVERLAP ---
    for i in range(len(all_boxes)):
        for j in range(i + 1, len(all_boxes)):
            b1, b2 = all_boxes[i], all_boxes[j]
            
            left, right = model.NewBoolVar(f'{i}L{j}'), model.NewBoolVar(f'{i}R{j}')
            below, above = model.NewBoolVar(f'{i}B{j}'), model.NewBoolVar(f'{i}A{j}')
            front, back = model.NewBoolVar(f'{i}F{j}'), model.NewBoolVar(f'{i}K{j}')

            model.Add(b1['xe'] <= b2['x']).OnlyEnforceIf(left)
            model.Add(b2['xe'] <= b1['x']).OnlyEnforceIf(right)
            model.Add(b1['ye'] <= b2['y']).OnlyEnforceIf(below)
            model.Add(b2['ye'] <= b1['y']).OnlyEnforceIf(above)
            model.Add(b1['ze'] <= b2['z']).OnlyEnforceIf(front)
            model.Add(b2['ze'] <= b1['z']).OnlyEnforceIf(back)

            model.AddBoolOr([
                b1['is_packed'].Not(), 
                b2['is_packed'].Not(), 
                left, right, below, above, front, back
            ])

    # --- OBJECTIVE ---
    packed_count = sum(all_boxes[idx]['is_packed'] for idx in item_indices)
    
    costs = []
    for idx in item_indices:
        b = all_boxes[idx]
        c_y = model.NewIntVar(0, VAN_H * 10000, f'cy_{idx}')
        c_x = model.NewIntVar(0, VAN_L * 100, f'cx_{idx}')
        
        model.Add(c_y == b['y'] * 10000).OnlyEnforceIf(b['is_packed'])
        model.Add(c_x == b['x'] * 100).OnlyEnforceIf(b['is_packed'])
        model.Add(c_y == 0).OnlyEnforceIf(b['is_packed'].Not())
        model.Add(c_x == 0).OnlyEnforceIf(b['is_packed'].Not())
        
        costs.append(c_y)
        costs.append(c_x)

    model.Maximize(packed_count * 1000000000 - sum(costs))

    # --- SOLVE ---
    solver = cp_model.CpSolver()
    solver.parameters.max_time_in_seconds = 10.0
    solver.parameters.log_search_progress = False 
    
    with suppress_stdout():
        status = solver.Solve(model)

    if status in (cp_model.OPTIMAL, cp_model.FEASIBLE):
        placed_items = []
        unplaced_items = []
        
        # --- CORREÇÃO AQUI: Inicializar variáveis ---
        total_weight = 0.0
        used_volume = 0
        
        for idx in item_indices:
            v = all_boxes[idx]
            raw = items[idx]
            
            if solver.Value(v['is_packed']):
                fl = solver.Value(v['xe']) - solver.Value(v['x'])
                fw = solver.Value(v['ze']) - solver.Value(v['z'])
                fh = solver.Value(v['ye']) - solver.Value(v['y'])
                
                # Calcular estatísticas
                total_weight += float(raw.get('weight_kg', 0) or 0)
                used_volume += (fl * fw * fh)
                
                rx, ry = 0, 0
                if fh != to_int(raw.get('height_mm',0)): rx = 90
                elif fw != to_int(raw.get('width_mm',0)): ry = 90

                placed_items.append({
                    "item": raw,
                    "position": {"x": solver.Value(v['x']), "y": solver.Value(v['y']), "z": solver.Value(v['z'])},
                    "rotation": { "x": rx, "y": ry, "z": 0 },
                    "placed_length": fl, "placed_width": fw, "placed_height": fh, "level": 0
                })
            else:
                unplaced_items.append(raw)
        
        van_vol = VAN_L * VAN_W * VAN_H
        util = (used_volume / van_vol * 100) if van_vol > 0 else 0
            
        return {
            "success": True,
            "plan": {
                "items": placed_items,
                "total_weight": total_weight, # Agora usa o nome correto
                "utilization_percent": util,
                "van_volume": van_vol,
                "used_volume": used_volume
            },
            "unplaced_items": unplaced_items,
            "warnings": [f"Items not packed: {len(unplaced_items)}"] if unplaced_items else []
        }
    else:
        return {"success": False, "plan": None, "warnings": [f"Optimization failed. Status: {solver.StatusName(status)}"]}

if __name__ == "__main__":
    try:
        input_str = sys.stdin.read()
        if input_str: print(json.dumps(solve_packing(json.loads(input_str))))
    except Exception as e:
        print(json.dumps({"success": False, "warnings": [f"Python Error: {str(e)}"]}))