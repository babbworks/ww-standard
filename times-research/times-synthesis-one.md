# The Workwarrior Stream System — Technical Synthesis

**For:** Claude Design — explanatory webpage brief  
**Audience:** Technical users, developers, and systems architects  
**Scope:** Complete technical treatment — architecture, lineage, formats, visualization, agentic and industrial applications  

---

## Overview

The Workwarrior Stream System (WWSS) is a deterministic, append-only event substrate that unifies task identity, behavioral signals, time intervals, and geometric visualization into a single replayable timeline. It is not a feature added to Workwarrior — it is the kernel from which all other representations are derived.

The system rests on one principle:

> **A single append-only stream is the source of truth. All views — tasks, intervals, signals, and geometry — are derived through deterministic replay.**

This design has a concrete lineage stretching from 19th-century industrial timekeeping through double-entry accounting through modern version control. The technical achievement is collapsing those historical models into one unified event format.

---

## Part I — Historical Lineage: Where the Layers Come From

Each layer in the system is named for and modeled on a real historical precedent. None are invented in isolation.

### Dey Layer — Continuous Analog Recording

Alexander Dey's dial-based time recorders (late 19th century) introduced rotating surfaces where a continuous trace replaced discrete punches. Time became a *field*, not a series of entries. Workwarrior's Dey layer inherits this: a sampled, continuous behavioral signal carrying three values at each point in time.

### Frick Layer — Discrete Event Impulses

Frederick W. Frick's mechanical recorders emphasized instantaneous marks — punches and state changes rather than duration. The system advances via discrete events, not continuous measurement. In Workwarrior, Frick events are the structural boundaries: START, STOP, PAUSE, RESUME, SWITCH.

### Bundy Layer — Interval Accumulation

Willard Le Grand Bundy's late-19th-century time clocks established start/stop recording and accumulation of worked time into bounded intervals. Bundy intervals are not stored in the stream — they are *derived* from Frick boundaries during replay. The interval is always a computation, never a fact on disk.

### Hollerith Encoding — Compact Machine-Readable Data

Herman Hollerith's punched-card systems (1890 US Census) introduced symbolic encoding of real-world states into compact, machine-readable records suitable for aggregation. The stream's `<t> <op> <a> <b> <c>` line format is directly analogous to a punched card row: fixed tokens, numeric payload, machine-processable.

### Pacioli / Git — Append-Only Reconstruction

From Luca Pacioli's double-entry accounting (1494 onward): the present state of a system is a derivation of recorded history, not a stored snapshot. The ledger is never corrected — only appended to. Modern version control (Git) formalized the same principle for software: history is primary, state is computed. Workwarrior extends this from files to behavioral time-state.

### The Synthesis

None of these components are novel individually. The technical achievement is collapsing them all into a **single unified event stream** where:

- Bundy → intervals (derived)
- Frick → events (explicit)
- Dey → continuous signal (sampled)
- Hollerith → encoding (compact schema)
- Pacioli/Git → replay (deterministic reconstruction)

---

## Part II — The Stream Format

The only required persistent artifact is a single append-only log file:

```
~/.ww/stream.log
```

### Canonical Line Structure

```
<t> <op> <a> <b> <c>
```

| Field | Type | Meaning |
|-------|------|---------|
| `t` | int | Timestamp (epoch minutes or epoch seconds) |
| `op` | char | Opcode: `D`, `F`, `T`, `B`, `S` |
| `a` | float/int | Primary payload |
| `b` | float/int | Secondary payload |
| `c` | float/int | Optional third payload (0 if unused) |

### Opcodes

| Code | Name | Meaning |
|------|------|---------|
| `D` | Dey | Continuous state sample: intensity, stability, fragmentation |
| `F` | Frick | State transition: START, STOP, PAUSE, RESUME, SWITCH |
| `T` | Tag | Semantic annotation or label |
| `B` | Binding | Link a Taskwarrior ID to the current stream segment |
| `S` | System | Regeneration snapshot, system events |

