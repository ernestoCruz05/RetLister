import { useEffect, useMemo, useRef, useState } from "react";
import "./App.css";
import { addResto, listRestos, removeResto, searchResto, updateResto, getStats, listVans, addVan, updateVan, deleteVan, optimizeLoading, getServerUrl, setServerUrl } from "./api";
import VanVisualization from "./VanVisualization";

function optimizeCuts(cuttingList, inventory, settings) {
  const { kerfWidth, minRemainderWidth, minRemainderHeight } = settings;
  
  const allCuts = [];
  cuttingList.forEach(cut => {
    for (let i = 0; i < cut.quantity; i++) {
      allCuts.push({ ...cut, originalId: cut.id, cutId: `${cut.id}-${i}` });
    }
  });
  
  const strategies = [
    { sortBy: 'area-desc', label: 'Largest area first' },
    { sortBy: 'width-desc', label: 'Widest first' },
    { sortBy: 'height-desc', label: 'Tallest first' },
    { sortBy: 'perimeter-desc', label: 'Largest perimeter first' }
  ];
  
  let bestResult = null;
  let bestEfficiency = 0;
  
  for (const strategy of strategies) {
    const result = tryPackingStrategy(allCuts, inventory, kerfWidth, minRemainderWidth, minRemainderHeight, strategy.sortBy);
    const efficiency = calculateEfficiency(result);
    
    if (efficiency > bestEfficiency || !bestResult) {
      bestEfficiency = efficiency;
      bestResult = result;
    }
  }
  
  return bestResult;
}

function tryPackingStrategy(allCuts, inventory, kerfWidth, minRemainderWidth, minRemainderHeight, sortBy) {
  const sortedCuts = [...allCuts].sort((a, b) => {
    if (sortBy === 'area-desc') return (b.width_mm * b.height_mm) - (a.width_mm * a.height_mm);
    if (sortBy === 'width-desc') return b.width_mm - a.width_mm;
    if (sortBy === 'height-desc') return b.height_mm - a.height_mm;
    if (sortBy === 'perimeter-desc') return (b.width_mm + b.height_mm) - (a.width_mm + a.height_mm);
    return 0;
  });
  
  const usedPlanks = [];
  const unplacedCuts = [];
  
  const availablePlanks = inventory
    .map(resto => ({ resto, freeRects: [{ x: 0, y: 0, width: resto.width_mm, height: resto.height_mm }] }))
    .sort((a, b) => (a.resto.width_mm * a.resto.height_mm) - (b.resto.width_mm * b.resto.height_mm));
  
  for (const cut of sortedCuts) {
    let placed = false;
    
    for (const plank of availablePlanks) {
      if (plank.resto.material.toLowerCase() !== cut.material.toLowerCase()) continue;
      if (plank.resto.thickness_mm !== cut.thickness_mm) continue;
      
      const orientations = [
        { w: cut.width_mm, h: cut.height_mm, rotated: false },
        { w: cut.height_mm, h: cut.width_mm, rotated: true }
      ];
      
      for (const orient of orientations) {
        let bestRect = null;
        let bestRectIndex = -1;
        let bestWaste = Infinity;
        
        for (let i = 0; i < plank.freeRects.length; i++) {
          const rect = plank.freeRects[i];
          if (rect.width >= orient.w && rect.height >= orient.h) {
            const waste = (rect.width * rect.height) - (orient.w * orient.h);
            if (waste < bestWaste) {
              bestWaste = waste;
              bestRect = rect;
              bestRectIndex = i;
            }
          }
        }
        
        if (bestRect) {
          const placedCut = {
            ...cut,
            x: bestRect.x,
            y: bestRect.y,
            width_mm: orient.w,
            height_mm: orient.h,
            rotated: orient.rotated
          };
          
          if (!plank.cuts) plank.cuts = [];
          plank.cuts.push(placedCut);
          
          plank.freeRects.splice(bestRectIndex, 1);
          
          const rightRect = {
            x: bestRect.x + orient.w + kerfWidth,
            y: bestRect.y,
            width: bestRect.width - orient.w - kerfWidth,
            height: orient.h
          };
          
          const bottomRect = {
            x: bestRect.x,
            y: bestRect.y + orient.h + kerfWidth,
            width: bestRect.width,
            height: bestRect.height - orient.h - kerfWidth
          };
          
          if (rightRect.width >= minRemainderWidth && rightRect.height >= minRemainderHeight) {
            plank.freeRects.push(rightRect);
          }
          if (bottomRect.width >= minRemainderWidth && bottomRect.height >= minRemainderHeight) {
            plank.freeRects.push(bottomRect);
          }
          
          placed = true;
          break;
        }
      }
      
      if (placed) break;
    }
    
    if (!placed) {
      unplacedCuts.push(cut);
    }
  }
  
  const actuallyUsed = availablePlanks.filter(p => p.cuts && p.cuts.length > 0);
  
  return {
    usedPlanks: actuallyUsed.map(p => ({
      resto: p.resto,
      cuts: p.cuts,
      wastePercent: calculatePlankWaste(p, kerfWidth)
    })),
    unplacedCuts,
    totalCuts: sortedCuts.length - unplacedCuts.length,
    efficiency: 0 
  };
}

function calculatePlankWaste(plank, kerfWidth) {
  const totalArea = plank.resto.width_mm * plank.resto.height_mm;
  const usedArea = plank.cuts.reduce((sum, cut) => {
    const cutArea = cut.width_mm * cut.height_mm;
    const kerfArea = (cut.width_mm + cut.height_mm) * kerfWidth;
    return sum + cutArea + kerfArea;
  }, 0);
  return ((totalArea - usedArea) / totalArea) * 100;
}

function calculateEfficiency(result) {
  if (result.usedPlanks.length === 0) return 0;
  const totalArea = result.usedPlanks.reduce((sum, p) => sum + (p.resto.width_mm * p.resto.height_mm), 0);
  const wasteArea = result.usedPlanks.reduce((sum, p) => sum + (p.resto.width_mm * p.resto.height_mm * p.wastePercent / 100), 0);
  result.efficiency = ((totalArea - wasteArea) / totalArea) * 100;
  return result.efficiency;
}

function IconSearch(props) {
  return (
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden {...props}>
      <path fill="currentColor" d="M15.5 14h-.79l-.28-.27A6.471 6.471 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 5 1.5-1.5-5-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"/>
    </svg>
  );
}

function IconPlus(props){
  return (
    <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden {...props}>
      <path fill="currentColor" d="M11 11V5h2v6h6v2h-6v6h-2v-6H5v-2z"/>
    </svg>
  );
}

function IconTrash(props){
  return (
    <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden {...props}>
      <path fill="currentColor" d="M9 3h6l1 2h4v2H4V5h4l1-2zm1 6h2v8h-2V9zm4 0h2v8h-2V9zM7 9h2v8H7V9z"/>
    </svg>
  );
}

function IconGear(props){
  return (
    <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden {...props}>
      <path fill="currentColor" d="M19.43 12.98c.04-.32.07-.64.07-.98 0-.34-.03-.66-.07-.98l2.11-1.65c.19-.15.24-.42.12-.64l-2-3.46c-.12-.22-.39-.3-.61-.22l-2.49 1c-.52-.4-1.08-.73-1.69-.98l-.38-2.65C14.46 2.18 14.25 2 14 2h-4c-.25 0-.46.18-.49.42l-.38 2.65c-.61.25-1.17.59-1.69.98l-2.49-1c-.23-.09-.49 0-.61.22l-2 3.46c-.13.22-.07.49.12.64l2.11 1.65c-.04.32-.07.65-.07.98 0 .33.03.66.07.98l-2.11 1.65c-.19.15-.24.42-.12.64l2 3.46c.12.22.39.3.61.22l2.49-1c.52.4 1.08.73 1.69.98l.38 2.65c.03.24.24.42.49.42h4c.25 0 .46-.18.49-.42l.38-2.65c.61-.25 1.17-.59 1.69-.98l2.49 1c.23.09.49 0 .61-.22l2-3.46c.12-.22.07-.49-.12-.64l-2.11-1.65zM12 15.5c-1.93 0-3.5-1.57-3.5-3.5s1.57-3.5 3.5-3.5 3.5 1.57 3.5 3.5-1.57 3.5-3.5 3.5z"/>
    </svg>
  );
}

