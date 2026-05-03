## Workwarrior Stream System — Technical Briefing

### 0. Objective

Implement a **canonical, append-only event stream** that unifies:

- Taskwarrior (identity + user intent)
    
- Timewarrior (intervals)
    
- Continuous behavioral signals (Dey)
    
- Derived structure (Frick, Bundy)
    
- Projections (Cooper) and learning (Regeneration)
    

The system must be **deterministic, replayable, and adapter-driven**. External tools remain intact.

---

# 1. Core Principle

> **Single source of truth = WW Stream (append-only log)**  
> All views (tasks, intervals, signals, geometry) are **derived via deterministic replay**.

---

# 2. Canonical Event Format (v0)

Line-oriented, fixed tokens:

```text
<t> <op> <a> <b> <c>
```

- `t` : integer timestamp (epoch minutes or seconds)
    
- `op`: opcode (`D`, `F`, `T`, `B`, `S`)
    
- `a,b,c`: numeric payload (floats/ints; 0 if unused)
    

## Opcodes

- `D` — Dey sample (continuous state)
    
- `F` — Frick event (state boundary)
    
- `T` — Task/semantic tag
    
- `B` — Binding (attach WW ID to timeline)
    
- `S` — System (regen, snapshots, etc.)
    

## Examples

```text
480 D 0.6 0.7 0.2   # intensity, stability, fragmentation
482 F 1 0 0         # START
482 B 2 0 0         # bind task:2
540 F 2 0 0         # PAUSE
545 F 4 0 0         # SWITCH
550 D 0.4 0.5 0.3
```

---

# 3. Data Model (derived, not stored)

```python
class WWState:
    dey: list[DeySample]
    frick: list[FrickEvent]
    bundy: list[Interval]
    bindings: dict[Interval, TaskID]
    cooper: list[CooperPoint]
    regeneration: RegenerationState
```

---

# 4. Replay Engine (deterministic)

## Contract

- Pure function: `state = replay(stream, t_end=None)`
    
- No side effects, no hidden clocks
    
- Same input → identical output
    

## Skeleton

```python
def replay(stream, t_end=None):
    state = init_state()

    for e in stream:
        if t_end and e.t > t_end:
            break
        state = apply(state, e)

    state.cooper = project_cooper(state.dey)
    state.regeneration = update_regen(state)

    return state
```

## Reducers

- `D` → append/interpolate Dey samples
    
- `F` → open/close/split intervals
    
- `B` → bind active interval → task ID
    
- `T` → semantic tags (optional)
    
- `S` → update regeneration state
    

---

# 5. Frick & Bundy Rules

- `F START` → open interval
    
- `F STOP` → close interval
    
- `F PAUSE` → close (paused)
    
- `F RESUME`→ reopen
    
- `F SWITCH`→ close current, open new
    

Bundy intervals are **derived during replay** (not stored).

---

# 6. Cooper Projection (post-replay)

- map `time → angle`
    
- map `intensity → radius`
    
- use:
    
    - `stability` → smoothing
        
    - `fragmentation` → curvature noise
        

```python
def project_cooper(dey):
    pts = []
    for s in dey:
        angle = time_to_angle(s.t)
        radius = s.i
        pts.append((angle, radius, s.s, s.f))
    return pts
```

---

# 7. Regeneration (persistent learning)

Stored via `S` events (optional snapshots).

State:

```python
class RegenerationState:
    elasticity: float
    fatigue: float
    crystallization: dict[str, float]
    fracture_memory: dict[str, float]
```

Update rules (example):

```python
fatigue += avg(fragmentation)
elasticity -= interruption_rate
crystallization[task] += sustained_blocks
```

---

# 8. Integration with Taskwarrior / Timewarrior

## Do not replace them. Wrap them.

### Wrapper pattern

```bash
ww task 2 start
```

Internally:

1. call:
    

```bash
task 2 start
```

2. emit:
    

```text
t F 1        # START
t B 2        # bind task:2
t D 0.6 0.8 0.1
```

### Stop

```bash
task 2 stop
```

→

```text
t F 5        # STOP
```

### Switch (implicit)

```bash
task 3 start
```

→

```text
t F 4        # SWITCH
t B 3
```

---

# 9. Stream Monitoring Manager (SMM)

## Role

Observe external signals and emit **D events + inferred F events**.

## Architecture

```text
sources → observers → normalizer → WW stream
```

### Observers (plugins)

- TaskObserver
    
- TimeObserver
    
- ShellObserver
    
- FocusObserver
    
- IdleObserver
    

### Normalizer

Maps raw signals → WW events:

```python
if idle:
    emit D(i=0.1, s=0.4, f=0.3)

if keystroke_rate_high:
    emit D(i=0.8, s=0.9, f=0.05)

if long idle gap:
    emit F(PAUSE)
```

## Sampling

- fixed interval (e.g., 30s)
    
- debounce + thresholds required
    
- must be deterministic
    

---

# 10. Storage

Single file:

```text
~/.ww/stream.log
```

Optional:

- periodic snapshots (`S` events)
    
- ephemeral caches for views
    

---

# 11. CLI Surface

### Emit

```bash
ww emit D 0.5 0.7 0.2
ww emit F START
```

### Sync

```bash
ww sync taskwarrior
ww sync timewarrior
```

### Views (derived)

```bash
ww view dey
ww view frick
ww view bundy
ww view cooper
```

### Replay

```bash
ww replay --full
ww replay --at 10:30
```

---

# 12. Determinism Requirements

- ordered stream
    
- idempotent reducers
    
- no implicit state outside stream
    
- any randomness must be seeded
    

---

# 13. Performance

- append-only writes
    
- periodic checkpoints (every N events)
    
- partial replay (snapshot + tail)
    
- lazy projections (Cooper computed on demand)
    

---

# 14. Non-goals (for v1)

- no ML/AI inference
    
- no multi-user merge
    
- no distributed sync
    
- no schema evolution complexity
    

---

# 15. Deliverables (v1)

1. **Stream writer**
    
2. **Replay engine**
    
3. **Taskwarrior wrapper**
    
4. **Basic SMM (idle + command + task)**
    
5. **Bundy + Frick derivation**
    
6. **Basic Cooper ASCII renderer**
    

---

# 16. One-line system definition

> **Workwarrior = append-only event stream + deterministic replay engine + adapters that project the same history into tasks, intervals, signals, and geometry.**