### Worked Example

```
480 D 0.6 0.7 0.2   # intensity=0.6, stability=0.7, fragmentation=0.2
482 F 1 0 0         # START
482 B 2 0 0         # bind task:2
540 F 2 0 0         # PAUSE
545 F 4 0 0         # SWITCH
550 D 0.4 0.5 0.3
```

This encodes: a focused work block on task 2 starting at t=482, paused at t=540, switching context at t=545, with measured behavioral state at both endpoints.

### Why This Format

The format is deliberately minimal — five tokens per line, no JSON, no headers, no schema versioning. This gives:

- Append-only writes with minimal I/O overhead
- Human-readable debugging without tools
- Exact analogy to punched-card encoding (fixed-width, positional)
- Deterministic replay by any parser that reads lines in order

---

## Part III — The Four Derived Layers

All derived structures are computed during replay. Nothing below is stored authoritatively.

### Dey — Continuous Signal Dynamics

Each `D` event produces a three-dimensional sample:

```python
DeySample {
    t: int
    i: float   # intensity — activity density / load
    s: float   # stability — coherence of execution state
    f: float   # fragmentation — context switching frequency
}
```

The full Dey layer is computed from a sliding window over the stream:

- **intensity**: exponential moving average over activity rate
- **stability**: variance of task-switching patterns
- **fragmentation**: entropy of project distribution over a time window

Dey is the system's behavioral CPU meter — a real-time readout of cognitive or operational load that is neither a task count nor a simple timer. It carries information that neither TaskWarrior nor TimeWarrior can express: *how* work is happening, not just *what* or *when*.

### Frick — Discrete State Transitions

```python
FrickEvent {
    t: int
    type: enum { START, STOP, PAUSE, RESUME, SWITCH }
}
```

Frick events are the control-flow graph of behavior. They define structural boundaries in the timeline. During replay, the Frick reducer:

- `F START` → opens an interval
- `F STOP` → closes the active interval
- `F PAUSE` → closes with paused state
- `F RESUME` → reopens
- `F SWITCH` → closes current, opens new (implicit in `ww task 3 start`)

Context switching is an inferred state transition, not a command pair. The system detects it from task binding changes.

### Bundy — Derived Intervals

```python
Interval {
    start: int
    end: int
    task_id: optional str
    confidence: float
}
```

Bundy intervals are derived entirely from Frick boundaries. They are never stored. The interval is always computed — a decision made during replay, not a fact written to disk. This means the *same stream* can produce different interval segmentations if the Frick reducer rules change, without touching any stored data.

Bundy also handles inferred sessions: gaps below a threshold are bridged into a single interval, giving a more human-meaningful segmentation than raw start/stop pairs.

### Regeneration — Long-Term Adaptive State

```python
RegenerationState {
    fatigue: float
    elasticity: float
    crystallization: dict[str, float]
    fracture_memory: dict[str, float]
}
```

Regeneration tracks how repeated patterns of work affect the system's state over longer time horizons than a single session. Update rules:

```python
fatigue += avg(fragmentation)
elasticity -= interruption_rate
crystallization[task] += sustained_blocks
```

This is stored via `S` events as optional snapshots. It allows the system to model operator recovery, habituation to specific task types, and accumulating cognitive cost from sustained fragmentation.

---

## Part IV — Cooper: The Geometric Projection Layer

Cooper is not a stored layer. It is a post-replay computation that transforms the Dey time series into a geometric field:

```
Cooper(t) = F(Dey(t), Bundy(t), Frick(t))
```

More precisely:

```
CooperField(x, y, t) =
    α · Dey(t)
  + β · IntervalDensity(Bundy, t)
  + γ · GraphActivity(Frick, t)
```

### The Core Projection

```python
def project_cooper(dey):
    pts = []
    for s in dey:
        angle = time_to_angle(s.t)   # time → polar angle (24h → 2π)
        radius = s.i                  # intensity → radius
        pts.append((angle, radius, s.s, s.f))
    return pts
```

