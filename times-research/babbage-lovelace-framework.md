# WWSS — Babbage-Lovelace Computational Framework

**Workwarrior Stream Service: Analytical Engine as Architectural Model**

---

## Preamble

This document is not decoration. The claim is structural: the Workwarrior Stream Service
is architecturally isomorphic to Charles Babbage's Analytical Engine as described in his
mechanical drawings and unpublished manuscripts (1837–1871), and enriched by Ada Lovelace's
Notes A–G (1843). The mapping is not metaphorical — the components correspond
one-to-one, and this correspondence produces new design decisions that a purely modern
framing would not yield.

The Difference Engine No. 2 (designed 1847–1849, built by the Science Museum London in
1991) is additionally referenced as the substrate for WWSS's signal computation layers
(Dey, Felt, Grant). The DE No. 2's method of finite differences — advancing a polynomial
curve through pure addition — is the correct algorithm for smoothed event-density signals.

Babbage and Lovelace never used these words together, but what they designed was:

> An append-only symbolic record (Store) processed by a deterministic instruction machine
> (Mill), sequenced by a program (Barrel), producing derived outputs (Print) — with the
> observation (Lovelace) that the symbolic record need not be numeric.

This is the Workwarrior Stream Service.

---

## Part I — The Analytical Engine Mapped to WWSS

### 1.1 Component correspondence

| Analytical Engine | WWSS Component | Notes |
|---|---|---|
| Store | `stream.log` + derived cache | append-only; non-destructive read |
| Mill | Lens computation engine | ingress → operation → egress |
| Barrel | Instruction scheduler | sequences lens programs; enables cron |
| Operation cards | Lens names in a program | what operation to perform |
| Variable cards | Field selectors + filters | which Store addresses to read from |
| Number cards | `stream emit` | literal values injected into Store |
| Combinatorial cards | Conditional lens dispatch | skip Bundy if no B events |
| Anticipatory carriage | Time-range branching | `--from` / `--to` filter at replay |
| Print mechanism | Codecs (text, JSON, TUI) | output of Mill computation |
| Curves drawing device | Cooper ASCII / ratatui | graphical output of geometric field |

### 1.2 The unifying claim

Lovelace, Note A: *"The Analytical Engine does not occupy common ground with mere
calculating machines... it holds a position wholly its own."*

She means: it is not a calculator that computes a fixed function. It is a general engine
whose function is specified by the program loaded into it. This is precisely the WWSS lens
model: the same Store (stream.log) produces entirely different outputs depending on which
lens (program) is loaded into the Mill.

Lovelace, Note A (on the limits of the engine): *"The Analytical Engine has no power of
originating anything. It can only do what we know how to order it to perform."*

This is the Pacioli guarantee expressed as an epistemological principle: the Stream cannot
produce information not already in the log. All lenses are deterministic derivations from
the record. No lens adds new facts. The log is the only source of truth.

---

## Part II — The Store

### 2.1 Babbage's design

The Analytical Engine Store held 1,000 variables of 40 digits each. Each variable occupied
a column of number wheels — a vertical axis of digit positions. Variables were addressed by
column number (V₀, V₁, … V₉₉₉). The wheels were visible to the operator: the current
state of the Store could be read directly without any operation. Reading a variable
(Babbage called it "giving out") did not erase its value — the Store retained it for
subsequent use. This non-destructive readout is architecturally critical.

### 2.2 WWSS Store: Row × Level × Bin

WWSS formalizes the Store as a three-dimensional address space:

```
Store[row][level][bin]
```

**Row** — the time axis. Every event is keyed by unix timestamp, bucketed to a resolution
appropriate to the computation (1-second for raw events, 3600-second for Hollerith matrix).
Rows are sorted ascending; `sort -n -k1` is the only required primitive. The Row is the
primary index — time-first ordering is the Hollerith encoding's core invariant.

**Level** — the computational tier. Not all events are at the same level of derivation:

