import { Canvas, useThree } from '@react-three/fiber';
import { OrbitControls, Line, Text, Html, ContactShadows } from '@react-three/drei';
import { useMemo, useState, useRef, useCallback } from 'react';
import * as THREE from 'three';

function DimensionLabel({ start, end, label, offset = [0, 0, 0], color = "#666" }) {
  const midPoint = [
    (start[0] + end[0]) / 2 + offset[0],
    (start[1] + end[1]) / 2 + offset[1],
    (start[2] + end[2]) / 2 + offset[2],
  ];
  
  return (
    <group>
      <Line points={[start, end]} color={color} lineWidth={1} dashed dashScale={10} />
      <Text
        position={midPoint}
        fontSize={0.12}
        color={color}
        anchorX="center"
        anchorY="middle"
      >
        {label}
      </Text>
    </group>
  );
}

function VanBox({ van, showDimensions = true }) {
  const { length_mm, width_mm, height_mm } = van;
  
  // Convert to meters for Three.js (scale down)
  const scale = 0.001;
  const l = length_mm * scale;
  const w = width_mm * scale;
  const h = height_mm * scale;

  // Wireframe edges
  const edges = useMemo(() => {
    return [
      // Bottom rectangle
      [[-l/2, 0, -w/2], [l/2, 0, -w/2]],
      [[l/2, 0, -w/2], [l/2, 0, w/2]],
      [[l/2, 0, w/2], [-l/2, 0, w/2]],
      [[-l/2, 0, w/2], [-l/2, 0, -w/2]],
      // Top rectangle
      [[-l/2, h, -w/2], [l/2, h, -w/2]],
      [[l/2, h, -w/2], [l/2, h, w/2]],
      [[l/2, h, w/2], [-l/2, h, w/2]],
      [[-l/2, h, w/2], [-l/2, h, -w/2]],
      // Vertical edges
      [[-l/2, 0, -w/2], [-l/2, h, -w/2]],
      [[l/2, 0, -w/2], [l/2, h, -w/2]],
      [[l/2, 0, w/2], [l/2, h, w/2]],
      [[-l/2, 0, w/2], [-l/2, h, w/2]],
    ];
  }, [l, w, h]);

  // Door frame at the back (+X side) - where loading happens
  const doorEdges = useMemo(() => {
    const inset = 0.02;
    return [
      [[l/2 - inset, 0.01, -w/2 + 0.05], [l/2 - inset, h - 0.05, -w/2 + 0.05]],
      [[l/2 - inset, h - 0.05, -w/2 + 0.05], [l/2 - inset, h - 0.05, w/2 - 0.05]],
      [[l/2 - inset, h - 0.05, w/2 - 0.05], [l/2 - inset, 0.01, w/2 - 0.05]],
    ];
  }, [l, w, h]);

  return (
    <group>
      {edges.map((points, i) => (
        <Line key={i} points={points} color="#333" lineWidth={2} />
      ))}
      
      {/* Door frame - thick green line at back (loading door) */}
      {doorEdges.map((points, i) => (
        <Line key={`door-${i}`} points={points} color="#4caf50" lineWidth={5} />
      ))}
      
      {/* Semi-transparent floor */}
      <mesh position={[0, 0.001, 0]} rotation={[-Math.PI / 2, 0, 0]}>
        <planeGeometry args={[l, w]} />
        <meshBasicMaterial color="#e8f5e9" transparent opacity={0.35} />
      </mesh>
      
      {/* Loading zone - green stripe at door */}
      <mesh position={[l/2 - 0.08, 0.003, 0]} rotation={[-Math.PI / 2, 0, 0]}>
        <planeGeometry args={[0.15, w - 0.1]} />
        <meshBasicMaterial color="#4caf50" transparent opacity={0.4} />
      </mesh>
      
      {/* Cab zone - gray stripe at front */}
      <mesh position={[-l/2 + 0.12, 0.003, 0]} rotation={[-Math.PI / 2, 0, 0]}>
        <planeGeometry args={[0.22, w - 0.1]} />
        <meshBasicMaterial color="#78909c" transparent opacity={0.4} />
      </mesh>
      
      {/* Front wall (cab side) - more visible */}
      <mesh position={[-l/2 + 0.005, h/2, 0]} rotation={[0, Math.PI / 2, 0]}>
        <planeGeometry args={[w - 0.02, h - 0.02]} />
        <meshBasicMaterial color="#546e7a" transparent opacity={0.25} side={THREE.DoubleSide} />
      </mesh>
      
      {/* CABINE label on front wall */}
      <Text
        position={[-l/2 - 0.12, h * 0.7, 0]}
        fontSize={0.12}
        color="#546e7a"
        anchorX="center"
        anchorY="middle"
        rotation={[0, Math.PI / 2, 0]}
        fontWeight="bold"
      >
        CABINE ▸
      </Text>
      
      {/* PORTA label at back (door) */}
      <Text
        position={[l/2 + 0.12, h * 0.7, 0]}
        fontSize={0.12}
        color="#2e7d32"
        anchorX="center"
        anchorY="middle"
        rotation={[0, -Math.PI / 2, 0]}
        fontWeight="bold"
      >
        ◂ PORTA
      </Text>
      
      {/* Loading arrow on floor pointing toward cab */}
      <Line 
        points={[[l/3, 0.015, 0], [-l/6, 0.015, 0]]} 
        color="#66bb6a" 
        lineWidth={3} 
      />
      <Line 
        points={[[-l/6 + 0.12, 0.015, 0.08], [-l/6, 0.015, 0], [-l/6 + 0.12, 0.015, -0.08]]} 
        color="#66bb6a" 
        lineWidth={3} 
      />
      
      {/* "Carregar" text on floor */}
      <Text
        position={[l/6, 0.02, w/3]}
        fontSize={0.07}
        color="#43a047"
        anchorX="center"
        anchorY="middle"
        rotation={[-Math.PI / 2, 0, 0]}
      >
        ← carregar primeiro
      </Text>
      
      {/* Dimension labels */}
      {showDimensions && (
        <>
          {/* Length - along X axis at the back */}
          <DimensionLabel 
            start={[-l/2, -0.1, -w/2 - 0.2]} 
            end={[l/2, -0.1, -w/2 - 0.2]} 
            label={`${length_mm}mm`}
            color="#1976d2"
          />
          {/* Width - along Z axis at the left */}
          <DimensionLabel 
            start={[-l/2 - 0.2, -0.1, -w/2]} 
            end={[-l/2 - 0.2, -0.1, w/2]} 
            label={`${width_mm}mm`}
            color="#388e3c"
          />
          {/* Height - along Y axis at the corner */}
          <DimensionLabel 
            start={[-l/2 - 0.2, 0, -w/2 - 0.2]} 
            end={[-l/2 - 0.2, h, -w/2 - 0.2]} 
            label={`${height_mm}mm`}
            color="#f57c00"
          />
        </>
      )}
    </group>
  );
}