| Dimension | Source | Visual encoding |
|-----------|--------|----------------|
| Angle | Time of day | Position around ring |
| Radius | Dey intensity | Distance from center |
| Smoothness | Dey stability | Edge coherence |
| Deformation | Dey fragmentation | Geometric distortion |

The result is a **state-space embedding in polar coordinates**. The entire work day becomes a single deformable shape. A focused, productive day produces a smooth ring with consistent radius. A fragmented, interrupted day produces jagged, asymmetric geometry. A collapsed day (overload or abandonment) inverts toward the center.

### Why This Matters Beyond Visualization

Cooper is not primarily a display technique. As a computational object, it is a mapping from time-series state vectors to a continuous geometric manifold. This enables:

**Anomaly detection via geometry divergence.** Normal behavior produces stable ring structure. Anomalies appear as radial bulges (spikes), jagged edges (fragmentation), angular jitter (instability), or radial collapse (overload). Detection becomes distance from canonical manifold — no threshold tuning required for individual metrics.

**Geometry-driven agent scheduling.** Agents can use the Cooper field as a scheduler input: high-radius zones indicate high-load periods where injecting tasks is costly; stable arcs indicate optimal execution windows; fragmentation zones indicate where context switching should be avoided.

**Multi-agent coordination as vector field alignment.** Multiple agents contributing to the same stream each generate Cooper fields that can be overlaid. Overlap produces contention zones; divergence indicates specialization; alignment reveals cooperative execution windows. Coordination becomes vector field geometry rather than message passing.

**Reinforcement learning reward shaping.** Cooper-derived metrics define reward signals without explicit programming: stability up → reward; fragmentation up → penalty; sustained intensity → nonlinear reward curve favoring coherent work trajectories over raw throughput.

**Temporal clustering and archetype discovery.** Cooper fields can be clustered by radial density distributions, curvature signatures, and deformation gradients — producing unsupervised discovery of operational archetypes: "types of days," "types of tasks," "types of agent behavior."

**Debugging agent failures geometrically.** Looping arcs indicate stuck reasoning. Radial collapse indicates overload. Discontinuities indicate context loss. The failure becomes visible in shape before it appears in logs.

---

## Part V — The Replay Engine

The replay engine is the read side of the entire system. Its contract:

```python
state = replay(stream, t_end=None)
```

Properties:
- **Pure function** — no side effects, no hidden clocks
- **Deterministic** — same input → identical output, always
- **No hidden state** — everything reconstructable from the stream

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

Reducers by opcode:
- `D` → append/interpolate Dey samples
- `F` → open/close/split Bundy intervals
- `B` → bind active interval → task ID
- `T` → semantic tags
- `S` → update regeneration state

**Performance model:** Append-only writes are cheap. Periodic snapshots (`S` events) allow partial replay: snapshot + tail instead of full stream. Cooper is computed lazily on demand. Derived caches (Dey, Frick, Bundy, Cooper) are all disposable — they can be rebuilt from the stream at any time.

**Determinism requirements:**
- Stream must be ordered by timestamp
- Reducers must be idempotent
- No implicit state outside the stream
- Any randomness must be seeded from stream data

---

## Part VI — System Architecture (WWSS as Kernel)

The key architectural decision: WWSS is not a feature. It is the runtime substrate everything else depends on.

### Position in the Stack

```
WWSS (Stream Service)
   ↓
Task adapter      (Taskwarrior hooks → B and F events)
   ↓
Time adapter      (F transitions → Timewarrior start/stop)
   ↓
Agent adapters    (agent inferences → T, D, F events)
   ↓
CLI / UI / APIs   (read projections, write commands)
```

Taskwarrior is no longer the authoritative system — it becomes the identity layer. WWSS is the temporal layer. The binding `B task:2` connects a Taskwarrior ID to a stream segment. Identity lives in TaskWarrior; time-shaped reality lives in WWSS.