| Level | Tier | Contents |
|---|---|---|
| 0 | Substrate | Raw Hollerith event lines (stream.log); Pacioli guarantee active |
| A | Event | Burroughs accumulation, Baldwin state deltas |
| B | Structure | Frick transitions, Bundy intervals (derived, not stored) |
| C | Signal | Grant metrics, Felt density, Dey continuous signal |
| D | Field | Cooper geometric projection |

The Level dimension enables filtered replay: `replay_load(from, to, level=B)` returns only
structural events, not raw substrate noise. A lens operating at Level C has access to all
Level 0–B events as inputs.

**Bin** — the field position within an event record. This is the Hollerith encoding
expressed as an address: the five positional columns of every stream.log line.

| Bin | Field | Type | Meaning |
|---|---|---|---|
| 0 | `ts` | u64 | Unix timestamp — the Row key |
| 1 | `op` | Op | Single uppercase letter opcode |
| 2 | `action` | &str | Verb: add, start, stop, done, write, post… |
| 3 | `object` | &str | UUID or 12-char hash identifier |
| 4 | `ctx` | Option<Ctx> | Minified JSON occurrence field |

Bin addressing is what Babbage's digit wheels are: the positional slots within a variable's
column. The Hollerith 5-column format is Babbage bin addressing applied to temporal records.

### 2.3 Non-destructive readout

Babbage's Store retained variable values after "giving out." The WWSS Store (stream.log)
is never modified after append — every replay reads the same record. This is not merely a
Pacioli principle; it is Babbage's non-destructive readout at file granularity. The
operator (user, lens, agent) can read the Store as many times as needed; the Store is
unchanged.

### 2.4 Rust types

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier { Substrate, Event, Structure, Signal, Field }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldBin { Ts, Op, Action, Object, Ctx }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op { T, F, B, D, H, S, A }

#[derive(Debug, Clone)]
struct Event {
    ts:     u64,
    op:     Op,
    action: String,
    object: String,
    ctx:    Option<Ctx>,
}

#[derive(Debug, Clone)]
struct Ctx {
    src:  String,
    prof: String,
    proj: String,
    tags: Vec<String>,
    name: Option<String>,
}

