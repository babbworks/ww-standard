## Workwarrior Data Formats — Implementation Briefing

This defines the **concrete storage and interchange formats** needed to implement the full system. Each layer can be built independently, but all compile from a shared canonical stream.

---

# 1. Canonical format (WW Stream v0)

This is the only required persistent format.

### Line structure

```text
<t> <op> <a> <b> <c>
```

### Fields

|Field|Type|Meaning|
|---|---|---|
|`t`|int|timestamp (minutes or epoch-min)|
|`op`|char|opcode (`D`, `F`, `T`, `B`, `S`)|
|`a`|float/int|primary payload|
|`b`|float/int|secondary payload|
|`c`|float/int|optional payload|

---

## Opcodes

|Code|Meaning|
|---|---|
|`D`|Dey (continuous state sample)|
|`F`|Frick (event boundary)|
|`T`|Task / semantic label|
|`B`|Binding (link task ID ↔ stream segment)|
|`S`|System / regen / snapshot|

---

## Example

```text
480 D 0.6 0.7 0.2
482 F 1 0 0
482 B 2 0 0
540 F 2 0 0
```

---

# 2. Derived format: Dey (signal model)

Not stored separately; reconstructed from `D` events.

### Internal representation

```python
DeySample {
    t: int
    i: float   # intensity
    s: float   # stability
    f: float   # fragmentation
}
```

### Optional cache format (for performance)

```text
t,i,s,f
480,0.6,0.7,0.2
485,0.4,0.6,0.3
```

Used only for rendering acceleration.

---

# 3. Frick format (event layer)

Derived from `F` opcodes.

### Internal structure

```python
FrickEvent {
    t: int
    type: enum {START, STOP, PAUSE, RESUME, SWITCH}
}
```

### Compact encoding

```text
t,type
482,START
540,PAUSE
545,SWITCH
```

---

# 4. Bundy format (interval layer)

Derived from Frick boundaries.

### Structure

```python
Interval {
    start: int
    end: int
    task_id: optional
}
```

### Storage format

```text
start,end,task
480,540,2
540,600,3
```

---

# 5. Task binding format

This connects WW stream ↔ Taskwarrior IDs.

### Structure

```python
Binding {
    task_id: string/int
    interval_id: int
}
```

### Storage

```text
interval_start,interval_end,task_id
480,540,2
540,600,3
```

---

# 6. Cooper format (geometry projection)

This is a **render-only structure**, derived from Dey.

### Structure

```python
CooperPoint {
    angle: float
    radius: float
    stability: float
    fragmentation: float
}
```

### Optional serialized form

```text
angle,radius,s,f
0.12,0.6,0.7,0.2
0.18,0.5,0.6,0.3
```

Used only for visualization caching.

---

# 7. System / Regeneration format

Stores long-term adaptive state.

### Structure

```python
RegenerationState {
    fatigue: float
    elasticity: float
    crystallization: map[string,float]
}
```

### Storage

```text
fatigue=0.42
elasticity=0.71
build=0.8
debug=0.3
```

---

# 8. Taskwarrior integration format (bridge layer)

This is NOT the stream. It is a mapping layer.

```text
ww_task_id,taskwarrior_id,active
2,task.abc123,true
3,task.def456,false
```

Used only for sync.

---

# 9. Timewarrior integration format

Derived from Bundy:

```text
start,end,description
480,540,"task:2 build"
540,600,"task:3 debug"
```

---

# 10. Event log (optional debug format)

Human-readable expansion of stream:

```text
480 D intensity=0.6 stability=0.7 frag=0.2
482 F START
482 B task:2
540 F PAUSE
```

Used for debugging only.

---

# 11. Storage architecture

## Primary

```text
stream.log   (canonical WW format)
```

## Derived caches (rebuildable)

```text
dey.cache
frick.cache
bundy.cache
cooper.cache
regen.state
```

All caches are disposable.

---

# 12. Processing pipeline

```text
WW Stream
   ↓
Replay Engine
   ↓
Dey reconstruction
   ↓
Frick extraction
   ↓
Bundy intervals
   ↓
Task binding overlay
   ↓
Cooper projection
   ↓
Regeneration update
```

---

# 13. Key design constraint

All formats obey:

### Rule 1 — Stream is source of truth

Everything else is derived.

### Rule 2 — Deterministic replay

Same stream → identical outputs.

### Rule 3 — No cross-format mutation

Only stream writes are authoritative.

---

# 14. Minimal implementation set (MVP)

To implement a working system:

### Required

- WW stream parser
    
- replay engine
    
- Frick boundary reducer
    
- Bundy interval builder
    
- Task binding adapter
    

### Optional

- Cooper renderer
    
- Dey cache
    
- regeneration model
    
- Timewarrior sync
    

---

# 15. One-line summary

> The system is implemented as a single append-only event stream, with deterministic reducers that generate specialized formats for signal (Dey), events (Frick), intervals (Bundy), identity bindings (Taskwarrior), and geometry (Cooper), all of which are discardable projections of the same underlying data.