### The Three Core Responsibilities

**1. Ingest** — Accept events from CLI commands, Taskwarrior hooks, Timewarrior hooks, agent processes, and SMM observers. Normalize inputs, enforce schema, assign timestamps.

**2. Append-only storage** — Atomic appends to `stream.log`. No mutation. No updates. No deletes. Idempotent retry handling for duplicate event delivery.

**3. Replay + projection API** — Deterministic reconstruction of Dey, Frick, Bundy, task bindings, Cooper geometry, and regeneration state. All views are derived endpoints, not stored data.

### Failure Model

WWSS tolerates:
- Missing agent input
- Delayed Taskwarrior hooks
- Duplicate events
- Partial ingestion

Rules: replay fixes inconsistency; the stream is never corrected, only appended to; contradictions become observable signal (they increase fragmentation in the Dey layer, making them visible in Cooper geometry).

### System Invariants

These must always hold:

1. **Append-only truth** — No modification of history
2. **Deterministic replay** — Same stream → same state
3. **No hidden state** — Everything reconstructable
4. **Time-first model** — All meaning is time-indexed

---

## Part VII — The Stream Monitoring Manager (SMM)

The SMM is the active observation subsystem. It runs inside WWSS, continuously sampling external signals and emitting D events and inferred F transitions.

```
SMM → observer plugins → normalized WW events → stream
```

### Observer Plugins

- **TaskObserver** — monitors Taskwarrior state changes
- **TimeObserver** — monitors Timewarrior tracking
- **ShellObserver** — monitors terminal activity
- **FocusObserver** — monitors application focus
- **IdleObserver** — detects idle periods

### Normalization Rules

```python
if idle:
    emit D(i=0.1, s=0.4, f=0.3)

if keystroke_rate_high:
    emit D(i=0.8, s=0.9, f=0.05)

if long_idle_gap:
    emit F(PAUSE)
```

Sampling is fixed-interval (e.g., 30s) with debounce and thresholds. The SMM can only *write* events — it can never modify past ones.

---

## Part VIII — CLI as Compiler Front-End

Every CLI command is a macro-expansion into stream events. The CLI is not an interface to the system — it is a compiler front-end for a time-structured event machine.

### Command Taxonomy

**Task lifecycle commands** — expand to F transitions + B bindings

```bash
ww task 2 start
# →  F START
#    B task:2
#    D intensity bump
```

```bash
ww task 3 start     # implicit switch
# →  F SWITCH
#    B task:3
```

**Stream primitive commands** — direct insertion for agents, SMM, debug

```bash
ww emit D 0.5 0.7 0.2
ww emit F START
ww emit T "note text"
```

**Observation commands** — deterministic projections, all read-only

```bash
ww view dey
ww view frick
ww view bundy
ww view cooper
```

**Replay commands**

```bash
ww replay --full
ww replay --at 10:30
ww replay --from 08:00 --to 12:00
ww replay --task 2
```

**Agent interface commands**

```bash
ww agent run optimizer
ww agent handoff task:2
# →  F HANDOFF agent=optimizer
#    B task:2
#    T context window
```

**System commands**

```bash
ww sync taskwarrior
ww sync timewarrior
ww daemon start
ww smm start
```

### The Full CLI → Stream → Projection Cycle

```
CLI command
   ↓
event emission
   ↓
stream.log append
   ↓
replay engine
   ↓
derived structures (Dey / Frick / Bundy)
   ↓
Cooper projection
   ↓
CLI views + agent inputs
```

Every action is both input and future computational substrate. The system is closed.

---

## Part IX — Data Format Specifications

### Primary Storage

```
stream.log           ← only required artifact
```

### Derived / Rebuildable Caches