struct StoreAddress {
    row:   u64,      // timestamp bucket
    level: Tier,
    bin:   FieldBin,
}
```

The `Op` enum is the key correctness guarantee: an event with an invalid opcode cannot
exist in the type system. The Store enforces the Hollerith contract at compile time.

---

## Part III — The Mill

### 3.1 Babbage's design

The Mill was the Analytical Engine's arithmetic unit. It had two ingress axes (input
columns brought from the Store) and one egress axis (the result, returned to the Store or
sent to the Print mechanism). Before operating, the Mill was "primed" — values loaded
onto the ingress axes. The Mill's accumulator (Babbage called it the "anvil") held
intermediate results across multiple operations. The carry mechanism propagated across all
digit positions simultaneously — Babbage called his anticipatory carriage mechanism the
key innovation over simpler designs.

Babbage: *"The whole of the Arithmetical part of the Difference Engine is contained in
the first and simplest of all mechanical movements, viz., the carrying of the tens."*

### 3.2 WWSS Mill: the lens computation model

The Mill in WWSS is the lens computation engine. Its contract:

- **Ingress**: events from the Store via `replay_load()` — the ingress axes
- **Operation**: the lens function — the Mill's current program
- **Egress**: derived output (terminal rows, JSON, TUI frames) — the egress axis or Print

The Mill's "primed" state maps to a lens's initialization phase (reading the first event,
establishing working variable state). The "anvil" maps to lens intermediate state:
Bundy's `open_intervals: HashMap<String, u64>`, Pacioli's `counts: HashMap<String, u64>`.

The carry mechanism maps to the Pacioli credit/debit accumulation — running totals that
propagate across events. Each new event "carries" the running count forward, exactly as
Babbage's tens-carry propagated across digit wheels.

### 3.3 Lovelace: working variables and result variables

Lovelace, Note G (Bernoulli numbers algorithm): She distinguishes **working variables**
(V₁–V₆: intermediate values updated each cycle) from **result variables** (V₇: the final
output accumulated across all cycles). This distinction is exact in lens design:

- **Working variables** = lens mutable state (open intervals, running counts, last bucket)
- **Result variables** = accumulated output rows written to the egress axis (stdout)

A well-designed lens has a clear boundary between its working variables (private, mutable,
destroyed after lens_run) and its result variables (the stream of output lines it emits
to stdout). This boundary is the Lovelace lens contract.

### 3.4 Rust trait

```rust
trait Lens {
    fn describe(&self) -> &str;
    fn run(&self, events: impl Iterator<Item = Event>, out: &mut impl Write) -> Result<()>;
}
```

Working variables live inside `run()` as local state. Result variables are written to
`out`. The Mill runs a `Lens` by calling `run()` with events from the Store iterator.
No lens implementation reaches outside this boundary.

---

## Part IV — The Barrel

### 4.1 Babbage's design

The Barrel was a rotating cylinder covered with studs and levers. As the Barrel turned,
its studs triggered the Mill's operations in sequence — specifying which arithmetic
operation to perform at each step. Different Barrels encoded different programs. The
Barrel could reverse ("back") to repeat a sequence — this is loop control: Lovelace's
"cycle" was a Barrel that repeated a group of operations before advancing.

Multiple Barrels could be loaded in sequence for complex programs. The Barrel was the
only mechanism that controlled the sequence of Mill operations — it was the stored
program.

### 4.2 WWSS Barrel: the instruction scheduler

The Barrel in WWSS is a named, saved program: a sequence of lens operations with optional
variable cards (filters) and a schedule (cron expression or one-shot).

```
barrel: daily-review
  operations:
    - lens: burroughs    vars: [--from yesterday, --to today]
    - lens: pacioli      vars: [--from yesterday, --to today]
    - lens: bundy        vars: [--format ascii]
  schedule: "0 8 * * *"   # every morning at 08:00
```

"Turning the Barrel" = executing the named program. "Backing the Barrel" = re-running
with an earlier time range (historical replay). Multiple Barrels = saved configurations
for different purposes (daily review, weekly sprint summary, project deep-dive).

The Barrel is what elevates `stream` from a query tool to a **computational substrate**:
users define programs, the Barrel executes them on schedule, the Store accumulates the
record of what was computed and when.

### 4.3 Barrel and cron

`stream barrel run daily-review` executes one turn.  
`stream barrel schedule daily-review` activates the cron trigger.  
`stream barrel list` shows all defined barrels and their next fire time.  
`stream barrel log` shows the S-event record of past barrel executions (recorded to stream.log).

Every barrel execution emits an `S barrel.run <barrel-name> {...}` event to stream.log —
the Pacioli guarantee means that the history of what programs were run, when, and on what
data, is itself part of the permanent record.

### 4.4 Rust types

```rust
struct VariableCard {
    from:    Option<u64>,
    to:      Option<u64>,
    filter:  Option<FilterExpr>,
    format:  CodecKind,
}

struct OperationCard {
    lens:  LensKind,
    vars:  VariableCard,
}

