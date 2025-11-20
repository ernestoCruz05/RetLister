# Carregador de Carrinhas - Especificações Técnicas

## Visão Geral
Sistema de otimização de carregamento de carrinhas (vans) para transporte de móveis, usando algoritmo de **3D bin packing** com visualização isométrica em Three.js.

---

## 1. Gestão de Frota

### 1.1 Dados de Carrinha
```typescript
interface Van {
  id: number;
  name: string;                     // "Carrinha 1", "Sprinter Mercedes"
  length_mm: number;                // Comprimento interno (ex: 3000mm)
  width_mm: number;                 // Largura interna (ex: 1800mm)
  height_mm: number;                // Altura interna (ex: 1900mm)
  max_weight_kg: number;            // Capacidade de carga (ex: 1000kg)
  wheel_well_height_mm?: number;    // Altura das rodas (ex: 300mm)
  wheel_well_width_mm?: number;     // Largura da intrusão de cada lado (ex: 400mm)
  wheel_well_start_x_mm?: number;   // Onde as rodas começam desde a traseira (ex: 1500mm)
  active: boolean;                  // Disponível para uso
  notes?: string;                   // Notas adicionais
}
```

**Wheel Wells (Rodas):**
```
Vista de cima (topo):
<-- length_mm -->
TRASEIRA                           FRENTE
┌─────────────────────────────────┐
│                                 │
│  ZONA LIVRE                     │  <- Antes do wheel_well_start_x_mm
│                                 │
├──────┬─────────────────┬────────┤  <- wheel_well_start_x_mm
│RODA  │  ESPAÇO CENTRO  │  RODA  │
└──────┴─────────────────┴────────┘
   ^                           ^
   wheel_well_width_mm cada lado

Vista lateral:
_______________________
|                     |  <- Tecto (height_mm)
|  ESPAÇO UTILIZÁVEL  |
|_____________________|
 |  |            |  |   <- Rodas (wheel_well_height_mm desde o chão)
 |__|____________|__|
```

**Zona proibida (não colocar items):**
- X: `wheel_well_start_x_mm` até o fim da carrinha (`length_mm`)
- Y: `0` até `wheel_well_height_mm`
- Z (largura): `0` até `wheel_well_width_mm` **OU** `width_mm - wheel_well_width_mm` até `width_mm`

Valores típicos:
- Altura: 250-400mm
- Largura cada lado: 300-500mm  
- Início: 1000-2000mm (meio/frente da carrinha)

### 1.2 Operações CRUD
- ✅ **Adicionar** nova carrinha
- ✅ **Editar** dimensões/capacidade
- ✅ **Desativar** (não apagar - manter histórico)
- ✅ **Listar** carrinhas ativas

---

## 2. Itens de Carga (Móveis/Materiais)

### 2.1 Dados de Item
```typescript
interface CargoItem {
  id: number;
  load_session_id?: number;  // Agrupa items de uma carga específica
  description: string;       // "Armário MDF", "Mesa", "Estante"
  length_mm: number;
  width_mm: number;
  height_mm: number;
  weight_kg: number;
  
  // Restrições
  fragile: boolean;          // Não pode ter peso em cima
  rotation_allowed: boolean; // Pode rodar? (alguns móveis têm orientação fixa)
  stackable: boolean;        // Pode empilhar outros em cima?
  
  // Metadados
  priority?: number;         // Ordem de descarga (1 = sai primeiro)
  color?: string;            // Cor na visualização 3D
}
```

### 2.2 Regras de Validação
- Dimensões: 10mm - 5000mm (móveis razoáveis)
- Peso: 0.1kg - 500kg
- Descrição: obrigatória, max 128 caracteres

---

## 3. Algoritmo de Otimização

### 3.1 Objetivos (em ordem de prioridade)
1. **Maximizar utilização de espaço** - Usar mínimo de carrinhas
2. **Respeitar peso máximo** - Não exceder capacidade
3. **Estabilidade** - Centro de massa equilibrado
4. **Ordem de descarga** - Items prioritários acessíveis (LIFO)
5. **Proteção** - Frágeis não levam peso em cima

### 3.2 Algoritmo Proposto: **3D Guillotine + Best-Fit**

#### Fase 1: Ordenação de Items
```
1. Items FRÁGEIS primeiro (vão para cima)
2. Items PESADOS no fundo (estabilidade)
3. Items GRANDES preenchem cantos
4. Prioridade de descarga (LIFO - últimos a entrar ficam acessíveis)
```

#### Fase 2: Posicionamento (Bottom-Left-Back)
```
Para cada item:
  1. Tenta posições livres, começando por (0,0,0)
  2. Verifica colisões com items já colocados
  3. Calcula "fit score":
     - Quanto espaço desperdiça
     - Quão estável fica (centro de massa)
     - Se respeita restrições (peso sobre frágil)
  4. Escolhe melhor posição
  5. Se rotation_allowed, testa 6 orientações
```