function App() {
  const clickTimerRef = useRef(null);
  const [inventory, setInventory] = useState([]);
  const [filteredInventory, setFilteredInventory] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [selectedIds, setSelectedIds] = useState([]);
  const [searchActive, setSearchActive] = useState(false);
  const [activeTab, setActiveTab] = useState("restos"); // "restos", "stats", "optimizer", or "estado"
  const [serverUrlInput, setServerUrlInput] = useState(getServerUrl());
  const [stats, setStats] = useState(null);
  const [filterMaterial, setFilterMaterial] = useState("");
  const [filterThickness, setFilterThickness] = useState("");

  const [vans, setVans] = useState([]);
  const [selectedVanId, setSelectedVanId] = useState(null);
  const [cargoItems, setCargoItems] = useState([]);
  const [loadingPlan, setLoadingPlan] = useState(null);
  const [optimizeWarnings, setOptimizeWarnings] = useState([]);
  const [isOptimizing, setIsOptimizing] = useState(false);
  const [vansLoading, setVansLoading] = useState(true);

  const [estadoData, setEstadoData] = useState({
    mainServer: { status: 'unknown', uptime: 0 },
    proxyServer: { status: 'unknown', uptime: 0, db: '', pending: 0 },
    lastCheck: null
  });

  const [cuttingList, setCuttingList] = useState([]);
  const [optimizationResult, setOptimizationResult] = useState(null);
  const [newCut, setNewCut] = useState({ width_mm: "", height_mm: "", thickness_mm: "18", material: "MDF", quantity: "1", label: "" });

  const [settings, setSettings] = useState({ kerfWidth: 3, minRemainderWidth: 300, minRemainderHeight: 300 });
  const [settingsOpen, setSettingsOpen] = useState(false);

  const [addOpen, setAddOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [editOpen, setEditOpen] = useState(false);

  const [vanModalOpen, setVanModalOpen] = useState(false);
  const [editingVan, setEditingVan] = useState(null);
  const [cargoModalOpen, setCargoModalOpen] = useState(false);
  const [editingCargo, setEditingCargo] = useState(null);

  const [addForm, setAddForm] = useState({ width_mm: "", height_mm: "", thickness_mm: "18", material: "MDF", notes: "" });
  const [searchForm, setSearchForm] = useState({ width_mm: "", height_mm: "", thickness_mm: "18", material: "MDF" });
  const [editForm, setEditForm] = useState({ width_mm: "", height_mm: "", thickness_mm: "", material: "", notes: "" });

  const tableRef = useRef(null);

  useEffect(() => {
    const handleKeyDown = (e) => {
      if (e.key === 'Escape') {
        if (addOpen) setAddOpen(false);
        if (searchOpen) setSearchOpen(false);
        if (editOpen) setEditOpen(false);
        if (settingsOpen) setSettingsOpen(false);
        return;
      }

      if (activeTab !== 'restos') return;

      if (e.key === 'Delete' && selectedIds.length > 0) {
        e.preventDefault();
        handleRemoveSelected();
        return;
      }

      if (e.key === 'Enter' && selectedIds.length > 0 && !addOpen && !searchOpen && !editOpen) {
        e.preventDefault();
        handleEditClick();
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [activeTab, selectedIds, addOpen, searchOpen, editOpen, settingsOpen]);

  useEffect(() => {
    const headers = tableRef.current?.querySelectorAll('th');
    if (!headers) return;

    headers.forEach((th) => {
      const resizer = document.createElement('div');
      resizer.className = 'col-resizer';
      th.appendChild(resizer);

      let startX, startWidth;
      
      const onMouseDown = (e) => {
        startX = e.clientX;
        startWidth = th.offsetWidth;
        document.addEventListener('mousemove', onMouseMove);
        document.addEventListener('mouseup', onMouseUp);
        e.preventDefault();
      };

      const onMouseMove = (e) => {
        const width = startWidth + (e.clientX - startX);
        th.style.width = `${Math.max(30, width)}px`;
      };

      const onMouseUp = () => {
        document.removeEventListener('mousemove', onMouseMove);
        document.removeEventListener('mouseup', onMouseUp);
      };

      resizer.addEventListener('mousedown', onMouseDown);
    });
  }, [filteredInventory]);

  const validAdd = useMemo(() => {
    const w = Number(addForm.width_mm), h = Number(addForm.height_mm), t = Number(addForm.thickness_mm);
    return w > 0 && h > 0 && t > 0 && addForm.material.trim().length > 0;
  }, [addForm]);

  const validSearch = useMemo(() => {
    const w = Number(searchForm.width_mm), h = Number(searchForm.height_mm), t = Number(searchForm.thickness_mm);
    return w > 0 && h > 0 && t > 0 && searchForm.material.trim().length > 0;
  }, [searchForm]);

  const uniqueMaterials = useMemo(() => {
    const materials = [...new Set(inventory.map(r => r.material))];
    return materials.sort();
  }, [inventory]);

  const uniqueThicknesses = useMemo(() => {
    const thicknesses = [...new Set(inventory.map(r => r.thickness_mm))];
    return thicknesses.sort((a, b) => a - b);
  }, [inventory]);

  async function refreshStats() {
    try {
      const data = await getStats();
      setStats(data);
    } catch (e) {
      setError(String(e));
    }
  }

  async function fetchEstado() {
    const newEstado = {
      mainServer: { status: 'error', uptime: 0 },
      proxyServer: { status: 'error', uptime: 0, db: '', pending: 0 },
      lastCheck: new Date().toLocaleTimeString('pt-PT')
    };

    try {
      const response = await fetch('http://localhost:8000/health');
      if (response.ok) {
        const data = await response.json();
        newEstado.mainServer.status = data.status === 'ok' ? 'ok' : 'error';
      }
    } catch (e) {
      console.error('Main server health check failed:', e);
    }

    try {
      const healthRes = await fetch('http://localhost:8001/health');
      if (healthRes.ok) {
        const health = await healthRes.json();
        newEstado.proxyServer.status = health.proxy_active ? 'ok' : 'error';
        newEstado.proxyServer.uptime = health.uptime_seconds || 0;
        newEstado.proxyServer.db = health.db_path || '';
      }

      const syncRes = await fetch('http://localhost:8001/sync/status');
      if (syncRes.ok) {
        const sync = await syncRes.json();
        newEstado.proxyServer.pending = sync.pending_changes || 0;
      }
    } catch (e) {
      console.error('Proxy server health check failed:', e);
    }

    setEstadoData(newEstado);
  }

  async function refresh() {
    try {
      setLoading(true);
      setError("");
      const data = await listRestos();
      setInventory(data);
      setFilteredInventory(data);
      setSearchActive(false);
      if (selectedIds.length > 0) {
        const validIds = data.map(r => r.id);
        setSelectedIds(prev => prev.filter(id => validIds.includes(id)));
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function loadVans() {
    try {
      setVansLoading(true);
      const data = await listVans();
      setVans(data);
    } catch (err) {
      console.error('Failed to load vans:', err);
      alert('Erro ao carregar carrinhas');
    } finally {
      setVansLoading(false);
    }
  }

  function handleAddVan() {
    setEditingVan(null);
    setVanModalOpen(true);
  }

  function handleEditVan(van) {
    setEditingVan(van);
    setVanModalOpen(true);
  }

  async function handleSaveVan(vanData) {
    try {
      if (editingVan) {
        await updateVan(editingVan.id, vanData);
      } else {
        await addVan(vanData);
      }
      setVanModalOpen(false);
      await loadVans(); 
    } catch (err) {
      console.error('Failed to save van:', err);
      alert(`Erro ao guardar carrinha: ${err.message}`);
    }
  }

  async function handleDeleteVan(vanId) {
    if (confirm('Desativar esta carrinha?')) {
      try {
        await deleteVan(vanId);
        if (selectedVanId === vanId) {
          setSelectedVanId(null);
        }
        await loadVans(); 
      } catch (err) {
        console.error('Failed to delete van:', err);
        alert('Erro ao desativar carrinha');
      }
    }
  }

  function handleAddCargo() {
    if (!selectedVanId) return;
    setEditingCargo(null);
    setCargoModalOpen(true);
  }

  function handleEditCargo(cargo, index) {
    setEditingCargo({ ...cargo, index });
    setCargoModalOpen(true);
  }

  function handleSaveCargo(cargoData) {
    if (editingCargo && editingCargo.index !== undefined) {
      setCargoItems(prev => prev.map((item, idx) => idx === editingCargo.index ? cargoData : item));
    } else {
      setCargoItems(prev => [...prev, cargoData]);
    }
    setCargoModalOpen(false);
  }

  function handleDeleteCargo(index) {
    if (confirm('Remover este item?')) {
      setCargoItems(prev => prev.filter((_, idx) => idx !== index));
    }
  }

  async function handleGeneratePlan() {
    if (!selectedVanId || cargoItems.length === 0) return;
    
    try {
      setIsOptimizing(true);
      setOptimizeWarnings([]);
      const result = await optimizeLoading(selectedVanId, cargoItems);
      
      if (result.success && result.plan) {
        setLoadingPlan(result.plan);
        setOptimizeWarnings(result.warnings || []);
      } else {
        alert('Falha na otimização');
      }
    } catch (err) {
      console.error('Optimization failed:', err);
      alert('Erro ao gerar plano de carregamento');
    } finally {
      setIsOptimizing(false);
    }
  }

  useEffect(() => { refresh(); }, []);

  useEffect(() => {
    if (activeTab === 'carrinhas') {
      loadVans();
    }
  }, [activeTab]);

  useEffect(() => {
    if (activeTab === "stats") {
      refreshStats();
    } else if (activeTab === "estado") {
      fetchEstado();
    }
  }, [activeTab]);

  useEffect(() => {
    let filtered = searchActive ? filteredInventory : inventory;
    
    if (filterMaterial) {
      filtered = filtered.filter(r => r.material.toLowerCase() === filterMaterial.toLowerCase());
    }
    
    if (filterThickness) {
      filtered = filtered.filter(r => r.thickness_mm === Number(filterThickness));
    }
    
    if (!searchActive && (filterMaterial || filterThickness)) {
      setFilteredInventory(filtered);
    }
  }, [filterMaterial, filterThickness, inventory, searchActive]);

  async function handleAdd(e) {
    e.preventDefault();
    if (!validAdd) return;
    try {
      setLoading(true);
      setError("");
      await addResto({
        width_mm: Number(addForm.width_mm),
        height_mm: Number(addForm.height_mm),
        thickness_mm: Number(addForm.thickness_mm),
        material: addForm.material.trim(),
        notes: addForm.notes.trim() || null,
      });
      setAddOpen(false);
      setAddForm({ width_mm: "", height_mm: "", thickness_mm: addForm.thickness_mm, material: addForm.material, notes: "" });
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleSearch(e) {
    e.preventDefault();
    if (!validSearch) return;
    try {
      setLoading(true);
      setError("");
      
      const requiredArea = Number(searchForm.width_mm) * Number(searchForm.height_mm);
      
      const filtered = inventory.filter((r) => 
        r.width_mm >= Number(searchForm.width_mm) &&
        r.height_mm >= Number(searchForm.height_mm) &&
        r.thickness_mm === Number(searchForm.thickness_mm) &&
        r.material === searchForm.material.trim()
      );
      
      const sorted = filtered.sort((a, b) => {
        const areaA = a.width_mm * a.height_mm;
        const areaB = b.width_mm * b.height_mm;
        const wasteA = areaA - requiredArea;
        const wasteB = areaB - requiredArea;
        return wasteA - wasteB;
      });
      
      setFilteredInventory(sorted);
      setSearchActive(true);
      setSearchOpen(false);
      
      if (sorted.length > 0) {
        setSelectedId(sorted[0].id);
        setTimeout(() => {
          const el = document.getElementById(`row-${sorted[0].id}`);
          if (el && tableRef.current) {
            el.scrollIntoView({ block: "center" });
          }
        }, 0);
      } else {
        setSelectedId(null);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleRemoveSelected() {
    if (selectedIds.length === 0) return;
    const count = selectedIds.length;
    if (!confirm(`Remover ${count} resto${count > 1 ? 's' : ''}?`)) return;
    try {
      setLoading(true);
      setError("");
      for (const id of selectedIds) {
        await removeResto(id);
      }
      setSelectedIds([]);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  function handleEditClick() {
    if (selectedIds.length === 0) return;
    const selectedId = selectedIds[0];
    const resto = inventory.find(r => r.id === selectedId);
    if (!resto) return;
    setEditForm({
      width_mm: String(resto.width_mm),
      height_mm: String(resto.height_mm),
      thickness_mm: String(resto.thickness_mm),
      material: resto.material,
      notes: resto.notes || ""
    });
    setEditOpen(true);
  }

  async function handleEdit(e) {
    e.preventDefault();
    if (selectedIds.length === 0) return;
    const selectedId = selectedIds[0];
    try {
      setLoading(true);
      setError("");
      const payload = {};
      if (editForm.width_mm) payload.width_mm = Number(editForm.width_mm);
      if (editForm.height_mm) payload.height_mm = Number(editForm.height_mm);
      if (editForm.thickness_mm) payload.thickness_mm = Number(editForm.thickness_mm);
      if (editForm.material) payload.material = editForm.material.trim();
      if (editForm.notes !== undefined) payload.notes = editForm.notes || null;
      
      await updateResto(selectedId, payload);
      setEditOpen(false);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="app">
      <header className="appbar">
        <div className="toolbar">
          <button className="tb-btn" onClick={() => setAddOpen(true)}>
            <IconPlus /><span>Adicionar</span>
          </button>
          <select 
            className="filter-select" 
            value={filterMaterial} 
            onChange={(e) => setFilterMaterial(e.target.value)}
            title="Filtrar por material"
          >
            <option value="">Todos os materiais</option>
            {uniqueMaterials.map(m => <option key={m} value={m}>{m}</option>)}
          </select>
          <select 
            className="filter-select" 
            value={filterThickness} 
            onChange={(e) => setFilterThickness(e.target.value)}
            title="Filtrar por espessura"
          >
            <option value="">Todas espessuras</option>
            {uniqueThicknesses.map(t => <option key={t} value={t}>{t}mm</option>)}
          </select>
          <button className="tb-btn" onClick={() => setSearchOpen(true)} aria-label="Procurar">
            <IconSearch /><span>Procurar</span>
          </button>
          <button className="tb-btn" disabled={selectedIds.length === 0} onClick={handleEditClick}>
            <span>✎ Editar</span>
          </button>
          <button className="tb-btn danger" disabled={selectedIds.length === 0} onClick={handleRemoveSelected}>
            <IconTrash /><span>{selectedIds.length > 1 ? `Remover (${selectedIds.length})` : 'Remover'}</span>
          </button>
        </div>
        <div className="spacer" style={{flex:1}} />
        <button className="tb-btn" onClick={() => setSettingsOpen(true)}>
          <IconGear /><span>Definicoes</span>
        </button>
        <button className="tb-btn" onClick={refresh}>{loading ? "A atualizar..." : "↻ Atualizar"}</button>
      </header>
      <nav className="tabs folderbar">
        <button className={`tab folder ${activeTab === "restos" ? "active" : ""}`} onClick={() => setActiveTab("restos")}>Retalhos</button>
        <button className={`tab folder ${activeTab === "stats" ? "active" : ""}`} onClick={() => setActiveTab("stats")}>Estatísticas</button>
        <button className={`tab folder ${activeTab === "optimizer" ? "active" : ""}`} onClick={() => setActiveTab("optimizer")}>Otimizador</button>
        <button className={`tab folder ${activeTab === "carrinhas" ? "active" : ""}`} onClick={() => setActiveTab("carrinhas")}>Carrinhas</button>
        <button className={`tab folder ${activeTab === "estado" ? "active" : ""}`} onClick={() => setActiveTab("estado")}>Estado</button>
      </nav>

      {error && <div className="banner error">{error}</div>}

      {activeTab === "restos" && (
      <section className="content">
        <div className="table-pane" ref={tableRef}>
          <table className="table">
            <thead>
              <tr>
                <th>ID</th>
                <th>Largura</th>
                <th>Altura</th>
                <th>Espessura</th>
                <th>Material</th>
                <th>Notas</th>
                <th>Criado</th>
              </tr>
            </thead>
            <tbody>
              {filteredInventory.length === 0 ? (
                <tr><td colSpan={7} className="muted">{searchActive ? "Sem resultados para a pesquisa" : "Sem retalhos"}</td></tr>
              ) : (
                filteredInventory.map((r) => (
                  <tr
                    id={`row-${r.id}`}
                    key={r.id}
                    className={selectedIds.includes(r.id) ? "selected" : ""}
                    onClick={(e) => {
                      const isCtrl = e.ctrlKey || e.metaKey;
                      if (clickTimerRef.current) {
                        clearTimeout(clickTimerRef.current);
                        clickTimerRef.current = null;
                      } else {
                        clickTimerRef.current = setTimeout(() => {
                          if (isCtrl) {
                            setSelectedIds(prev => 
                              prev.includes(r.id) ? prev.filter(i => i !== r.id) : [...prev, r.id]
                            );
                          } else {
                            setSelectedIds([r.id]);
                          }
                          clickTimerRef.current = null;
                        }, 200);
                      }
                    }}
                    onDoubleClick={(e) => {
                      if (clickTimerRef.current) {
                        clearTimeout(clickTimerRef.current);
                        clickTimerRef.current = null;
                      }
                      setSelectedIds([r.id]);
                      const resto = inventory.find(item => item.id === r.id);
                      if (resto) {
                        setEditForm({
                          width_mm: String(resto.width_mm),
                          height_mm: String(resto.height_mm),
                          thickness_mm: String(resto.thickness_mm),
                          material: resto.material,
                          notes: resto.notes || ""
                        });
                        setEditOpen(true);
                      }
                    }}
                  >
                    <td>#{r.id}</td>
                    <td>{r.width_mm}</td>
                    <td>{r.height_mm}</td>
                    <td>{r.thickness_mm}</td>
                    <td>{r.material}</td>
                    <td>
                      {r.notes ? (
                        <div className="note-tooltip">
                          {r.notes.length > 30 ? r.notes.substring(0, 30) + '...' : r.notes}
                          {r.notes.length > 0 && <span className="tooltip-text">{r.notes}</span>}
                        </div>
                      ) : '-'}
                    </td>
                    <td className="muted small">{r.created_at}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </section>
      )}

      {activeTab === "stats" && (
      <section className="content">
        <div className="stats-pane">
          {!stats ? (
            <p className="muted">A carregar estatísticas...</p>
          ) : (
            <div className="stats-grid">
              <div className="stat-card">
                <h3>Resumo Geral</h3>
                <div className="stat-row"><span>Total de Retalhos:</span> <strong>{stats.total_count}</strong></div>
                <div className="stat-row"><span>Área Total:</span> <strong>{(stats.total_area_mm2 / 1_000_000).toFixed(2)} m²</strong></div>
              </div>

              <div className="stat-card">
                <h3>Por Material</h3>
                {stats.by_material.length === 0 ? (
                  <p className="muted small">Sem dados</p>
                ) : (
                  <table className="stat-table">
                    <thead>
                      <tr><th>Material</th><th>Quantidade</th><th>Área (m²)</th></tr>
                    </thead>
                    <tbody>
                      {stats.by_material.map((m) => (
                        <tr key={m.material}>
                          <td>{m.material}</td>
                          <td>{m.count}</td>
                          <td>{(m.total_area_mm2 / 1_000_000).toFixed(2)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>

              <div className="stat-card">
                <h3>Por Espessura</h3>
                {stats.by_thickness.length === 0 ? (
                  <p className="muted small">Sem dados</p>
                ) : (
                  <table className="stat-table">
                    <thead>
                      <tr><th>Espessura (mm)</th><th>Quantidade</th></tr>
                    </thead>
                    <tbody>
                      {stats.by_thickness.map((t) => (
                        <tr key={t.thickness_mm}>
                          <td>{t.thickness_mm}</td>
                          <td>{t.count}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
            </div>
          )}
        </div>
      </section>
      )}

      {activeTab === "optimizer" && (
      <section className="content">
        <div className="optimizer-layout">
          <div className="optimizer-sidebar">
            <h3>Lista de Corte</h3>
            <form className="cut-form" onSubmit={(e) => {
              e.preventDefault();
              const w = Number(newCut.width_mm);
              const h = Number(newCut.height_mm);
              const t = Number(newCut.thickness_mm);
              const q = Number(newCut.quantity);
              if (w > 0 && h > 0 && t > 0 && q > 0 && newCut.material.trim()) {
                setCuttingList(prev => [...prev, { ...newCut, id: Date.now(), width_mm: w, height_mm: h, thickness_mm: t, quantity: q }]);
                setNewCut({ width_mm: "", height_mm: "", thickness_mm: "18", material: "MDF", quantity: "1", label: "" });
              }
            }}>
              <input type="number" placeholder="Largura (mm)" value={newCut.width_mm} onChange={(e)=>setNewCut(s=>({...s,width_mm:e.target.value}))} required />
              <input type="number" placeholder="Altura (mm)" value={newCut.height_mm} onChange={(e)=>setNewCut(s=>({...s,height_mm:e.target.value}))} required />
              <input type="number" placeholder="Espessura (mm)" value={newCut.thickness_mm} onChange={(e)=>setNewCut(s=>({...s,thickness_mm:e.target.value}))} required />
              <input type="text" placeholder="Material" value={newCut.material} onChange={(e)=>setNewCut(s=>({...s,material:e.target.value}))} required />
              <input type="number" placeholder="Qtd" value={newCut.quantity} onChange={(e)=>setNewCut(s=>({...s,quantity:e.target.value}))} min="1" required />
              <input type="text" placeholder="Etiqueta (opcional)" value={newCut.label} onChange={(e)=>setNewCut(s=>({...s,label:e.target.value}))} />
              <button type="submit" className="btn primary">+ Adicionar</button>
            </form>
            <div className="cut-list">
              {cuttingList.length === 0 ? (
                <p className="muted small">Nenhuma peça adicionada</p>
              ) : (
                cuttingList.map(cut => (
                  <div key={cut.id} className="cut-item">
                    <div className="cut-info">
                      <strong>{cut.width_mm}x{cut.height_mm}x{cut.thickness_mm}mm</strong>
                      <span>{cut.material} (x{cut.quantity})</span>
                      {cut.label && <span className="muted small">{cut.label}</span>}
                    </div>
                    <button className="btn-icon" onClick={() => setCuttingList(prev => prev.filter(c => c.id !== cut.id))}>✕</button>
                  </div>
                ))
              )}
            </div>
            <button 
              className="btn primary" 
              disabled={cuttingList.length === 0}
              onClick={() => {
                const result = optimizeCuts(cuttingList, inventory, settings);
                setOptimizationResult(result);
              }}
            >
              Otimizar Cortes
            </button>
          </div>
          <div className="optimizer-main">
            <h3>Plano de Corte</h3>
            {!optimizationResult ? (
              <div className="empty-state">
                <p className="muted">Adicione peças à lista e clique em "Otimizar Cortes" para ver o plano</p>
              </div>
            ) : (
              <div className="optimization-results">
                <div className="results-summary">
                  <div className="summary-card">
                    <span>Placas Usadas:</span>
                    <strong>{optimizationResult.usedPlanks.length}</strong>
                  </div>
                  <div className="summary-card">
                    <span>Taxa de Aproveitamento:</span>
                    <strong>{optimizationResult.efficiency.toFixed(1)}%</strong>
                  </div>
                  <div className="summary-card">
                    <span>Peças Cortadas:</span>
                    <strong>{optimizationResult.totalCuts} / {cuttingList.reduce((sum, c) => sum + c.quantity, 0)}</strong>
                  </div>
                </div>
                <div className="optimization-actions">
                  <button 
                    className="btn primary"
                    onClick={async () => {
                      try {
                        setLoading(true);
                        for (const plank of optimizationResult.usedPlanks) {
                          await removeResto(plank.resto.id);
                        }
                        setCuttingList([]);
                        setOptimizationResult(null);
                        await refresh();
                        setError("");
                        alert(`${optimizationResult.usedPlanks.length} placas removidas do inventário`);
                      } catch (e) {
                        setError(String(e));
                      } finally {
                        setLoading(false);
                      }
                    }}
                  >
                    Confirmar Corte
                  </button>
                  <button 
                    className="btn"
                    onClick={() => {
                      setOptimizationResult(null);
                      setCuttingList([]);
                    }}
                  >
                    Descartar
                  </button>
                </div>
                <div className="planks-visualization">
                  {optimizationResult.usedPlanks.map((plank, idx) => (
                    <div key={idx} className="plank-card">
                      <div className="plank-header">
                        <strong>Placa #{plank.resto.id} - {plank.resto.material} {plank.resto.thickness_mm}mm</strong>
                        <span>{plank.resto.width_mm}x{plank.resto.height_mm}mm</span>
                      </div>
                      <svg className="plank-svg" viewBox={`0 0 ${plank.resto.width_mm} ${plank.resto.height_mm}`} preserveAspectRatio="xMidYMid meet">
                        <rect width={plank.resto.width_mm} height={plank.resto.height_mm} fill="#f0f0f0" stroke="#333" strokeWidth="2"/>
                        {plank.cuts.map((cut, cidx) => (
                          <g key={cidx}>
                            <rect x={cut.x} y={cut.y} width={cut.width_mm} height={cut.height_mm} fill="#4a9eff" stroke="#000" strokeWidth="1" opacity="0.7"/>
                            <text x={cut.x + cut.width_mm/2} y={cut.y + cut.height_mm/2} textAnchor="middle" fontSize="12" fill="#000">
                              {cut.width_mm}x{cut.height_mm}
                            </text>
                          </g>
                        ))}
                      </svg>
                      <div className="plank-waste">Desperdício: {plank.wastePercent.toFixed(1)}%</div>
                    </div>
                  ))}
                  {optimizationResult.unplacedCuts.length > 0 && (
                    <div className="alert warning">
                      <strong>Peças não encaixadas:</strong>
                      <ul>
                        {optimizationResult.unplacedCuts.map((cut, idx) => (
                          <li key={idx}>{cut.width_mm}x{cut.height_mm}x{cut.thickness_mm}mm {cut.material} (x{cut.quantity})</li>
                        ))}
                      </ul>
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
        </div>
      </section>
      )}

      {activeTab === "carrinhas" && (
      <section className="content">
        <div className="van-loader-layout" style={{display: 'flex', gap: '20px', height: '100%'}}>
          {/* Left Panel - Van Selection & Cargo List */}
          <aside className="van-sidebar" style={{flex: '0 0 350px', display: 'flex', flexDirection: 'column', gap: '20px'}}>
            {/* Van Management */}
            <div className="panel">
              <div className="panel-header" style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px'}}>
                <h3>Carrinhas Disponíveis</h3>
                <button className="btn btn-primary" onClick={handleAddVan}>+ Nova Carrinha</button>
              </div>
              {vansLoading ? (
                <div className="empty-state" style={{padding: '20px', textAlign: 'center', color: '#666'}}>
                  <p>A carregar carrinhas...</p>
                </div>
              ) : vans.filter(v => v.active).length === 0 ? (
                <div className="empty-state" style={{padding: '20px', textAlign: 'center', color: '#666'}}>
                  <p>Nenhuma carrinha disponível.</p>
                  <p className="small">Adicione a primeira carrinha para começar.</p>
                </div>
              ) : (
                <div className="van-list">
                  {vans.filter(v => v.active).map(van => (
                    <div 
                      key={van.id} 
                      className={`van-item ${selectedVanId === van.id ? 'selected' : ''}`}
                      style={{padding: '10px', border: '1px solid #ccc', marginBottom: '5px', background: selectedVanId === van.id ? '#e3f2fd' : 'white'}}
                    >
                      <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'start'}}>
                        <div style={{flex: 1, cursor: 'pointer'}} onClick={() => setSelectedVanId(van.id)}>
                          <strong>{van.name}</strong>
                          <div className="small muted">
                            {van.length_mm}×{van.width_mm}×{van.height_mm} mm
                            {van.max_weight_kg && ` | ${van.max_weight_kg}kg`}
                          </div>
                        </div>
                        <div style={{display: 'flex', gap: '5px'}}>
                          <button className="btn-icon" onClick={(e) => { e.stopPropagation(); handleEditVan(van); }} title="Editar">✎</button>
                          <button className="btn-icon" onClick={(e) => { e.stopPropagation(); handleDeleteVan(van.id); }} title="Desativar">🗑</button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Cargo Items */}
            <div className="panel" style={{flex: 1, display: 'flex', flexDirection: 'column'}}>
              <div className="panel-header" style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px'}}>
                <h3>Itens a Carregar</h3>
                <button className="btn btn-primary" disabled={!selectedVanId} onClick={handleAddCargo}>+ Adicionar Item</button>
              </div>
              {cargoItems.length === 0 ? (
                <div className="empty-state" style={{padding: '20px', textAlign: 'center', color: '#666'}}>
                  <p>Nenhum item adicionado.</p>
                  {selectedVanId && <p className="small">Adicione móveis para planejar a carga.</p>}
                  {!selectedVanId && <p className="small">Selecione uma carrinha primeiro.</p>}
                </div>
              ) : (
                <div className="cargo-list" style={{flex: 1, overflowY: 'auto'}}>
                  {cargoItems.map((item, idx) => (
                    <div key={idx} className="cargo-item" style={{padding: '8px', border: '1px solid #ddd', marginBottom: '5px', background: 'white'}}>
                      <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'start'}}>
                        <div style={{flex: 1}}>
                          <strong>{item.description || `Item ${idx + 1}`}</strong>
                          <div className="small muted">
                            {item.length_mm}×{item.width_mm}×{item.height_mm} mm
                            {item.weight_kg && ` | ${item.weight_kg}kg`}
                            {item.fragile && ' | 🔴 Frágil'}
                          </div>
                        </div>
                        <div style={{display: 'flex', gap: '5px'}}>
                          <button className="btn-icon" onClick={() => handleEditCargo(item, idx)} title="Editar">✎</button>
                          <button className="btn-icon" onClick={() => handleDeleteCargo(idx)} title="Remover">×</button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              {cargoItems.length > 0 && selectedVanId && (
                <button 
                  className="btn btn-success" 
                  style={{marginTop: '10px'}} 
                  onClick={handleGeneratePlan}
                  disabled={isOptimizing}
                >
                  {isOptimizing ? 'A processar...' : 'Gerar Plano de Carregamento'}
                </button>
              )}
              
              {/* Warnings display */}
              {optimizeWarnings.length > 0 && (
                <div style={{marginTop: '10px', padding: '10px', background: '#fff3cd', border: '1px solid #ffc107', borderRadius: '4px'}}>
                  <strong style={{color: '#856404'}}>⚠ Avisos:</strong>
                  <ul style={{margin: '5px 0 0 0', paddingLeft: '20px', color: '#856404', fontSize: '12px'}}>
                    {optimizeWarnings.map((warn, idx) => (
                      <li key={idx}>{warn}</li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          </aside>

          {/* Main Panel - 3D Visualization */}
          <main className="van-visualization" style={{flex: 1, display: 'flex', flexDirection: 'column', background: '#f5f5f5', border: '1px solid #ccc', borderRadius: '4px'}}>
            {!selectedVanId ? (
              <div style={{display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: '#999'}}>
                <div style={{textAlign: 'center'}}>
                  <h2>Selecione uma carrinha</h2>
                  <p>Escolha uma carrinha à esquerda para visualizar o plano de carregamento</p>
                </div>
              </div>
            ) : !loadingPlan ? (
              <div style={{display: 'flex', flexDirection: 'column', height: '100%'}}>
                <div style={{padding: '20px', borderBottom: '1px solid #ddd'}}>
                  <h3>Visualização 3D</h3>
                  <p className="small muted">Adicione itens e clique em "Gerar Plano" para ver a otimização</p>
                </div>
                <div style={{flex: 1}}>
                  <VanVisualization van={vans.find(v => v.id === selectedVanId)} loadingPlan={null} />
                </div>
              </div>
            ) : (
              <div style={{display: 'flex', flexDirection: 'column', height: '100%'}}>
                <div style={{padding: '20px', borderBottom: '1px solid #ddd'}}>
                  <div className="panel-header" style={{marginBottom: '15px'}}>
                    <h3>Plano de Carregamento</h3>
                  </div>
                  
                  {/* Stats */}
                  <div style={{display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '10px'}}>
                    <div className="stat-box" style={{padding: '10px', background: 'white', border: '1px solid #ddd', borderRadius: '4px'}}>
                      <div className="small muted">Items</div>
                      <div style={{fontSize: '20px', fontWeight: 'bold'}}>{loadingPlan.items.length}</div>
                    </div>
                    <div className="stat-box" style={{padding: '10px', background: 'white', border: '1px solid #ddd', borderRadius: '4px'}}>
                      <div className="small muted">Peso Total</div>
                      <div style={{fontSize: '20px', fontWeight: 'bold'}}>{loadingPlan.total_weight.toFixed(1)} kg</div>
                    </div>
                    <div className="stat-box" style={{padding: '10px', background: 'white', border: '1px solid #ddd', borderRadius: '4px'}}>
                      <div className="small muted">Utilização</div>
                      <div style={{fontSize: '20px', fontWeight: 'bold', color: loadingPlan.utilization_percent > 80 ? '#28a745' : loadingPlan.utilization_percent > 50 ? '#ffc107' : '#dc3545'}}>
                        {loadingPlan.utilization_percent.toFixed(1)}%
                      </div>
                    </div>
                  </div>
                </div>

                {/* 3D Visualization */}
                <div style={{flex: 1, minHeight: 0}}>
                  <VanVisualization van={vans.find(v => v.id === selectedVanId)} loadingPlan={loadingPlan} />
                </div>
              </div>
            )}
          </main>
        </div>
      </section>
      )}

      {activeTab === "estado" && (
      <section className="content">\
        <div className="stats-pane">
          <div className="estado-header" style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px'}}>
            <h2>Estado dos Serviços</h2>
            <button className="btn" onClick={fetchEstado}>↻ Atualizar</button>
          </div>

          {/* Configuration Section */}
          <div className="stat-card" style={{marginBottom: '20px'}}>
            <h3>Configuração de Conexão</h3>
            <div style={{marginTop: '15px'}}>
              <label style={{display: 'block', marginBottom: '8px'}}>
                <strong>URL do Servidor Principal (IP:Porta)</strong>
              </label>
              <div style={{display: 'flex', gap: '10px', alignItems: 'stretch'}}>
                <input 
                  type="text" 
                  value={serverUrlInput} 
                  onChange={(e) => setServerUrlInput(e.target.value)} 
                  placeholder="http://192.168.1.10:8000"
                  style={{flex: 1, padding: '8px', fontSize: '14px', border: '1px solid #ccc', borderRadius: '4px'}}
                />
                <button 
                  className="btn primary" 
                  onClick={() => {
                    setServerUrl(serverUrlInput);
                    alert("URL atualizado! A página será recarregada.");
                    window.location.reload();
                  }}
                  style={{padding: '8px 20px', whiteSpace: 'nowrap'}}
                >
                  Salvar
                </button>
              </div>
              <div style={{marginTop: '8px', fontSize: '12px', color: '#666'}}>
                Exemplo: http://192.168.1.10:8000 (use o IP do servidor)
              </div>
            </div>
            <div style={{marginTop: '15px', padding: '10px', background: '#f5f5f5', borderRadius: '4px', borderLeft: '3px solid #2196F3'}}>
              <div style={{fontSize: '13px', color: '#333'}}>
                <strong>URL atual:</strong> <code style={{background: '#e0e0e0', padding: '2px 6px', borderRadius: '3px'}}>{getServerUrl()}</code>
              </div>
            </div>
          </div>

          {estadoData.lastCheck && (
            <p className="muted small" style={{marginBottom: '20px'}}>Última verificação: {estadoData.lastCheck}</p>
          )}
          <div className="stats-grid">
            <div className="stat-card">
              <h3>Servidor Principal (RustServer)</h3>
              <div className="estado-row" style={{display: 'flex', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid #ccc'}}>
                <span>Estado:</span>
                <div className="estado-indicator" style={{display: 'flex', alignItems: 'center', gap: '8px'}}>
                  <span className={`led ${estadoData.mainServer.status === 'ok' ? 'led-green' : estadoData.mainServer.status === 'unknown' ? 'led-gray' : 'led-red'}`} style={{display: 'inline-block', width: '12px', height: '12px', borderRadius: '50%', backgroundColor: estadoData.mainServer.status === 'ok' ? '#4CAF50' : estadoData.mainServer.status === 'unknown' ? '#999' : '#f44336'}}></span>
                  <strong>{estadoData.mainServer.status === 'ok' ? 'Online' : estadoData.mainServer.status === 'unknown' ? 'Desconhecido' : 'Offline'}</strong>
                </div>
              </div>
              <div className="estado-row" style={{display: 'flex', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid #ccc'}}>
                <span>Porta:</span>
                <strong>8000</strong>
              </div>
            </div>

            <div className="stat-card">
              <h3>Proxy Service (Windows 11)</h3>
              <div className="estado-row" style={{display: 'flex', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid #ccc'}}>
                <span>Estado:</span>
                <div className="estado-indicator" style={{display: 'flex', alignItems: 'center', gap: '8px'}}>
                  <span className={`led ${estadoData.proxyServer.status === 'ok' ? 'led-green' : estadoData.proxyServer.status === 'unknown' ? 'led-gray' : 'led-red'}`} style={{display: 'inline-block', width: '12px', height: '12px', borderRadius: '50%', backgroundColor: estadoData.proxyServer.status === 'ok' ? '#4CAF50' : estadoData.proxyServer.status === 'unknown' ? '#999' : '#f44336'}}></span>
                  <strong>{estadoData.proxyServer.status === 'ok' ? 'Online' : estadoData.proxyServer.status === 'unknown' ? 'Desconhecido' : 'Offline'}</strong>
                </div>
              </div>
              <div className="estado-row" style={{display: 'flex', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid #ccc'}}>
                <span>Porta:</span>
                <strong>8001</strong>
              </div>
              <div className="estado-row" style={{display: 'flex', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid #ccc'}}>
                <span>Uptime:</span>
                <strong>{estadoData.proxyServer.uptime > 0 ? `${Math.floor(estadoData.proxyServer.uptime / 60)}m ${estadoData.proxyServer.uptime % 60}s` : '-'}</strong>
              </div>
              <div className="estado-row" style={{display: 'flex', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid #ccc'}}>
                <span>Base de Dados:</span>
                <strong style={{fontSize: '0.85em', wordBreak: 'break-all'}}>{estadoData.proxyServer.db || '-'}</strong>
              </div>
              <div className="estado-row" style={{display: 'flex', justifyContent: 'space-between', padding: '8px 0', borderBottom: '1px solid #ccc'}}>
                <span>Sincronizações Pendentes:</span>
                <strong style={{color: estadoData.proxyServer.pending > 0 ? '#ff9800' : 'inherit'}}>{estadoData.proxyServer.pending}</strong>
              </div>
            </div>

            <div className="stat-card">
              <h3>Legenda</h3>
              <div className="estado-row" style={{display: 'flex', alignItems: 'center', gap: '12px', padding: '8px 0'}}>
                <div className="estado-indicator" style={{display: 'flex', alignItems: 'center', gap: '8px'}}>
                  <span className="led led-green" style={{display: 'inline-block', width: '12px', height: '12px', borderRadius: '50%', backgroundColor: '#4CAF50'}}></span>
                  <span>Online / OK</span>
                </div>
              </div>
              <div className="estado-row" style={{display: 'flex', alignItems: 'center', gap: '12px', padding: '8px 0'}}>
                <div className="estado-indicator" style={{display: 'flex', alignItems: 'center', gap: '8px'}}>
                  <span className="led led-red" style={{display: 'inline-block', width: '12px', height: '12px', borderRadius: '50%', backgroundColor: '#f44336'}}></span>
                  <span>Offline / Erro</span>
                </div>
              </div>
              <div className="estado-row" style={{display: 'flex', alignItems: 'center', gap: '12px', padding: '8px 0'}}>
                <div className="estado-indicator" style={{display: 'flex', alignItems: 'center', gap: '8px'}}>
                  <span className="led led-gray" style={{display: 'inline-block', width: '12px', height: '12px', borderRadius: '50%', backgroundColor: '#999'}}></span>
                  <span>Desconhecido</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>
      )}

      <footer className="statusbar">
        <div>
          {activeTab === "restos" && `${filteredInventory.length} itens${searchActive ? ` (filtrados de ${inventory.length})` : ""}`}
          {activeTab === "stats" && "Estatísticas"}
          {activeTab === "optimizer" && `${cuttingList.length} peças na lista de corte`}
          {activeTab === "carrinhas" && `${vans.length} carrinhas | ${cargoItems.length} itens a carregar`}
          {activeTab === "estado" && "Monitorização de serviços"}
        </div>
        <div className="spacer"/>
        <div>
          {selectedIds.length === 0 && "Sem selecao"}
          {selectedIds.length === 1 && `Selecionado #${selectedIds[0]}`}
          {selectedIds.length > 1 && `${selectedIds.length} itens selecionados`}
        </div>
      </footer>

      {addOpen && (
        <div className="modal" role="dialog" aria-modal="true">
          <div className="modal-content">
            <div className="modal-head">
              <h3>Adicionar Resto</h3>
              <button className="btn" onClick={() => setAddOpen(false)}>✕</button>
            </div>
            <form className="form-grid" onSubmit={handleAdd}>
              <label>
                <span>Largura (mm)</span>
                <input type="number" min={1} required value={addForm.width_mm} onChange={(e)=>setAddForm(s=>({...s,width_mm:e.target.value}))} />
              </label>
              <label>
                <span>Altura (mm)</span>
                <input type="number" min={1} required value={addForm.height_mm} onChange={(e)=>setAddForm(s=>({...s,height_mm:e.target.value}))} />
              </label>
              <label>
                <span>Espessura (mm)</span>
                <input type="number" min={1} required value={addForm.thickness_mm} onChange={(e)=>setAddForm(s=>({...s,thickness_mm:e.target.value}))} />
              </label>
              <label>
                <span>Material</span>
                <input required value={addForm.material} onChange={(e)=>setAddForm(s=>({...s,material:e.target.value}))} />
              </label>
              <label className="wide">
                <span>Notas</span>
                <input value={addForm.notes} onChange={(e)=>setAddForm(s=>({...s,notes:e.target.value}))} />
              </label>
              <div className="modal-actions">
                <button type="button" className="btn" onClick={()=>setAddOpen(false)}>Cancelar</button>
                <button type="submit" className="btn primary" disabled={!validAdd}>Adicionar</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {searchOpen && (
        <div className="modal" role="dialog" aria-modal="true">
          <div className="modal-content">
            <div className="modal-head">
              <h3>Procurar Melhor Peça</h3>
              <button className="btn" onClick={() => setSearchOpen(false)}>✕</button>
            </div>
            <form className="form-grid" onSubmit={handleSearch}>
              <label>
                <span>Largura (mm)</span>
                <input type="number" min={1} required value={searchForm.width_mm} onChange={(e)=>setSearchForm(s=>({...s,width_mm:e.target.value}))} />
              </label>
              <label>
                <span>Altura (mm)</span>
                <input type="number" min={1} required value={searchForm.height_mm} onChange={(e)=>setSearchForm(s=>({...s,height_mm:e.target.value}))} />
              </label>
              <label>
                <span>Espessura (mm)</span>
                <input type="number" min={1} required value={searchForm.thickness_mm} onChange={(e)=>setSearchForm(s=>({...s,thickness_mm:e.target.value}))} />
              </label>
              <label>
                <span>Material</span>
                <input required value={searchForm.material} onChange={(e)=>setSearchForm(s=>({...s,material:e.target.value}))} />
              </label>
              <div className="modal-actions">
                <button type="button" className="btn" onClick={()=>setSearchOpen(false)}>Cancelar</button>
                <button type="submit" className="btn primary" disabled={!validSearch}>Procurar</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {editOpen && (
        <div className="modal" onClick={()=>setEditOpen(false)}>
          <div className="modal-content" onClick={(e)=>e.stopPropagation()}>
            <div className="modal-head">
              <strong>Editar Resto #{selectedIds[0]}</strong>
              <button onClick={()=>setEditOpen(false)}>✕</button>
            </div>
            <form className="form-grid" onSubmit={handleEdit}>
              <label>
                <span>Largura (mm)</span>
                <input type="number" min={1} value={editForm.width_mm} onChange={(e)=>setEditForm(s=>({...s,width_mm:e.target.value}))} />
              </label>
              <label>
                <span>Altura (mm)</span>
                <input type="number" min={1} value={editForm.height_mm} onChange={(e)=>setEditForm(s=>({...s,height_mm:e.target.value}))} />
              </label>
              <label>
                <span>Espessura (mm)</span>
                <input type="number" min={1} value={editForm.thickness_mm} onChange={(e)=>setEditForm(s=>({...s,thickness_mm:e.target.value}))} />
              </label>
              <label>
                <span>Material</span>
                <input value={editForm.material} onChange={(e)=>setEditForm(s=>({...s,material:e.target.value}))} />
              </label>
              <label className="wide">
                <span>Notas</span>
                <input value={editForm.notes} onChange={(e)=>setEditForm(s=>({...s,notes:e.target.value}))} />
              </label>
              <div className="modal-actions">
                <button type="button" className="btn" onClick={()=>setEditOpen(false)}>Cancelar</button>
                <button type="submit" className="btn primary">Guardar</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {settingsOpen && (
        <div className="modal" onClick={()=>setSettingsOpen(false)}>
          <div className="modal-content" onClick={(e)=>e.stopPropagation()}>
            <div className="modal-head">
              <strong>Definicoes</strong>
              <button onClick={()=>setSettingsOpen(false)}>X</button>
            </div>
            <form className="form-grid" onSubmit={(e) => { e.preventDefault(); setSettingsOpen(false); }}>
              <label>
                <span>Largura da Serra (mm)</span>
                <input 
                  type="number" 
                  min={0} 
                  step={0.1}
                  value={settings.kerfWidth} 
                  onChange={(e)=>setSettings(s=>({...s,kerfWidth:Number(e.target.value)}))} 
                />
              </label>
              <label>
                <span>Resto Minimo - Largura (mm)</span>
                <input 
                  type="number" 
                  min={0}
                  value={settings.minRemainderWidth} 
                  onChange={(e)=>setSettings(s=>({...s,minRemainderWidth:Number(e.target.value)}))} 
                />
              </label>
              <label>
                <span>Resto Minimo - Altura (mm)</span>
                <input 
                  type="number" 
                  min={0}
                  value={settings.minRemainderHeight} 
                  onChange={(e)=>setSettings(s=>({...s,minRemainderHeight:Number(e.target.value)}))} 
                />
              </label>
              <div className="modal-actions">
                <button type="button" className="btn" onClick={()=>setSettingsOpen(false)}>Fechar</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Van Modal */}
      {vanModalOpen && (
        <div className="modal-overlay" onClick={() => setVanModalOpen(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <h2>{editingVan ? 'Editar Carrinha' : 'Nova Carrinha'}</h2>
            <form onSubmit={(e) => {
              e.preventDefault();
              const formData = new FormData(e.target);
              const maxWeight = formData.get('max_weight_kg');
              const wheelHeight = formData.get('wheel_well_height_mm');
              const wheelWidth = formData.get('wheel_well_width_mm');
              const wheelStart = formData.get('wheel_well_start_x_mm');
              handleSaveVan({
                name: formData.get('name'),
                length_mm: Number(formData.get('length_mm')),
                width_mm: Number(formData.get('width_mm')),
                height_mm: Number(formData.get('height_mm')),
                max_weight_kg: maxWeight && Number(maxWeight) > 0 ? Number(maxWeight) : null,
                wheel_well_height_mm: wheelHeight && Number(wheelHeight) > 0 ? Number(wheelHeight) : null,
                wheel_well_width_mm: wheelWidth && Number(wheelWidth) > 0 ? Number(wheelWidth) : null,
                wheel_well_start_x_mm: wheelStart && Number(wheelStart) > 0 ? Number(wheelStart) : null,
                notes: formData.get('notes') || null
              });
            }}>
              <div className="form-grid">
                <label>
                  Nome:
                  <input 
                    type="text" 
                    name="name" 
                    defaultValue={editingVan?.name || ''} 
                    required 
                    placeholder="Carrinha 1, Sprinter..." 
                    autoFocus 
                  />
                </label>
                <label>
                  Comprimento (mm):
                  <input 
                    type="number" 
                    name="length_mm" 
                    defaultValue={editingVan?.length_mm || ''} 
                    required 
                    min="500" 
                    max="10000" 
                    placeholder="3000" 
                  />
                </label>
                <label>
                  Largura (mm):
                  <input 
                    type="number" 
                    name="width_mm" 
                    defaultValue={editingVan?.width_mm || ''} 
                    required 
                    min="500" 
                    max="5000" 
                    placeholder="1800" 
                  />
                </label>
                <label>
                  Altura (mm):
                  <input 
                    type="number" 
                    name="height_mm" 
                    defaultValue={editingVan?.height_mm || ''} 
                    required 
                    min="500" 
                    max="5000" 
                    placeholder="1900" 
                  />
                </label>
                <label>
                  Capacidade Máxima (kg):
                  <input 
                    type="number" 
                    name="max_weight_kg" 
                    defaultValue={editingVan?.max_weight_kg || ''} 
                    min="0" 
                    max="5000" 
                    placeholder="1000 (opcional)" 
                  />
                </label>
                <label>
                  Altura Rodas (mm):
                  <input 
                    type="number" 
                    name="wheel_well_height_mm" 
                    defaultValue={editingVan?.wheel_well_height_mm || ''} 
                    min="0" 
                    max="1000" 
                    placeholder="300 (opcional)" 
                    title="Altura que as rodas ocupam no chão"
                  />
                </label>
                <label>
                  Largura Rodas (mm):
                  <input 
                    type="number" 
                    name="wheel_well_width_mm" 
                    defaultValue={editingVan?.wheel_well_width_mm || ''} 
                    min="0" 
                    max="1000" 
                    placeholder="400 (opcional)" 
                    title="Largura da intrusão das rodas de cada lado"
                  />
                </label>
                <label>
                  Início Rodas (mm):
                  <input 
                    type="number" 
                    name="wheel_well_start_x_mm" 
                    defaultValue={editingVan?.wheel_well_start_x_mm || ''} 
                    min="0" 
                    max="5000" 
                    placeholder="1500 (opcional)" 
                    title="Distância da traseira onde as rodas começam"
                  />
                </label>
                <label style={{gridColumn: '1 / -1'}}>
                  Notas:
                  <textarea 
                    name="notes" 
                    defaultValue={editingVan?.notes || ''} 
                    rows="2" 
                    placeholder="Informações adicionais..."
                  />
                </label>
              </div>
              <div className="modal-actions">
                <button type="button" className="btn" onClick={() => setVanModalOpen(false)}>Cancelar</button>
                <button type="submit" className="btn btn-primary">{editingVan ? 'Guardar' : 'Adicionar'}</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Cargo Modal */}
      {cargoModalOpen && (
        <div className="modal-overlay" onClick={() => setCargoModalOpen(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <h2>{editingCargo ? 'Editar Item' : 'Adicionar Item de Carga'}</h2>
            <form onSubmit={(e) => {
              e.preventDefault();
              const formData = new FormData(e.target);
              handleSaveCargo({
                description: formData.get('description'),
                length_mm: Number(formData.get('length_mm')),
                width_mm: Number(formData.get('width_mm')),
                height_mm: Number(formData.get('height_mm')),
                weight_kg: Number(formData.get('weight_kg')) || 1,
                fragile: formData.get('fragile') === 'on',
                rotation_allowed: formData.get('rotation_allowed') === 'on',
                stackable: formData.get('stackable') === 'on',
                color: formData.get('color') || '#74c0fc'
              });
            }}>
              <div className="form-grid">
                <label style={{gridColumn: '1 / -1'}}>
                  Descrição:
                  <input 
                    type="text" 
                    name="description" 
                    defaultValue={editingCargo?.description || ''} 
                    required 
                    placeholder="Armário MDF, Mesa, Estante..." 
                    autoFocus 
                  />
                </label>
                <label>
                  Comprimento (mm):
                  <input 
                    type="number" 
                    name="length_mm" 
                    defaultValue={editingCargo?.length_mm || ''} 
                    required 
                    min="10" 
                    max="5000" 
                    placeholder="2000" 
                  />
                </label>
                <label>
                  Largura (mm):
                  <input 
                    type="number" 
                    name="width_mm" 
                    defaultValue={editingCargo?.width_mm || ''} 
                    required 
                    min="10" 
                    max="5000" 
                    placeholder="600" 
                  />
                </label>
                <label>
                  Altura (mm):
                  <input 
                    type="number" 
                    name="height_mm" 
                    defaultValue={editingCargo?.height_mm || ''} 
                    required 
                    min="10" 
                    max="5000" 
                    placeholder="1800" 
                  />
                </label>
                <label>
                  Peso (kg):
                  <input 
                    type="number" 
                    name="weight_kg" 
                    defaultValue={editingCargo?.weight_kg || ''} 
                    required
                    min="0.1" 
                    max="500" 
                    step="0.1" 
                    placeholder="80" 
                  />
                </label>
                <label>
                  Cor (visualização):
                  <input 
                    type="color" 
                    name="color" 
                    defaultValue={editingCargo?.color || '#74c0fc'} 
                  />
                </label>
              </div>
              <div style={{display: 'flex', flexDirection: 'column', gap: '8px', marginTop: '12px'}}>
                <label style={{display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer'}}>
                  <input 
                    type="checkbox" 
                    name="fragile" 
                    defaultChecked={editingCargo?.fragile || false}
                    style={{accentColor: '#d32f2f', margin: 0, flexShrink: 0}}
                  />
                  <span style={{whiteSpace: 'nowrap'}}>Frágil</span>
                </label>
                <label style={{display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer'}}>
                  <input 
                    type="checkbox" 
                    name="rotation_allowed" 
                    defaultChecked={editingCargo?.rotation_allowed !== false}
                    style={{accentColor: '#1976d2', margin: 0, flexShrink: 0}}
                  />
                  <span style={{whiteSpace: 'nowrap'}}>Permitir rotação</span>
                </label>
                <label style={{display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer'}}>
                  <input 
                    type="checkbox" 
                    name="stackable" 
                    defaultChecked={editingCargo?.stackable !== false}
                    style={{accentColor: '#388e3c', margin: 0, flexShrink: 0}}
                  />
                  <span style={{whiteSpace: 'nowrap'}}>Empilhável</span>
                </label>
              </div>
              <div className="modal-actions" style={{marginTop: '12px'}}>
                <button type="button" className="btn" onClick={() => setCargoModalOpen(false)}>Cancelar</button>
                <button type="submit" className="btn btn-primary">{editingCargo ? 'Guardar' : 'Adicionar'}</button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
