## WW Stream Encoding + CLI Subcommand Architecture — Detailed Speculative Design

This is a systems-level design of how the **stream encoding layer, CLI commands, and Workwarrior subsystems interlock** into a single coherent runtime.

The key principle:

> the CLI is not an interface to a system — it is a _compiler front-end for a time-structured event machine_

---

# 1. Core abstraction: WW Stream as instruction tape

Everything reduces to a single artifact:

```text
WW Stream = ordered instruction tape over time
```

Each line is:

```text
<t> <opcode> <a> <b> <c>
```

But semantically:

|Layer|Meaning|
|---|---|
|opcode|instruction type|
|payload|parameters|
|timestamp|execution ordering|

So the system behaves like:

> a time-indexed virtual machine program

---

# 2. CLI as compiler front-end

Every CLI command is a **macro-expansion into stream events**.

Example:

```bash
ww task 2 start
```

Expands into:

```text
F START
B task:2
D i=0.6 s=0.8 f=0.1
```

So CLI is:

> syntactic sugar over event emission

---

# 3. Command taxonomy (full model)

## 3.1 Identity commands (Taskwarrior layer)

```bash
ww task <id> start|stop|modify
```

Maps to:

- `F` transitions
    
- `B` bindings
    
- `T` annotations
    

---

## 3.2 Stream primitives (low-level control)

```bash
ww emit D 0.6 0.7 0.2
ww emit F START
ww emit T "note"
```

Direct insertion into stream.

Used by:

- agents
    
- SMM
    
- debugging tools
    

---

## 3.3 Observation commands (read-only projections)

```bash
ww view dey
ww view frick
ww view bundy
ww view cooper
```

All are:

> deterministic projections of the same stream

---

## 3.4 Replay engine commands

```bash
ww replay --from 08:00 --to 12:00
ww replay --task 2
```

Outputs reconstructed state:

- intervals
    
- signal curves
    
- geometry
    

---

## 3.5 Agent interface commands

```bash
ww agent run optimizer
ww agent handoff task:2
```

These generate:

```text
F HANDOFF agent=optimizer
B task:2
T context window
```

---

## 3.6 System commands (infrastructure layer)

```bash
ww sync taskwarrior
ww sync timewarrior
ww daemon start
ww smm start
```

These attach external systems into stream ingestion.

---

# 4. Stream encoding hierarchy

The encoding system is layered:

---

## Layer 1: raw event stream

Minimal form:

```text
t op a b c
```

---

## Layer 2: typed semantic decoding

```text
D → signal sample
F → state transition
B → identity binding
T → annotation
S → system event
```

---

## Layer 3: structured objects (derived)

- Dey → continuous field
    
- Frick → state machine
    
- Bundy → intervals
    
- Task graph → bindings
    
- Cooper → geometry
    

---

## Layer 4: projection outputs

- ASCII Cooper
    
- dashboards
    
- agent inputs
    
- anomaly detection
    

---

# 5. Interlinkages between CLI commands and stream semantics

## 5.1 Task lifecycle mapping

### Start

```bash
ww task 2 start
```

→ stream:

```text
F START
B task:2
D intensity bump
```

---

### Stop

```bash
ww task 2 stop
```

→ stream:

```text
F STOP
D decay sample
```

---

## 5.2 Context switching (implicit intelligence layer)

```bash
ww task 3 start
```

System infers:

```text
F SWITCH
B task:3
```

No explicit stop required.

This is important:

> switching is an inferred state transition, not a command pair

---

## 5.3 Annotation-driven evolution

```bash
ww note "machine unstable"
```

→ stream:

```text
T "machine unstable"
D fragmentation increase
```

Annotations feed:

- Dey adjustments
    
- Cooper deformation
    
- agent prioritization
    

---

# 6. Stream as shared execution substrate (critical concept)

All actors write into the same stream:

|Actor|Writes|
|---|---|
|human CLI|explicit commands|
|Taskwarrior|lifecycle events|
|Timewarrior|interval hints|
|SMM|inferred Dey samples|
|agents|predictions + actions|

So:

> the CLI is not the system — it is one writer into a shared temporal field

---

# 7. Command composition model (macro system)

Commands can nest:

```bash
ww task 2 analyze --agent
```

Expands to:

```text
F HANDOFF agent=analysis
B task:2
T "analysis requested"
D inferred load shift
```

This implies:

> CLI is a macro compiler for temporal programs

---

# 8. Encoding optimizations (speculative but important)

## 8.1 Delta compression

Instead of full samples:

```text
D 0.6 0.7 0.2
D 0.61 0.71 0.19
```

Store:

```text
ΔD +0.01 +0.01 -0.01
```

---

## 8.2 Bit-packed opcodes (advanced mode)

Map opcodes:

```text
F=0001
D=0010
B=0011
T=0100
```

Enables:

- high-frequency ingestion
    
- sensor-level integration
    

---

## 8.3 Time quantization

Instead of absolute timestamps:

```text
t = delta from previous event
```

This allows:

> streaming compression similar to event logs in hardware telemetry systems

---

# 9. CLI ↔ Stream ↔ Projection cycle

The full loop:

```text
CLI command
   ↓
event emission
   ↓
stream.log append
   ↓
replay engine
   ↓
derived structures (Dey/Frick/Bundy)
   ↓
Cooper projection
   ↓
CLI views + agent inputs
```

This creates a closed system:

> every action is both input and future computational substrate

---

# 10. Key architectural insight

The system behaves like:

### A time-based virtual machine

- program = stream
    
- execution = replay
    
- state = derived projections
    
- operators = CLI + agents
    

---

# 11. Why this is structurally powerful

Because it removes:

- session state
    
- tool boundaries
    
- static dashboards
    
- external memory systems
    

And replaces them with:

> a single evolving instruction tape over time

---

# 12. One-line synthesis

> The Workwarrior CLI functions as a macro-compiler for a unified temporal event stream, where every command expands into structured time-indexed instructions that are deterministically replayed into multiple derived representations (Dey, Frick, Bundy, Cooper), enabling both human and agent systems to operate over a shared, continuously evolving computational representation of work.