struct Barrel {
    name:       String,
    operations: Vec<OperationCard>,
    schedule:   Option<CronExpr>,
}
```

---

## Part V — The Difference Engine No. 2 and Signal Computation

### 5.1 Babbage's method

The Difference Engine (No. 1, 1820s; No. 2, 1847–1849) computed polynomial values
using the method of finite differences. Given a polynomial f(t), the k-th order difference
Δᵏf is eventually constant. The engine initializes a table of differences and advances
by pure addition — no multiplication required.

For a second-order polynomial (quadratic), two registers suffice:
- Δ²f = constant (loaded once)
- Δf = advances by adding Δ²f each step
- f(t) = advances by adding Δf each step

This is how the Difference Engine "draws a curve" — not by evaluating f(t) for each t,
but by accumulating differences forward from an initial state.

Babbage, on DE No. 2: *"The method of differences is applicable to all polynomial
functions... and by taking sufficiently many orders of differences, it may be made to
approximate any tabulated function to any desired degree of accuracy."*

### 5.2 WWSS signal computation

The Dey, Felt, and Grant lenses are difference engines operating on event density:

Let **f(t)** = event count per time bucket (raw activity density from stream.log).

```
Felt(t)  = Δf(t)  = f(t) - f(t-1)          [first difference  = rate of change]
Grant(t) = Δ²f(t) = Δf(t) - Δf(t-1)        [second difference = acceleration]
Dey(t)   = smoothed f(t) via k-th order      [continuous signal = smooth curve]
```

The Dey signal uses Babbage's advance method: a difference table of order k is initialized
from the first k time buckets, then advanced forward through pure addition. This produces
a smooth continuous signal from sparse, noisy event data — the "continuous dial" Dey
described in his time-recording machines, now computed via Babbage's polynomial method.

**Difference table structure:**

```rust
struct DifferenceTable {
    order:  usize,
    regs:   Vec<f64>,    // registers[0] = f(t), [1] = Δf, [2] = Δ²f, ...
}