```
dey.cache            ← CSV: t,i,s,f
frick.cache          ← CSV: t,type
bundy.cache          ← CSV: start,end,task
cooper.cache         ← CSV: angle,radius,s,f
regen.state          ← key=value: fatigue, elasticity, crystallization
```

All caches are disposable. The stream is canonical.

### Dey Cache Format

```
t,i,s,f
480,0.6,0.7,0.2
485,0.4,0.6,0.3
```

### Frick Cache Format

```
t,type
482,START
540,PAUSE
545,SWITCH
```

### Bundy Interval Format

```
start,end,task
480,540,2
540,600,3
```

### Cooper Point Format

```
angle,radius,s,f
0.12,0.6,0.7,0.2
0.18,0.5,0.6,0.3
```

### Regeneration State Format

```
fatigue=0.42
elasticity=0.71
build=0.8
debug=0.3
```

### Taskwarrior Bridge Format (not stream — sync layer only)

```
ww_task_id,taskwarrior_id,active
2,task.abc123,true
3,task.def456,false
```

---

## Part X — ASCII Visualization Layer

The ASCII layer is a deterministic projection of the stream onto a fixed-width character grid. It is not a fallback — it is a first-class rendering that directly maps the same primitives driving the underlying system.

### Primitive Mappings

| Layer | Data type | ASCII primitive |
|-------|-----------|----------------|
| Dey | Continuous signal | Height / density / block characters |
| Frick | Discrete events | Markers: `│ ● S P R ↑ ↓` |
| Bundy | Intervals | Horizontal bars |
| Task | Identity | Labels / ANSI color |
| Cooper | Geometry | Radial / deformed fields |

### Bundy Timeline (simplest rendering)

```
build  ███████████████████
email       ████
debug           █████████████
```

Width = duration. Row = task identity. Segmentation = Frick-derived.

### Frick Event Markers

```
build  ███████|██|████████
              ↑  ↑
           pause resume
```

### Dey Intensity Waveform

```
▁▂▃▄▅▆▇█▇▆▅▄▃▂▁
```

High blocks = peak intensity. Jaggedness = fragmentation. Smoothness = stability.

### Combined Bundy + Dey (core hybrid)

```
build  ███████████████████
       ▁▂▃▄▅▆▇███▇▆▅▄▃▂▁

email       ████
            ▂▃▄▅

debug           █████████████
                ▅▆▇███▇▆▅▄
```

Duration from Bundy. Quality from Dey. Together: how long and how well.

### Cooper Ring (radial full-day view)

```
        ████████▓▓▓▓▓▓▒▒▒▒
    ███████████▓▓▓▒▒▒▒░░░░░
  ████████▓▓▓▓▓▒▒▒░░░░░░░░░
 ███████▓▓▓▓▒▒▒▒░░░░░░░░░░░
 █████████▓▓▓▓▓▒▒▒▒░░░░░░░
  ████████▓▓▓▒▒▒▒░░░░░░░░
    ███████▓▓▓▒▒▒▒░░░░
        ████████▓▓▓
```

The entire work day compressed to one shape. Center = low load / idle. Outer ring = peak intensity. Distortion = fragmentation.

### Full Multi-Layer Composite

```
TIME  08:00      09:00      10:00      11:00

TASK  build███████████ email██ debug████████

DEY   ▁▂▃▄▅▆▇███▇▆▅▄▃▂▁▂▃▄▅▆▇

FRK   S───────P──R───────────S

COOP       ███████████▓▓▓▓▒▒▒░░░
```

### Cooper Variants

**Fragmentation-heavy day:**

```
        ████████▓▓▒▒▒▒░░░░
    ███████▓▓▒▒▒▒░░░░░░░
  ██████▓▓▓▒▒▒▒░░░░░░░
```

Jagged asymmetry = frequent context switching. Broken radial continuity = interrupted execution chains.

**Time-annotated ring:**

```
          12:00
       ██████████
   09:00 ███▓▓▒▒░░ 15:00
       ████████▓▓
          18:00
```

