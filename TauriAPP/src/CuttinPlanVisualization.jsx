import React, { useState } from 'react';

const COLORS = {
  part: '#e3f2fd',
  partHover: '#bbdefb',
  partStroke: '#1565c0',
  waste: '#f5f5f5',
  grid: '#e0e0e0',
  text: '#1565c0'
};

const CuttingPlanVisualization = ({ plank, maxSheetDimension, onPartClick }) => {
  const [hoveredCut, setHoveredCut] = useState(null);

  // --- SAFETY CHECKS ---
  if (!plank || !plank.resto) {
    return <div style={{color: 'red'}}>Erro: Dados da placa inválidos</div>;
  }
  
  // Default dimensions if missing
  const width = plank.resto.width_mm || 1000;
  const height = plank.resto.height_mm || 1000;
  const safeMaxDim = maxSheetDimension || width;

  // --- CALCULATIONS ---
  // [*] CHANGED: Scaled down to 85% of the available width
  const relativeWidthPercent = Math.max(25, (width / safeMaxDim) * 85);
  
  const fontSize = Math.max(width, height) / 35;
  const strokeWidth = Math.max(width, height) / 500;

  return (
    <div className="cutting-plan-wrapper" style={{ marginBottom: '2rem' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '8px', fontSize: '0.9rem', borderBottom: '2px solid #333', paddingBottom: '4px' }}>
        <strong>
          #{plank.resto.id} {plank.resto.material} ({plank.resto.thickness_mm}mm)
        </strong>
        <span>
           {width} x {height} mm
          <span style={{ marginLeft: '10px', fontSize: '0.85em', color: (plank.wastePercent || 0) > 20 ? '#d32f2f' : '#388e3c' }}>
            ({(plank.wastePercent || 0).toFixed(1)}% Livre)
          </span>
        </span>
      </div>

      <div style={{ width: `${relativeWidthPercent}%`, transition: 'width 0.3s ease' }}>
        <div style={{ position: 'relative', border: '1px solid #999', borderRadius: '2px', overflow: 'hidden', boxShadow: '2px 2px 5px rgba(0,0,0,0.1)' }}>
          
          <svg 
            viewBox={`0 0 ${width} ${height}`} 
            style={{ width: '100%', display: 'block', background: COLORS.waste }}
            preserveAspectRatio="xMinYMin meet"
          >
            <defs>
              <pattern id={`grid-${plank.resto.id}`} width="100" height="100" patternUnits="userSpaceOnUse">
                <path d="M 100 0 L 0 0 0 100" fill="none" stroke={COLORS.grid} strokeWidth={strokeWidth} />
              </pattern>
            </defs>
            <rect width="100%" height="100%" fill={`url(#grid-${plank.resto.id})`} />

            {/* Origin Marker */}
            <circle cx="0" cy="0" r={strokeWidth * 4} fill="red" />
            <text x={strokeWidth * 6} y={strokeWidth * 12} fontSize={fontSize * 0.8} fill="red" fontWeight="bold">0,0</text>

            {(plank.cuts || []).map((cut, idx) => {
              const isHovered = hoveredCut === idx;
              return (
                <g 
                  key={idx}
                  onClick={() => onPartClick && onPartClick(cut)}
                  onMouseEnter={() => setHoveredCut(idx)}
                  onMouseLeave={() => setHoveredCut(null)}
                  style={{ cursor: 'pointer' }}
                >
                  <rect
                    x={cut.x}
                    y={cut.y}
                    width={cut.width_mm}
                    height={cut.height_mm}
                    fill={isHovered ? COLORS.partHover : COLORS.part}
                    stroke={COLORS.partStroke}
                    strokeWidth={strokeWidth}
                  />
                  
                  {cut.width_mm > 100 && cut.height_mm > 100 && (
                    <>
                      <text
                        x={cut.x + cut.width_mm / 2}
                        y={cut.y + cut.height_mm / 2}
                        textAnchor="middle"
                        dominantBaseline="middle"
                        fontSize={fontSize}
                        fill={COLORS.text}
                        fontWeight="600"
                        style={{ pointerEvents: 'none', textShadow: '0px 0px 2px white' }}
                      >
                        {cut.width_mm} x {cut.height_mm}
                      </text>
                      {cut.rotated && (
                        <text
                          x={cut.x + cut.width_mm / 2}
                          y={cut.y + cut.height_mm / 2 + fontSize * 1.2}
                          textAnchor="middle"
                          fontSize={fontSize * 0.8}
                          fill="#d32f2f"
                        >
                          ↻
                        </text>
                      )}
                    </>
                  )}
                </g>
              );
            })}
          </svg>

          {hoveredCut !== null && plank.cuts[hoveredCut] && (
            <div style={{
              position: 'absolute',
              top: 0,
              right: 0,
              background: 'rgba(255, 255, 255, 0.95)',
              border: '1px solid #ccc',
              borderBottomLeftRadius: '4px',
              padding: '8px',
              fontSize: '12px',
              pointerEvents: 'none',
              boxShadow: '-2px 2px 4px rgba(0,0,0,0.1)'
            }}>
              <div style={{fontWeight:'bold', color: COLORS.text}}>{plank.cuts[hoveredCut].material}</div>
              <div>Dim: {plank.cuts[hoveredCut].width_mm} x {plank.cuts[hoveredCut].height_mm}</div>
              <div style={{color:'#666', marginTop:'4px'}}>Posição:</div>
              <div>X: {plank.cuts[hoveredCut].x}mm</div>
              <div>Y: {plank.cuts[hoveredCut].y}mm</div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default CuttingPlanVisualization;