impl DifferenceTable {
    fn advance(&mut self) -> f64 {
        // Babbage's addition cascade: accumulate from highest order down
        for i in (1..self.regs.len()).rev() {
            self.regs[i - 1] += self.regs[i];
        }
        self.regs[0]
    }
}
```

`advance()` is exactly one turn of the Difference Engine No. 2's mechanism: the constant
highest-order difference carries down through all registers, advancing the function by
one step. Each call returns the next smooth value.

### 5.3 DE No. 2 and the Print mechanism

The Difference Engine No. 2 included a direct stereotype printing mechanism — it could
produce printing plates from computed values, bypassing handwritten transcription. This
maps directly to WWSS codecs: the Mill computes, the Print mechanism produces the artifact.

In DE No. 2 terms:
- **Text codec** = the typeset printer (human-readable columns)
- **JSON codec** = the sterotype plate (machine-importable structured artifact)
- **TUI codec** = the curves drawing mechanism (graphical, interactive)

---

## Part VI — Lovelace's Extensions

### 6.1 The engine operates on any symbolic system

Lovelace, Note A: *"Supposing, for instance, that the fundamental relations of pitched
sounds in the science of harmony and of musical composition were susceptible of such
expression and adaptations, the engine might compose elaborate and scientific pieces of
music of any degree of complexity or extent."*

She makes the claim that the engine is not inherently numeric — it can operate on any
system of symbols as long as the relations between them can be expressed as operations.
This is the theoretical foundation for WWSS operating on symbolic event records rather
than numbers.

Stream events are symbols: `T add <uuid>`, `B start <hash>`, `A write <hash>`. The Mill
(lens) operates on these symbols — counting, grouping, sequencing, projecting — without
treating them as numbers. The Hollerith matrix is a symbolic grid. The Frick transition
graph is a symbolic state machine. The Cooper field converts symbolic events to geometric
coordinates. None of these are arithmetic in Babbage's sense; all are symbolic operations
in Lovelace's sense.

**Implication for WWSS**: the Op enum and Ctx schema are the symbolic alphabet. Lenses
are symbolic programs, not arithmetic programs. The Mill is not an adding machine — it
is a symbol processor. Rust's pattern matching on `enum Op` is the natural implementation
of Lovelace's symbolic computation.

```rust
match event.op {
    Op::B => handle_bundy_boundary(event),
    Op::T => handle_task_event(event),
    Op::A => handle_annotation(event),
    Op::D => handle_dey_sample(event),
    _ => skip(event),
}
```

### 6.2 Cycles — the replay loop and monitor mode

Lovelace, Note C: She introduces the term **cycle** for a repeating group of operations.
She describes backing the operation cards to repeat a set: *"the engine will work the
same set of operations over and over again, for as many times as may be required."*

She also describes **nested cycles**: an inner cycle (repeating a sub-computation) within
an outer cycle (iterating over the full dataset). Her Bernoulli algorithm has exactly this
structure.

WWSS cycle types:

```rust
enum CycleMode {
    Single,                            // one-shot replay: ww stream view
    Bounded { from: u64, to: u64 },   // time-sliced replay: --from --to
    Perpetual { interval_ms: u64 },   // stream monitor: continuous loop
}
```

The `Perpetual` mode is Lovelace's "indefinitely continued cycle" — the Barrel keeps
turning, each turn reading new events appended to the Store since the last cycle.
`stream monitor` is a perpetual cycle advancing over the Store's growing tail.

### 6.3 Approximation and the Dey iteration

Lovelace, Note D: The engine can perform **successive approximation** — running multiple
cycles, each refining a result toward greater accuracy. She describes this in the context
of solving equations by iteration, where each cycle improves the estimate.

The Dey signal is computed by successive approximation: the difference table is advanced
one step per time bucket, producing a progressively smoother estimate of the underlying
activity curve. More events → better-initialized difference table → more accurate smooth.
Sparse data (few events) → coarser approximation. Dense data → fine approximation.

This is not a limitation of the design but a correct implementation of Lovelace's principle:
the engine's approximation quality is bounded by the information in the Store. The Pacioli
guarantee (no information added by lenses) is preserved.

### 6.4 The engine cannot originate

Lovelace, Note A: *"The Analytical Engine has no power of originating anything. It can
only do what we know how to order it to perform."*

This is the definitive statement of deterministic replay. Applied to WWSS:
- No lens can add events to stream.log (only `stream emit` and adapters can)
- Every derived view (Bundy interval, Dey signal, Cooper field) is a deterministic function
  of stream.log contents and the lens program
- The same stream.log + same lens = identical output, always
- Agent actions (emit events) expand the Store; they do not modify the program

---

## Part VII — The Human Memory Interface

### 7.1 Babbage's visible Store

Babbage designed the Store with the number wheels visible to the operator. An operator
standing before the Analytical Engine could read the current value of any variable by
looking at its column. The Store was not opaque — its state was observable at a glance.

This design principle — the machine's internal state should be directly legible to a
human observer — is the architectural requirement for WWSS's TUI interface.

### 7.2 Row × Level × Bin navigation

The TUI implements Babbage's visible Store as a three-dimensional navigable space:

```
┌─────────────────────────────────────────────────────────────────┐
│ WWSS Store Viewer    Level: Structure (B)    Bins: 0-4          │
├─────────────────────────────────────────────────────────────────┤
│ ROW (time)          OP   ACTION   OBJECT        CTX             │
│ 2026-04-13 13:00  → B    start    ac662f4c      proj:ww-dev     │
│ 2026-04-13 13:40    B    stop     ac662f4c      proj:ww-dev     │
│ 2026-04-21 14:00    B    start    12278ae4      proj:browser    │
│ 2026-04-22 09:00    B    stop     12278ae4      proj:browser    │
├─────────────────────────────────────────────────────────────────┤
│ [r] change row range  [l] change level  [b] expand bin          │
│ [Enter] load selected row into Mill  [m] run barrel             │
└─────────────────────────────────────────────────────────────────┘
```

Navigation:
- **Row scroll** (↑/↓): advance through time
- **Level switch** (l): filter to Substrate / Event / Structure / Signal / Field
- **Bin expand** (b): drill into a specific field (e.g., expand Ctx JSON in full)
- **Mill load** (Enter): select a row set and run a lens against it — "giving out" a
  variable from the Store to the Mill
- **Barrel run** (m): execute a named Barrel on the current selection

The "human memory-compatible" capability the user described is this: operators navigate
the Store as a spatial object (rows = time scrolling, levels = depth/tier, bins = field
width), and arbitrarily invoke computation (Mill) or scheduled programs (Barrel) on their
current selection. This matches how human memory works spatially — locate by approximate
time and context, then drill.

### 7.3 Recall and action

"Recall" = navigate to a Store address, load the events into the Mill, run a lens.
"Action" = emit new events based on what the Mill produces (a new annotation, a task
modification, a ledger entry). The Store accumulates the action as a new append — the
Pacioli guarantee preserves the full history of both the original events and the actions
they triggered.

This is Babbage's full loop: Store → Mill → Print → (human decision) → new Number cards
→ Store (appended). WWSS closes this loop via `stream emit` after any lens view.

---

## Part VIII — Rust Module Structure

```
wwss/
  src/
    main.rs              // CLI entry point (clap)
    lib.rs
    store/
      mod.rs             // StreamLog: open, append, iter
      address.rs         // StoreAddress, Tier, FieldBin
      event.rs           // Event, Op, Ctx (serde Deserialize)
      pacioli.rs         // append-only guarantees, file locking
    mill/
      mod.rs             // Mill: run(lens, events) → output
      lens.rs            // Lens trait: describe() + run()
      cycle.rs           // CycleMode: Single | Bounded | Perpetual
      working.rs         // WorkingVars: type-safe lens state
    barrel/
      mod.rs             // Barrel: name, operations, schedule
      program.rs         // OperationCard, VariableCard
      scheduler.rs       // cron integration, barrel log emission
    adapters/
      mod.rs
      task.rs            // TaskWarrior → Event stream
      timew.rs           // TimeWarrior → B events
      jrnl.rs            // jrnl → A events
      ledger.rs          // hledger → T post events
    lenses/
      mod.rs             // registry: name → Box<dyn Lens>
      burroughs.rs       // raw log view (Tier A)
      bundy.rs           // interval accumulation (Tier B)
      hollerith.rs       // matrix grid (Tier 0 encoding view)
      pacioli.rs         // running balance (Tier 0 substrate view)
      frick.rs           // transition graph (Tier B)
      baldwin.rs         // state mutation diff (Tier A)
      felt.rs            // density: Δf(t) (Tier C)
      grant.rs           // acceleration: Δ²f(t) (Tier C)
      dey.rs             // smooth signal: difference table (Tier C)
      cooper.rs          // geometric field (Tier D)
    signal/
      difference.rs      // DifferenceTable: Babbage DE No.2 advance()
      smooth.rs          // exponential moving average, interpolation
    codecs/
      mod.rs
      text.rs            // human-readable table
      json.rs            // serde_json array output
      tui.rs             // ratatui Store viewer (Row × Level × Bin)
    hooks/
      onadd.rs           // TaskWarrior on-add hook writer
      onmodify.rs        // TaskWarrior on-modify hook writer