#### Fase 3: Níveis/Andares
```
Agrupa items por altura (Z):
  Nível 0: 0-600mm (chão)
  Nível 1: 600-1200mm
  Nível 2: 1200-1800mm
  etc.
```

### 3.3 Restrições Físicas
- **Suporte**: Item só pode ser colocado se tiver suporte >= 70% da base
- **Peso**: Soma de pesos acima <= capacidade de carga do item de baixo
- **Altura máxima**: Não exceder altura interna da carrinha
- **Fragilidade**: Items frágeis não podem ter peso em cima (weight_above = 0)

---

## 4. Visualização 3D (Three.js)

### 4.1 Tecnologias
- **@react-three/fiber** - React renderer para Three.js
- **@react-three/drei** - Helpers (OrbitControls, Grid, etc.)
- **three** - Core 3D engine

### 4.2 Features da Visualização
- ✅ Vista isométrica (45° angle)
- ✅ OrbitControls para rodar/zoom
- ✅ Grid no chão da carrinha
- ✅ Cores diferentes por item (ou por tipo)
- ✅ Labels com descrição e dimensões
- ✅ Wireframe da carrinha (edges)
- ✅ Botão "Ver Nível X" - mostra só items daquele andar
- ✅ Highlight ao hover (outline glow)
- ✅ Click para ver detalhes do item

### 4.3 Esquema de Cores
```javascript
// Sugestão
const colors = {
  fragile: '#ff6b6b',      // Vermelho
  heavy: '#4a4a4a',        // Cinza escuro
  stackable: '#51cf66',    // Verde
  default: '#74c0fc',      // Azul
  van_wireframe: '#868e96' // Cinza
};
```

---

## 5. Fluxo de Trabalho (UX)

### Cenário Típico:
```
1. Utilizador seleciona "Carrinha 1" (3000×1800×1900mm)
2. Adiciona items:
   - Armário MDF: 2000×600×1800mm, 80kg, frágil=false
   - Mesa: 1400×800×750mm, 45kg, frágil=false
   - Estante: 1200×400×2100mm, 35kg, frágil=true (!)
   - Cadeiras (4x): 450×450×900mm, 8kg cada
   
3. Clica "Gerar Plano"
4. Sistema calcula:
   - Peso total: 80+45+35+32 = 192kg ✅ (< 1000kg)
   - Espaço necessário: ~5.2m³ ✅ (< 10.26m³)
   - Estante FRÁGIL vai para cima
   - Armário pesado no fundo
   
5. Visualização mostra:
   - Nível 0 (chão): Armário + Mesa
   - Nível 1 (meio): Cadeiras empilhadas
   - Nível 2 (topo): Estante (frágil, sem peso em cima)
   
6. Utilizador pode:
   - Rodar vista 3D
   - Ver planta 2D por nível
   - Exportar lista de carregamento (PDF?)
   - Ajustar manualmente (drag & drop futuro)
```

---

## 6. Architecture Decision: In-Memory Cargo

**Cargo items are NOT stored in the database** - they exist only in React state during planning.

### Rationale:
- ✅ **Simpler**: No session management, no cleanup needed
- ✅ **Faster**: No DB round-trips for temporary planning
- ✅ **Cleaner**: Database only stores persistent data (vans, final plans)

### Data Flow:
```
1. User selects van from DB
2. User adds items to in-memory list (React state)
3. Click "Gerar Plano" → sends {van_id, items: [...]} to /optimize
4. Backend calculates positions, returns plan
5. Optionally save final plan to loading_plans table
```

## 7. Structure of Base de Dados

### 7.1 Tabela: `vans`
```sql
CREATE TABLE vans (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  length_mm INTEGER NOT NULL CHECK(length_mm > 0),
  width_mm INTEGER NOT NULL CHECK(width_mm > 0),
  height_mm INTEGER NOT NULL CHECK(height_mm > 0),
  max_weight_kg INTEGER CHECK(max_weight_kg > 0),
  active BOOLEAN DEFAULT 1,
  notes TEXT,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### 6.2 Tabela: `cargo_items`
```sql
CREATE TABLE cargo_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  load_session_id INTEGER,  -- NULL = em edição, valor = carga finalizada
  description TEXT NOT NULL,
  length_mm INTEGER NOT NULL CHECK(length_mm BETWEEN 10 AND 5000),
  width_mm INTEGER NOT NULL CHECK(width_mm BETWEEN 10 AND 5000),
  height_mm INTEGER NOT NULL CHECK(height_mm BETWEEN 10 AND 5000),
  weight_kg REAL NOT NULL CHECK(weight_kg > 0),
  fragile BOOLEAN DEFAULT 0,
  rotation_allowed BOOLEAN DEFAULT 1,
  stackable BOOLEAN DEFAULT 1,
  priority INTEGER DEFAULT 0,
  color TEXT,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### 6.3 Tabela: `loading_plans` (histórico)
