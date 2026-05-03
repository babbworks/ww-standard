## Technical briefing — Advanced Cooper / Dey / Bundy / Frick field system

This describes how to move from ASCII concepts to a **real streaming analytical + visualization system** inside Workwarrior (or a similar orchestration layer).

The core shift is:

> from “rendering diagrams” → to “computing a temporal state field from event streams”

---

# 1. System objective

Build a service that:

- ingests Workwarrior + Taskwarrior + Timewarrior + custom stream events
    
- normalizes them into a unified timeline
    
- computes multi-layer state fields (Dey, Bundy, Frick)
    
- produces a continuous “Cooper field”
    
- renders it as heatmaps / radial views / time-scrubbed interfaces
    

---

# 2. Core architecture

## Pipeline

```text
[Event Streams]
   ↓
[Normalizer]
   ↓
[Temporal Index Store]
   ↓
[Feature Extractors]
   ↓
[Dey | Bundy | Frick Models]
   ↓
[Field Composer (Cooper)]
   ↓
[Renderer Layer]
```

---

## 2.1 Event Streams (inputs)

Sources:

- Taskwarrior (`task start/stop/modify`)
    
- Timewarrior (`tracking intervals`)
    
- Workwarrior CLI events (`ww log`)
    
- system hooks (process + editor + file activity)
    
- optional agent events (AI actions)
    

Canonical event format:

```json
{
  "ts": 1714651200,
  "type": "task.start",
  "task_id": "42",
  "project": "build-core",
  "metadata": {
    "urgency": 8.4
  }
}
```

---

## 2.2 Normalizer

Converts everything into a **single timeline schema**:

```text
Event → (timestamp, actor, action, context, weight)
```

Outputs append-only log:

```text
t | actor | action | weight | tags
```

---

# 3. Feature layers (your conceptual models made real)

---

## 3.1 Dey layer (signal dynamics)

Purpose:

> continuous intensity + stability + fragmentation

Computed from sliding window:

```text
Dey(t) = {
  i = activity_density,
  s = state_stability,
  f = transition_frequency
}
```

Implementation:

- exponential moving average over activity rate
    
- variance of task switching
    
- entropy of project distribution
    

Output:

```text
DeyVector[t] = [i, s, f]
```

Used for:

- heat intensity
    
- turbulence mapping
    
- fatigue estimation
    

---

## 3.2 Bundy layer (interval system)

Purpose:

> structured time segmentation

Derived from:

- Taskwarrior active intervals
    
- Timewarrior tracking blocks
    
- inferred sessions (gaps < threshold)
    

Model:

```text
Interval = {
  start,
  end,
  type,
  project,
  confidence
}
```

Transforms into:

- time blocks
    
- boundaries
    
- session scaffolding
    

Used for:

- segmentation overlay
    
- ring partitioning
    
- schedule enforcement
    

---

## 3.3 Frick layer (event topology)

Purpose:

> discrete control flow graph of behavior

Model:

```text
EventGraph = {
  nodes: events,
  edges: transitions,
  weights: transition_probability
}
```

Derived from:

- task start/stop
    
- context switches
    
- project changes
    
- agent actions
    

Used for:

- detecting switching cost
    
- workflow interruptions
    
- causal structure
    

---

# 4. Cooper field (emergent synthesis layer)

This is NOT stored — it is computed.

```text
Cooper(t) = F(Dey(t), Bundy(t), Frick(t))
```

More explicitly:

```text
CooperField(x,y,t) =
    α·Dey(t)
  + β·IntervalDensity(Bundy,t)
  + γ·GraphActivity(Frick,t)
```

Outputs:

- scalar energy field
    
- vector flow field (optional)
    
- topology ridges (focus zones)
    

---

# 5. Temporal indexing (critical design point)

You need a dual index:

## A. Linear timeline index

```text
t → event list
```

## B. Circular projection index

```text
θ = (t / 24h) * 2π
```

This enables:

- linear debugging view
    
- circular visualization view
    

---

# 6. Rendering system

You have 3 valid rendering modes:

---

## 6.1 Heatmap (primary)

- grid-based canvas
    
- Cooper field intensity as color
    

Mapping:

```text
low → dark
mid → amber
high → white / cyan
```

---

## 6.2 Radial Cooper ring

- angle = time of day
    
- radius = intensity
    
- layers = overlays
    

Best implemented in:

- WebGL / Canvas
    
- SVG polar transform
    

---

## 6.3 Timeline field (debug mode)

- x = time
    
- y = intensity
    
- overlays = Bundy intervals
    
- markers = Frick events
    

---

# 7. Stream monitoring manager (new Workwarrior component)

This is the missing service you described earlier.

```text
WW Stream Manager
```

Responsibilities:

- subscribe to all event sources
    
- maintain rolling state window
    
- compute Dey/Bundy/Frick continuously
    
- publish Cooper field updates
    

Interfaces:

```bash
ww stream start
ww stream status
ww stream export --format=field
ww stream query --t 14:30
```

---

# 8. Agentic integration layer

This is where it becomes “AI employee infrastructure”.

Agents can:

### Read

- current Cooper field
    
- task density hotspots
    
- switching cost zones
    

### Write

- inject tasks
    
- annotate Frick transitions
    
- adjust Bundy intervals
    

Example:

```text
agent: "detect overload"
→ inserts Frick event: SWITCH
→ rebalances Bundy interval
→ lowers Dey pressure
```

---

# 9. Industrial / systems use case

This maps cleanly to factories:

|Layer|Industrial meaning|
|---|---|
|Dey|machine load / vibration / throughput|
|Bundy|shift schedules / production runs|
|Frick|machine state transitions|
|Cooper|plant-wide energy/efficiency field|

Result:

> a live operational “heat + behavior map” of a factory

---

# 10. Key implementation stack

Minimal viable stack:

- Rust or Go (stream processor)
    
- SQLite / TimescaleDB (event store)
    
- WebSocket stream server
    
- Canvas/WebGL frontend
    
- CLI (Workwarrior integration layer)
    

---

# 11. Core design insight

Everything reduces to:

```text
Event Stream → State Field → Visual Manifold
```

Not:

- charts
    
- logs
    
- dashboards
    

But:

> a continuously evolving behavioral physics simulation of work

---

# One-line summary

This system becomes an event-driven temporal state engine where Workwarrior streams are normalized into a unified event graph, decomposed into Dey (signal dynamics), Bundy (interval structure), and Frick (event topology), and recomposed into a continuously updated Cooper field rendered as heatmaps or radial projections for real-time human and agent interpretation.