function WheelWells({ van }) {
  if (!van.wheel_well_height_mm || !van.wheel_well_width_mm) return null;

  const scale = 0.001;
  const l = van.length_mm * scale;
  const w = van.width_mm * scale;
  const wh = van.wheel_well_height_mm * scale;
  const ww = van.wheel_well_width_mm * scale;
  const startX = (van.wheel_well_start_x_mm || 0) * scale;

  // Left and right wheel wells
  const wheelLength = l - startX;
  const wheelX = -l/2 + startX + wheelLength/2;
  
  return (
    <group>
      {/* Left wheel well */}
      <mesh position={[wheelX, wh/2, -w/2 + ww/2]}>
        <boxGeometry args={[wheelLength, wh, ww]} />
        <meshStandardMaterial color="#d32f2f" transparent opacity={0.4} />
      </mesh>
      
      {/* Right wheel well */}
      <mesh position={[wheelX, wh/2, w/2 - ww/2]}>
        <boxGeometry args={[wheelLength, wh, ww]} />
        <meshStandardMaterial color="#d32f2f" transparent opacity={0.4} />
      </mesh>
      
      {/* Label */}
      <Text
        position={[wheelX, wh + 0.05, -w/2 + ww/2]}
        fontSize={0.08}
        color="#d32f2f"
        anchorX="center"
        anchorY="bottom"
      >
        Roda
      </Text>
    </group>
  );
}