```

### Key invariants enforced by the type system

1. `Op` enum: invalid opcodes cannot exist at compile time
2. `Ctx` struct with serde: malformed ctx fields fail at ingest, not at render
3. `Lens` trait: `run()` takes `Iterator<Item=Event>` and `Write` — cannot reach outside
4. `Pacioli::append()`: the only write path; returns `Err` if file would be truncated
5. `DifferenceTable::advance()`: pure addition, no allocation per step
6. `Barrel::execute()` always emits an `S barrel.run` event before returning

---

## Part IX — Times Service as Mill Orchestrator

### 9.1 The active-interval problem

Timew knows what project is being worked on RIGHT NOW. No other service has this
real-time knowledge. When `timew start project-alpha`, the active interval becomes a
context frame for all subsequent events until `timew stop`.

This makes Times the natural **Mill orchestrator**: it holds the current working variable
(`active_interval`) that all other adapters should annotate their events with.

### 9.2 Interval context enrichment

When the shell wrappers (`_stream_emit`) fire events from jrnl, task, or ledger, they
can query timew's current state:

```bash
_stream_active_interval() {
  timew get dom.active.tag.1 2>/dev/null || echo ""
}
```

If an interval is active, the `c` field gains `"interval":"<hash>"`:
```json
{"src":"jrnl","prof":"work","proj":"alpha","tags":["alpha"],"name":"Standup notes","interval":"ac662f4c3fa6"}
```

This creates a natural join in the Store: events during an active Bundy interval share
the interval hash. The Bundy lens can group annotated events by interval to show "what
happened during this block of time."

### 9.3 Times as the Barrel trigger

When `timew stop`, the closed interval can trigger a Barrel: compute the Bundy lens
for the just-completed interval and emit a summary annotation (A event) back to stream.log.
This is a closed-loop: time tracking → stream event → automatic lens computation → summary
event → back into stream.

```
timew stop
  → B stop event
  → barrel: interval-summary
    → bundy lens on completed interval
    → emit A summarize <interval-hash> {"duration":3600,"proj":"alpha","tasks":[...]}
  → stream.log grows with the summary
