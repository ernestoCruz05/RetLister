import { Canvas } from '@react-three/fiber';
import { OrbitControls, Line } from '@react-three/drei';
import { useMemo } from 'react';
import * as THREE from 'three';

function VanBox({ van }) {
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

  return (
    <group>
      {edges.map((points, i) => (
        <Line key={i} points={points} color="#333" lineWidth={2} />
      ))}
      
      {/* Semi-transparent floor */}
      <mesh position={[0, 0.001, 0]} rotation={[-Math.PI / 2, 0, 0]}>
        <planeGeometry args={[l, w]} />
        <meshBasicMaterial color="#e0e0e0" transparent opacity={0.3} />
      </mesh>
    </group>
  );
}

function WheelWells({ van }) {
  if (!van.wheel_well_height_mm || !van.wheel_well_width_mm) return null;

  const scale = 0.001;
  const l = van.length_mm * scale;
  const wh = van.wheel_well_height_mm * scale;
  const ww = van.wheel_well_width_mm * scale;
  const startX = (van.wheel_well_start_x_mm || 0) * scale;

  // Left and right wheel wells
  const wheelLength = l - startX;
  
  return (
    <group>
      {/* Left wheel well */}
      <mesh position={[startX/2 - l/2 + wheelLength/2, wh/2, -van.width_mm * scale/2 + ww/2]}>
        <boxGeometry args={[wheelLength, wh, ww]} />
        <meshStandardMaterial color="#ff6b6b" transparent opacity={0.3} />
      </mesh>
      
      {/* Right wheel well */}
      <mesh position={[startX/2 - l/2 + wheelLength/2, wh/2, van.width_mm * scale/2 - ww/2]}>
        <boxGeometry args={[wheelLength, wh, ww]} />
        <meshStandardMaterial color="#ff6b6b" transparent opacity={0.3} />
      </mesh>
    </group>
  );
}

function CargoItem({ item, vanLength, vanWidth }) {
  const scale = 0.001;
  
  // Backend structure: item.position.{x,y,z} and item.item.{length_mm, width_mm, height_mm}
  const pos = item.position;
  const cargo = item.item;
  
  // Backend coordinates are from back of van (0,0,0)
  // Convert to Three.js centered coordinates
  const x = (pos.x + cargo.length_mm / 2) * scale - (vanLength * scale) / 2;
  const y = (pos.y + cargo.height_mm / 2) * scale;
  const z = (pos.z + cargo.width_mm / 2) * scale - (vanWidth * scale) / 2;
  
  const dims = [
    cargo.length_mm * scale,
    cargo.height_mm * scale,
    cargo.width_mm * scale
  ];

  // Color logic: use item color, or red if fragile
  const color = cargo.fragile ? '#f44336' : (cargo.color || '#74c0fc');

  return (
    <mesh position={[x, y, z]}>
      <boxGeometry args={dims} />
      <meshStandardMaterial 
        color={color} 
        transparent 
        opacity={0.8}
        roughness={0.7}
        metalness={0.1}
      />
      {/* Wireframe edges */}
      <lineSegments>
        <edgesGeometry args={[new THREE.BoxGeometry(...dims)]} />
        <lineBasicMaterial color="#000" linewidth={1} />
      </lineSegments>
    </mesh>
  );
}

function Scene({ van, loadingPlan }) {
  return (
    <>
      <ambientLight intensity={0.6} />
      <directionalLight position={[10, 10, 5]} intensity={0.8} />
      <directionalLight position={[-10, 10, -5]} intensity={0.4} />
      
      <VanBox van={van} />
      <WheelWells van={van} />
      
      {loadingPlan?.items.map((item, idx) => (
        <CargoItem 
          key={idx} 
          item={item} 
          vanLength={van.length_mm}
          vanWidth={van.width_mm}
        />
      ))}
      
      <OrbitControls 
        enablePan={true}
        enableZoom={true}
        enableRotate={true}
        minDistance={2}
        maxDistance={20}
      />
      
      {/* Grid helper */}
      <gridHelper args={[10, 20, '#888', '#ccc']} position={[0, 0, 0]} />
    </>
  );
}

export default function VanVisualization({ van, loadingPlan }) {
  if (!van) {
    return (
      <div style={{display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: '#999'}}>
        <p>Selecione uma carrinha para visualizar</p>
      </div>
    );
  }

  return (
    <div style={{width: '100%', height: '100%', minHeight: '500px'}}>
      <Canvas
        camera={{ 
          position: [5, 5, 5], 
          fov: 50,
          near: 0.1,
          far: 1000
        }}
        style={{background: '#f5f5f5'}}
      >
        <Scene van={van} loadingPlan={loadingPlan} />
      </Canvas>
    </div>
  );
}