```sql
CREATE TABLE loading_plans (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  van_id INTEGER NOT NULL REFERENCES vans(id),
  plan_date DATE DEFAULT CURRENT_DATE,
  total_items INTEGER,
  total_weight_kg REAL,
  utilization_percent REAL,  -- Espaço usado / espaço total
  plan_json TEXT,            -- JSON com posições 3D de cada item
  notes TEXT,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

---

## 8. API Endpoints (Backend)

### 8.1 Vans
```
GET    /api/vans              - Listar todas
POST   /api/vans              - Criar nova
PUT    /api/vans/:id          - Atualizar
PUT    /api/vans/:id          - Atualizar
DELETE /api/vans/:id          - Desativar (soft delete)
```

### 8.2 Otimização (Cargo is in-memory, sent in request)
```
POST   /api/optimize
Body: {
  van_id: number,
  items: [{
    description: string,
    length_mm: number,
    width_mm: number,
    height_mm: number,
    weight_kg: number,
    fragile: boolean,
    rotation_allowed: boolean,
    stackable: boolean,
    color?: string
  }]
}
Response: {
  success: boolean,
  plan?: {
    items: [{
      item: CargoItem,
      position: {x, y, z},
      rotation: {x, y, z},
      level: number
    }],
    total_weight: number,
    utilization_percent: number,
    van_volume: number,
    used_volume: number
  },
  warnings: string[]
}
```

---

## 9. Casos Extremos & Edge Cases

### 8.1 Item não cabe
```
- Avisar utilizador: "Estante (2100mm altura) excede altura da carrinha (1900mm)"
- Sugerir: rodar, ou dividir item, ou usar carrinha maior
```

### 8.2 Peso excede capacidade
```
- Avisar: "Peso total (1200kg) excede capacidade (1000kg)"
- Sugerir: remover items ou dividir em 2 carrinhas
```

### 8.3 Impossível otimizar
```
- Items muito irregulares
- Muitas restrições conflitantes
- Fallback: mostrar "melhor esforço" com avisos
```

### 8.4 Ordem de descarga
```
- Items com priority=1 devem ficar perto da porta (último nível)
- LIFO: se item A vai para cliente antes de B, A deve estar acessível
```

---

## 9. Melhorias Futuras (v2)

- 🔮 **Drag & drop manual** - Ajustar posições na vista 3D
- 🔮 **Export PDF/PNG** - Plano de carregamento impresso
- 🔮 **Histórico** - Ver cargas anteriores
- 🔮 **Templates** - Guardar configurações frequentes
- 🔮 **Multi-carrinha** - Otimizar para várias carrinhas simultaneamente
- 🔮 **Rota otimizada** - Integrar com ordem de entrega GPS
- 🔮 **AR Preview** - Ver carregamento em realidade aumentada (mobile)

---

## 10. Prioridades de Implementação

### Sprint 1: MVP Básico
- [x] UI skeleton (tab Carrinhas)
- [ ] CRUD de carrinhas (modal add/edit)
- [ ] CRUD de cargo items
- [ ] Algoritmo 3D bin packing simples (sem restrições avançadas)
- [ ] Visualização básica Three.js (cubos coloridos)

### Sprint 2: Restrições & UX
- [ ] Fragilidade, peso, rotação
- [ ] Validação de limites
- [ ] Melhorar algoritmo (estabilidade, LIFO)
- [ ] Labels e hover na vista 3D

### Sprint 3: Polish
- [ ] Níveis/andares com toggle
- [ ] Backend endpoints
- [ ] Guardar histórico de planos
- [ ] Export básico (JSON ou print)

---

## Questões em Aberto

1. **Perspetiva da carrinha**: Vista de trás (porta aberta) ou de cima?
   - Sugestão: Isométrica com rotação livre (OrbitControls)

2. **Unidade de medida**: Sempre mm? Permitir cm?
   - Sugestão: Input em cm, guardar em mm (consistência com leftovers)

3. **Items parciais**: E se móvel for desmontável (ex: mesa = tampo + 4 pernas)?
   - Sugestão: v1 trata como unidade, v2 permite "kit" de items

4. **Peso distribuído**: Como calcular pressão sobre items de baixo?
   - Sugestão: Simplificar - soma total sobre item, stackable=false se frágil

5. **Gaps/espaços**: Deixar margem entre items (5cm?) para segurança?
   - Sugestão: Sim, adicionar 50mm padding em cada dimensão na colisão

---

**Status**: 📋 Spec Draft v1.0  
**Última atualização**: 2025-11-17  
**Próximo passo**: Implementar CRUD de carrinhas + items
