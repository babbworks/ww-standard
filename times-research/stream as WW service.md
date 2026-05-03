## Workwarrior Stream Service (WWSS) — First-Class System Design

This is how you turn the idea into an actual **core service inside Workwarrior**, rather than a library or add-on.

The key architectural decision:

> WWSS is not a feature. It is the runtime substrate everything else depends on.

---

# 1. System position

### Replace this mental model:

- Taskwarrior = core system
    
- Timewarrior = timing layer
    
- scripts = glue
    

### With this:

```text
WWSS (Stream Service)
   ↓
Task adapter
   ↓
Time adapter
   ↓
Agent adapters
   ↓
CLI / UI / APIs
```

Everything depends on WWSS, not the reverse.

---

# 2. Core service responsibilities

WWSS has exactly three responsibilities:

## 1. Ingest

Accept events from:

- CLI commands
    
- Taskwarrior hooks
    
- Timewarrior hooks
    
- agent processes
    
- system observers (SMM)
    

---

## 2. Append-only storage

Write to:

```text
stream.log
```

No mutation. No updates. No deletes.

---

## 3. Replay + projection API

Provide deterministic reconstruction:

- Dey (signal)
    
- Frick (events)
    
- Bundy (intervals)
    
- Task bindings
    
- Cooper rendering
    
- regeneration state
    

---

# 3. Internal architecture

## A. Ingestion layer

```text
[CLI] [Taskwarrior] [Agents] [SMM]
            ↓
        WWSS ingest
```

Responsibilities:

- normalize inputs
    
- convert to WW events
    
- assign timestamps
    
- enforce schema
    

---

## B. Stream writer (kernel core)

Strict append-only log:

```text
write(event) → stream.log
```

Constraints:

- atomic append
    
- ordered timestamps
    
- no overwrite
    
- idempotent retry handling
    

---

## C. Event model (canonical)

```text
<t> <op> <a> <b> <c>
```

Ops:

- D = signal sample
    
- F = state transition
    
- T = semantic annotation
    
- B = binding
    
- S = system/regeneration
    

---

# 4. Adapter layer (critical for Workwarrior integration)

WWSS does NOT call Taskwarrior directly in core logic.

Instead:

## Task adapter

```text
task start → emits:
F START
B task:id
D intensity update
```

## Time adapter

```text
F START → Timewarrior start
F STOP → Timewarrior stop
```

## Agent adapter

Agents write directly:

```text
T annotation
D inferred state
F inferred transitions
```

---

# 5. Replay engine (read side of service)

This is the read API:

```text
GET /replay?from=...&to=...
```

Returns:

- reconstructed state machine
    
- Dey signal
    
- Bundy intervals
    
- task mapping
    
- Cooper projection
    

Implementation rule:

> replay must be pure and deterministic

---

# 6. Projection API (core product surface)

WWSS exposes multiple views:

## Signal view

```text
GET /view/dey
```

## Interval view

```text
GET /view/bundy
```

## Task view

```text
GET /view/tasks
```

## Geometry view

```text
GET /view/cooper
```

These are all **derived endpoints**, not stored data.

---

# 7. Stream Monitoring Manager (SMM as subsystem)

SMM runs inside WWSS:

```text
SMM → observer plugins → normalized WW events
```

Responsibilities:

- sampling system state
    
- detecting idle/activity
    
- generating D events
    
- inferring F transitions
    

Important constraint:

> SMM can only WRITE events, never modify past ones

---

# 8. Concurrency model (important for agents + humans)

WWSS is:

### append-only + multi-writer

So you need:

## Option A (simple)

- single writer process
    
- all inputs serialized
    

## Option B (scalable)

- queue-based ingestion
    

```text
CLI / agents / hooks → message queue → WWSS writer
```

Guarantee:

- total ordering per timestamp bucket
    

---

# 9. Identity model (Taskwarrior integration)

Taskwarrior IDs are external identifiers.

WWSS introduces:

```text
binding = (taskwarrior_id ↔ stream interval)
```

Stored as:

```text
B task:2
```

So:

- Taskwarrior = identity layer
    
- WWSS = temporal layer
    

---

# 10. Storage model

## Primary store

```text
stream.log
```

## Optional snapshots

```text
snapshot_YYYYMMDD.json
```

Snapshots are:

- optional
    
- derived
    
- disposable
    

---

# 11. Service API surface (minimal viable)

### Write API

```bash
POST /event
```

### Ingest CLI

```bash
ww emit D 0.5 0.7 0.2
ww task 2 start
```

### Replay API

```bash
ww replay --from 08:00
```

### View API

```bash
ww view cooper
```

---

# 12. Failure model (critical for correctness)

WWSS must tolerate:

- missing agent input
    
- delayed Taskwarrior hooks
    
- duplicate events
    
- partial ingestion
    

Rules:

- replay fixes inconsistency
    
- stream is never corrected, only appended
    
- contradictions become observable signal (fragmentation)
    

---

# 13. System invariants

These must always hold:

### 1. Append-only truth

No modification of history.

### 2. Deterministic replay

Same stream → same state.

### 3. No hidden state

Everything reconstructable.

### 4. Time-first model

All meaning is time-indexed.

---

# 14. What makes this a “first-class service”

WWSS becomes first-class when:

- all tools write into it
    
- no tool bypasses it
    
- all views derive from it
    
- Taskwarrior is no longer authoritative, only contributory
    

So:

> WWSS becomes the kernel of Workwarrior

---

# 15. One-line engineering definition

> Workwarrior Stream Service is a deterministic, append-only event kernel that ingests human, system, and agent actions and exposes all operational state (tasks, time, signals, and structure) as replayable projections over a unified temporal stream.

---

If you want the next step, the real implementation decision is:

> whether WWSS runs as a daemon (central kernel) or as a distributed append-only log with local replicas per machine/agent

That choice determines whether this becomes a personal system or an industrial-scale platform.