```

The summary annotation becomes queryable later: `stream view --op A --action summarize`
shows all closed-interval summaries. The Pacioli guarantee means nothing is lost; the full
interval record and the summary exist together permanently.

---

## Part X — What This Framework Enables That the Modern Framing Does Not

1. **Typed Store addressing** (Row × Level × Bin) gives the TUI a navigation model that
   is conceptually grounded, not just a UI decision. Users understand "I am at Row
   2026-04-13, Level B (Structure), Bin 3 (Object)" — each dimension has clear semantics.

2. **The Barrel** introduces saved, named, schedulable programs as a first-class WWSS
   concept — not just cron but a formal instruction concept with its own Store record
   (S events for barrel execution history).

3. **Difference Engine signal computation** (Dey, Felt, Grant) has a mathematically
   correct algorithm (Babbage's finite differences) rather than ad-hoc smoothing. The
   algorithm is simple (only addition), exactly as Babbage intended, and produces provably
   continuous polynomial approximations.

4. **Lovelace's symbolic operations** justify the Op enum design: stream events are not
   numbers, they are symbols in a symbolic system. The type system enforces the symbolic
   alphabet at compile time. Pattern matching on Op is not a convenience — it is the
   correct implementation of symbolic computation on the Mill.

5. **The human memory interface** (visible Store, Row × Level × Bin navigation) follows
   directly from Babbage's design principle that the Store should be legible without
   computation. The TUI is not a dashboard — it is an operator's view of the Store.

6. **Times as Mill orchestrator** with interval context enrichment closes the loop:
   the active timew interval becomes a working variable that all other adapters can
   reference, making temporal context an explicit part of every event's metadata rather
   than something reconstructed later.

---

## Appendix: Primary Sources

- Babbage, C. (1864). *Passages from the Life of a Philosopher.* Longman.
- Babbage, C. Unpublished Analytical Engine drawings and notebooks (1837–1871).
  Science Museum, London. Partially available via Science Museum Group Collection.
- Lovelace, A. (1843). Notes on Menabrea's "Sketch of the Analytical Engine." In:
  *Taylor's Scientific Memoirs*, Vol. III, pp. 666–731. (Notes A through G.)
- Swade, D. (2000). *The Difference Engine.* Viking. [On DE No. 2 and its construction.]
- Hyman, A. (1982). *Charles Babbage: Pioneer of the Computer.* Princeton University Press.

---

*This document is part of the WWSS specification. It informs the Rust module structure,
the TUI design, the signal computation algorithms, and the architectural vocabulary of
the service. It is a design document, not a historical survey.*