function CargoItem({ item, vanLength, vanWidth, index, opacity = 0.85, showLoadingOrder = true, onHover }) {
  const scale = 0.001;
  const [hovered, setHovered] = useState(false);
  
  // Backend structure: item.position.{x,y,z} and item.item.{length_mm, width_mm, height_mm}
  // Also item.placed_length, item.placed_width, item.placed_height for actual rotated dimensions
  const pos = item.position;
  const cargo = item.item;
  
  // Use placed dimensions if available (accounts for rotation), otherwise fall back to original
  const placedLength = item.placed_length || cargo.length_mm;
  const placedWidth = item.placed_width || cargo.width_mm;
  const placedHeight = item.placed_height || cargo.height_mm;
  
  // Backend coordinates are from back of van (0,0,0)
  // Convert to Three.js centered coordinates
  const x = (pos.x + placedLength / 2) * scale - (vanLength * scale) / 2;
  const y = (pos.y + placedHeight / 2) * scale;
  const z = (pos.z + placedWidth / 2) * scale - (vanWidth * scale) / 2;
  
  const dims = [
    placedLength * scale,
    placedHeight * scale,
    placedWidth * scale
  ];

  // Color logic: use item color, or red if fragile, blue otherwise
  const getColor = () => {
    if (cargo.color) return cargo.color;
    if (cargo.fragile) return '#ef5350';
    if (!cargo.stackable) return '#ffb74d';
    return '#64b5f6';
  };
  
  const handlePointerOver = useCallback((e) => {
    e.stopPropagation();
    setHovered(true);
    if (onHover) onHover(item);
    document.body.style.cursor = 'pointer';
  }, [item, onHover]);
  
  const handlePointerOut = useCallback(() => {
    setHovered(false);
    if (onHover) onHover(null);
    document.body.style.cursor = 'auto';
  }, [onHover]);

  return (
    <group position={[x, y, z]}>
      <mesh
        onPointerOver={handlePointerOver}
        onPointerOut={handlePointerOut}
      >
        <boxGeometry args={dims} />
        <meshStandardMaterial 
          color={hovered ? '#fff' : getColor()} 
          transparent 
          opacity={hovered ? 0.95 : opacity}
          roughness={0.6}
          metalness={0.1}
          emissive={hovered ? getColor() : '#000'}
          emissiveIntensity={hovered ? 0.3 : 0}
        />
      </mesh>
      {/* Wireframe edges */}
      <lineSegments>
        <edgesGeometry args={[new THREE.BoxGeometry(...dims)]} />
        <lineBasicMaterial color={hovered ? '#000' : '#333'} linewidth={hovered ? 2 : 1} />
      </lineSegments>
      {/* Label */}
      <Html
        position={[0, dims[1]/2 + 0.05, 0]}
        center
        style={{
          pointerEvents: 'none',
          whiteSpace: 'nowrap',
        }}
      >
        <div style={{
          background: hovered ? 'rgba(0,0,0,0.9)' : 'rgba(0,0,0,0.7)',
          color: 'white',
          padding: hovered ? '4px 8px' : '2px 6px',
          borderRadius: '3px',
          fontSize: hovered ? '12px' : '10px',
          fontWeight: 'bold',
          transition: 'all 0.2s',
          border: hovered ? '1px solid #fff' : 'none',
        }}>
          {showLoadingOrder && <span style={{color: '#ffc107', marginRight: '4px'}}>#{index + 1}</span>}
          {cargo.description || `Item ${index + 1}`}
          {cargo.fragile && ' [F]'}
        </div>
      </Html>
      {/* Hover details tooltip */}
      {hovered && (
        <Html
          position={[0, -dims[1]/2 - 0.1, 0]}
          center
          style={{ pointerEvents: 'none' }}
        >
          <div style={{
            background: 'rgba(255,255,255,0.95)',
            color: '#333',
            padding: '6px 10px',
            borderRadius: '4px',
            fontSize: '11px',
            boxShadow: '0 2px 8px rgba(0,0,0,0.2)',
            whiteSpace: 'nowrap',
          }}>
            <div><strong>{placedLength}×{placedWidth}×{placedHeight}</strong> mm</div>
            <div>{cargo.weight_kg} kg</div>
            <div>Pos: ({pos.x}, {pos.y}, {pos.z})</div>
          </div>
        </Html>
      )}
    </group>
  );
}

function CameraController({ controlsRef, van }) {
  const { camera } = useThree();
  const maxDim = Math.max(van.length_mm, van.width_mm, van.height_mm) * 0.001;
  
  const setView = useCallback((view) => {
    const d = maxDim * 1.8;
    const h = van.height_mm * 0.001 / 2;
    
    switch(view) {
      case 'top':
        camera.position.set(0, d * 2, 0);
        break;
      case 'front':
        camera.position.set(d * 1.5, h, 0);
        break;
      case 'side':
        camera.position.set(0, h, d * 1.5);
        break;
      case 'iso':
      default:
        camera.position.set(d, d * 0.8, d);
    }
    camera.lookAt(0, h, 0);
    if (controlsRef.current) {
      controlsRef.current.target.set(0, h, 0);
      controlsRef.current.update();
    }
  }, [camera, maxDim, van.height_mm, controlsRef]);
  
  // Expose setView to parent
  if (controlsRef.current) {
    controlsRef.current.setView = setView;
  }
  
  return null;
}

function Scene({ van, loadingPlan, opacity, showLoadingOrder, controlsRef }) {
  return (
    <>
      <ambientLight intensity={0.5} />
      <directionalLight position={[10, 15, 5]} intensity={0.8} castShadow />
      <directionalLight position={[-10, 10, -5]} intensity={0.3} />
      <hemisphereLight args={['#fff', '#666', 0.3]} />
      
      <CameraController controlsRef={controlsRef} van={van} />
      
      <VanBox van={van} showDimensions={true} />
      <WheelWells van={van} />
      
      {loadingPlan?.items.map((item, idx) => (
        <CargoItem 
          key={idx} 
          item={item} 
          vanLength={van.length_mm}
          vanWidth={van.width_mm}
          index={idx}
          opacity={opacity}
          showLoadingOrder={showLoadingOrder}
        />
      ))}
      
      {/* Contact shadows for depth */}
      <ContactShadows 
        position={[0, -0.01, 0]} 
        opacity={0.4} 
        scale={10} 
        blur={2} 
        far={4}
      />
      
      <OrbitControls 
        ref={controlsRef}
        enablePan={true}
        enableZoom={true}
        enableRotate={true}
        minDistance={1}
        maxDistance={25}
        target={[0, van.height_mm * 0.001 / 2, 0]}
      />
      
      {/* Grid helper */}
      <gridHelper args={[10, 20, '#aaa', '#ddd']} position={[0, -0.01, 0]} />
      
      {/* Axes helper - small, in corner */}
      <axesHelper args={[0.3]} position={[-van.length_mm * 0.001 / 2 - 0.5, 0, -van.width_mm * 0.001 / 2 - 0.5]} />
    </>
  );
}

function VanInfoPanel({ van, loadingPlan }) {
  const volume = van.length_mm * van.width_mm * van.height_mm;
  const volumeM3 = (volume / 1e9).toFixed(2);
  
  const usedVolume = loadingPlan?.used_volume || 0;
  const usedVolumeM3 = (usedVolume / 1e9).toFixed(2);
  const utilization = loadingPlan?.utilization_percent?.toFixed(1) || 0;
  
  return (
    <div style={{
      position: 'absolute',
      top: '10px',
      left: '10px',
      background: 'rgba(255,255,255,0.95)',
      padding: '12px 16px',
      borderRadius: '8px',
      boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
      fontSize: '13px',
      lineHeight: '1.6',
      zIndex: 10,
      minWidth: '200px',
    }}>
      <div style={{fontWeight: 'bold', fontSize: '15px', marginBottom: '8px', color: '#333'}}>
        {van.name}
      </div>
      <div style={{color: '#555'}}>
        <div><strong>Dimensões:</strong> {van.length_mm} × {van.width_mm} × {van.height_mm} mm</div>
        <div><strong>Volume:</strong> {volumeM3} m³</div>
        {van.max_weight_kg && <div><strong>Peso máx:</strong> {van.max_weight_kg} kg</div>}
        {van.wheel_well_height_mm && (
          <div><strong>Rodas:</strong> {van.wheel_well_height_mm}mm alt × {van.wheel_well_width_mm}mm larg</div>
        )}
      </div>
      {loadingPlan && (
        <div style={{marginTop: '10px', paddingTop: '10px', borderTop: '1px solid #e0e0e0'}}>
          <div style={{fontWeight: 'bold', color: '#1976d2'}}>Carregamento</div>
          <div><strong>Itens:</strong> {loadingPlan.items?.length || 0}</div>
          <div><strong>Ocupação:</strong> {usedVolumeM3} m³ ({utilization}%)</div>
          <div><strong>Peso total:</strong> {loadingPlan.total_weight?.toFixed(1) || 0} kg</div>
        </div>
      )}
    </div>
  );
}

function Legend() {
  return (
    <div style={{
      position: 'absolute',
      bottom: '10px',
      left: '10px',
      background: 'rgba(255,255,255,0.95)',
      padding: '10px 14px',
      borderRadius: '8px',
      boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
      fontSize: '11px',
      zIndex: 10,
      maxWidth: '280px',
    }}>
      <div style={{fontWeight: 'bold', marginBottom: '6px'}}>Legenda</div>
      <div style={{display: 'flex', gap: '10px', flexWrap: 'wrap', marginBottom: '8px'}}>
        <div style={{display: 'flex', alignItems: 'center', gap: '4px'}}>
          <div style={{width: '12px', height: '12px', background: '#64b5f6', borderRadius: '2px'}}></div>
          <span>Normal</span>
        </div>
        <div style={{display: 'flex', alignItems: 'center', gap: '4px'}}>
          <div style={{width: '12px', height: '12px', background: '#ef5350', borderRadius: '2px'}}></div>
          <span>Frágil</span>
        </div>
        <div style={{display: 'flex', alignItems: 'center', gap: '4px'}}>
          <div style={{width: '12px', height: '12px', background: '#ffb74d', borderRadius: '2px'}}></div>
          <span>Não empilhável</span>
        </div>
      </div>
      <div style={{borderTop: '1px solid #e0e0e0', paddingTop: '6px', display: 'flex', gap: '10px', flexWrap: 'wrap'}}>
        <div style={{display: 'flex', alignItems: 'center', gap: '4px'}}>
          <div style={{width: '12px', height: '12px', background: '#4caf50', borderRadius: '2px'}}></div>
          <span>Porta (carregar)</span>
        </div>
        <div style={{display: 'flex', alignItems: 'center', gap: '4px'}}>
          <div style={{width: '12px', height: '12px', background: '#78909c', borderRadius: '2px'}}></div>
          <span>Cabine</span>
        </div>
        <div style={{display: 'flex', alignItems: 'center', gap: '4px'}}>
          <div style={{width: '12px', height: '12px', background: '#d32f2f', opacity: 0.5, borderRadius: '2px'}}></div>
          <span>Rodas</span>
        </div>
      </div>
    </div>
  );
}

function Controls({ onViewChange }) {
  return (
    <div style={{
      position: 'absolute',
      bottom: '10px',
      right: '10px',
      background: 'rgba(255,255,255,0.95)',
      padding: '8px 12px',
      borderRadius: '8px',
      boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
      fontSize: '11px',
      color: '#666',
      zIndex: 10,
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
    }}>
      <div style={{display: 'flex', gap: '4px'}}>
        <button onClick={() => onViewChange('iso')} style={viewBtnStyle} title="Vista Isométrica">◈</button>
        <button onClick={() => onViewChange('top')} style={viewBtnStyle} title="Vista de Cima">↑</button>
        <button onClick={() => onViewChange('front')} style={viewBtnStyle} title="Vista Frontal">→</button>
        <button onClick={() => onViewChange('side')} style={viewBtnStyle} title="Vista Lateral">↗</button>
      </div>
      <div style={{fontSize: '10px', color: '#888', textAlign: 'center'}}>
        Rodar: arrastar | Zoom: scroll
      </div>
    </div>
  );
}

const viewBtnStyle = {
  padding: '4px 8px',
  border: '1px solid #ccc',
  borderRadius: '4px',
  background: '#fff',
  cursor: 'pointer',
  fontSize: '12px',
};

function VisualizationControls({ opacity, setOpacity, showLoadingOrder, setShowLoadingOrder }) {
  return (
    <div style={{
      position: 'absolute',
      top: '10px',
      right: '10px',
      background: 'rgba(255,255,255,0.95)',
      padding: '10px 14px',
      borderRadius: '8px',
      boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
      fontSize: '12px',
      zIndex: 10,
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
    }}>
      <div style={{display: 'flex', alignItems: 'center', gap: '8px'}}>
        <span>Transparência:</span>
        <input 
          type="range" 
          min="0.2" 
          max="1" 
          step="0.05" 
          value={opacity}
          onChange={(e) => setOpacity(parseFloat(e.target.value))}
          style={{width: '80px'}}
        />
      </div>
      <label style={{display: 'flex', alignItems: 'center', gap: '6px', cursor: 'pointer'}}>
        <input 
          type="checkbox" 
          checked={showLoadingOrder}
          onChange={(e) => setShowLoadingOrder(e.target.checked)}
        />
        <span>Mostrar ordem</span>
      </label>
    </div>
  );
}

export default function VanVisualization({ van, loadingPlan }) {
  const [opacity, setOpacity] = useState(0.85);
  const [showLoadingOrder, setShowLoadingOrder] = useState(true);
  const controlsRef = useRef();
  
  const handleViewChange = useCallback((view) => {
    if (controlsRef.current?.setView) {
      controlsRef.current.setView(view);
    }
  }, []);
  
  if (!van) {
    return (
      <div style={{
        display: 'flex', 
        alignItems: 'center', 
        justifyContent: 'center', 
        height: '100%', 
        color: '#999',
        flexDirection: 'column',
        gap: '10px',
      }}>
        <p>Selecione uma carrinha para visualizar</p>
      </div>
    );
  }

  // Calculate good camera position based on van size
  const maxDim = Math.max(van.length_mm, van.width_mm, van.height_mm) * 0.001;
  const cameraDistance = maxDim * 2;

  return (
    <div style={{width: '100%', height: '100%', minHeight: '500px', position: 'relative'}}>
      <VanInfoPanel van={van} loadingPlan={loadingPlan} />
      <VisualizationControls 
        opacity={opacity} 
        setOpacity={setOpacity}
        showLoadingOrder={showLoadingOrder}
        setShowLoadingOrder={setShowLoadingOrder}
      />
      <Legend />
      <Controls onViewChange={handleViewChange} />
      <Canvas
        camera={{ 
          position: [cameraDistance, cameraDistance * 0.8, cameraDistance], 
          fov: 50,
          near: 0.1,
          far: 1000
        }}
        shadows
        style={{background: 'linear-gradient(180deg, #f5f7fa 0%, #e4e8ec 100%)'}}
      >
        <Scene 
          van={van} 
          loadingPlan={loadingPlan} 
          opacity={opacity}
          showLoadingOrder={showLoadingOrder}
          controlsRef={controlsRef}
        />
      </Canvas>
    </div>
  );
}