**Industrial / multi-subsystem Cooper:**

```
   HVAC ████████▓▓▓▒▒▒▒░░░
   LINE ███████████▓▓▓▒▒▒▒
   ROBOT ████████▓▓▓▓▒▒▒▒░
   QC    ██████▓▓▒▒▒▒░░░░
```

Each sector = a machine subsystem. Radius = throughput load. Fragmentation = faults.

---

## Part XI — Agentic Systems: The Operational Case

Modern agent systems fail in predictable ways:

- **Fragmented memory** — LLM agent memory ≠ execution logs ≠ task trackers
- **No temporal structure** — agents understand discrete steps, not work rhythm
- **No grounding in operational load** — no signal for fatigue, fragmentation, focus drift

WWSS solves this by giving agents a shared time spine where every action is anchored:

```
time → state → action → outcome
```

### Agents as Stream Citizens

Agents are not external actors. They are deterministic participants in the same event system as humans.

**Reading:** An agent replays a window to reconstruct current state:

```bash
ww replay --from 08:00 --to now
```

It receives: current Dey state (load, stability, fragmentation), active Bundy intervals, task bindings, unresolved Frick transitions.

**Writing:** An agent appends to the stream exactly as a human does:

```
510 D i=0.7 s=0.9 f=0.1
512 F SWITCH
512 B task:2
```

No special agent API. No privileged channel. Same stream.

### Handoffs as Stream Events

Instead of meeting notes, status updates, or ticket transfers:

```
F HANDOFF agent=optimizer task:2 context=stream:480-520
```

The receiving agent resumes via replay:

```bash
ww replay stream[480:520]
```

No re-explaining required. The handoff is a state transition in the record, not a communication act.

### Self-Regulating Behavior

With Dey and Cooper available, agents can govern their own execution:

- **Fragmentation spike** → reduce concurrency
- **Low stability** → pause, request clarification
- **Sustained high intensity** → schedule recovery block
- **Cooper radial collapse** → trigger overload alert

This is equivalent to an operating system scheduler applied to cognitive or computational work.

### Multi-Agent Coordination via Shared Geometry

Multiple agents operating concurrently each contribute to the same stream. Conflicts become visible:

- Overlapping Bundy intervals → resource contention
- Cooper distortion zones → coordination friction
- Fragmentation peaks → handoff cost

Coordination emerges as geometric structure, not message negotiation.

### Eliminating Sessions

Sessions disappear because state is not held in memory — it is reconstructed from the stream. Instead of `open session → work → close session`, the model is `continuous stream → replay at any point → act`. Agents and humans both enter work by selecting a time window, not opening a session.

---

## Part XII — Industrial Application

The system maps directly to factory and industrial operations contexts.

| WWSS Layer | Industrial Equivalent |
|-----------|----------------------|
| Dey | Sensor telemetry: load, vibration, throughput |
| Frick | Machine state transitions: start / stop / fault |
| Bundy | Production runs / shifts |
| Task bindings | Work orders / maintenance tickets |
| Cooper | Plant-wide operational "shape" — energy and efficiency field |

What industrial systems currently lack is not data — SCADA, PLC logs, sensor streams, and maintenance tickets already exist. What is missing is a **unified interpretive layer** connecting machine state → human intent → operational rhythm.

WWSS provides:

- **Continuous load field (Dey)** — where the factory is under pressure, moment to moment
- **Event boundaries (Frick)** — where transitions, faults, or shifts occur
- **Production intervals (Bundy)** — clean segmentation of operational cycles
- **Geometry of operation (Cooper)** — entire facility state as a single shape

The Cooper field becomes a "heat map of time dynamics" — not a chart, not a dashboard, but a topological projection of the entire system.

Managers gain rhythm understanding (when is the system stable vs. chaotic), structural bottleneck visibility (where fragmentation clusters), load balancing information (where intensity is consistently high), and change traceability (what caused shifts in throughput shape) — from a unified operational narrative rather than fragmented per-tool dashboards.

---

## Part XIII — What Makes This Architecturally Distinctive

Most operational systems are:

- **Reactive** — alert-driven, not structurally interpretable
- **Fragmented** — one tool per domain, no unified layer
- **Non-replayable** — lost history meaning; present state is not derivable from past

WWSS is:

- **Replayable** — full history is always recoverable
- **Unified** — single stream, multiple projections
- **Structurally interpretable** — behavior has geometry, not just logs

This positions the system closer to:

- **Git for operational reality** — full replay of factory behavior or agent work history
- **CPU scheduler for human and machine work** — intensity = load, fragmentation = context switching cost, stability = execution coherence
- **Single observability layer** — replacing logs + metrics + task systems + dashboards with one event stream and multiple derived views

The real innovation is not the individual layers (Dey, Frick, Bundy are each well-precedented). It is the reduction of all of them to a single time-indexed event tape with deterministic replay — a **temporal abstraction layer over operational systems** that is both replayable and geometrically interpretable.

---

## Part XIV — Implementation Stack

### Minimal Viable Implementation (v1)

Required components:
1. Stream writer (append-only log management)
2. Replay engine (pure function, deterministic)
3. Frick boundary reducer
4. Bundy interval builder
5. Taskwarrior wrapper (hook → B/F events)
6. Basic SMM (idle + command + task observers)

Optional for v1:
- Cooper renderer
- Dey cache
- Regeneration model
- Timewarrior sync adapter

### Reference Technology Stack

- **Stream processor**: Rust or Go (performance, correctness guarantees)
- **Event store**: SQLite for local; TimescaleDB for time-series at scale
- **WebSocket server**: real-time stream delivery to browser UI
- **Frontend renderer**: Canvas or WebGL for Cooper geometry; SVG for simpler views
- **CLI integration**: Workwarrior service layer as compiler front-end

### Encoding Optimizations (for high-frequency / industrial use)

**Delta compression:**

```
ΔD +0.01 +0.01 -0.01
```

Store differences rather than full samples for consecutive Dey readings.

**Bit-packed opcodes:**

```
F=0001  D=0010  B=0011  T=0100
```

Enables sensor-level ingestion at high frequency.

**Time quantization:**

```
t = delta from previous event
```

Streaming compression analogous to hardware telemetry event logs.

### The Daemon / Distributed Decision

The most consequential architecture decision: whether WWSS runs as a **daemon (central kernel)** or as a **distributed append-only log with local replicas**. The former fits personal and small-team use. The latter enables industrial-scale and multi-agent deployments where the stream itself becomes the coordination medium across machines.

---

## Summary: One-Line Definitions

**The stream format:**  
> A time-indexed instruction tape of five-token events where opcodes encode continuous signals (D), state transitions (F), semantic labels (T), identity bindings (B), and system events (S).

**The replay engine:**  
> A pure, deterministic function over an ordered event stream that produces full system state — intervals, signals, task mappings, geometric projections, and adaptive state — from a single source of truth.

**Cooper:**  
> A state-space embedding of work behavior in polar coordinates where time becomes angle, intensity becomes radius, and fragmentation becomes geometric deformation — enabling anomaly detection, scheduling optimization, and multi-agent coordination through field geometry rather than threshold rules.

**The system:**  
> Workwarrior Stream Service is a deterministic, append-only event kernel that ingests human, system, and agent actions and exposes all operational state — tasks, time, signals, and structure — as replayable projections over a unified temporal stream, making it suitable for personal productivity, agentic systems, and industrial operational observability.

---

*Synthesized from 11 source documents in `times-research/`. Source documents cover: canonical stream format and briefing, historical lineage (stream bg), first-class service design, advanced telemetry treatment, agentic and industrial relevance, human-agent collaboration model, Cooper computational uses, multiple Cooper visualizations, ASCII visualization layer, CLI and encoding architecture, data format specifications.*
