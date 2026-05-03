## Workwarrior as a Shared Human–Agent Operational Layer (Concurrency + Handoff Model)

### 1. Core idea

Workwarrior becomes a **shared, append-only temporal substrate** where both humans and agents operate on the same event stream without session boundaries.

Instead of:

- human sessions
    
- agent sessions
    
- project-specific contexts
    

you get:

> a continuous operational field where intent, execution, and observation are co-written into a single timeline

This removes the traditional “handoff boundary” between human work and agent work.

---

# 2. Structural shift: from session-based work to stream-based work

## Traditional model

- Human opens task/session
    
- Agent works in isolation
    
- Results are returned
    
- Context is reloaded or lost
    

Problem:

- repeated context switching
    
- duplicated reasoning
    
- brittle state transfer
    

---

## Workwarrior model

```text
human + agent → shared WW stream → shared state reconstruction
```

There is no session boundary.

Only:

> continuous event accumulation + deterministic replay

---

# 3. The key mechanism: handoffs become stream events

Instead of “passing a task to an agent”, you emit:

```text
F HANDOFF
B task:2
```

or richer:

```text
F HANDOFF agent=analysis-bot confidence=0.8
T "summarize factory downtime logs"
```

### Meaning:

- handoff is not an action
    
- it is a **state transition in the stream**
    

---

# 4. Agents as stream readers + stream writers

Agents do two things:

### A. Read

They replay a window:

```text
ww replay --from 08:00 --to now
```

They reconstruct:

- current Dey state (load, stability, fragmentation)
    
- active Bundy intervals
    
- task bindings
    
- unresolved Frick transitions
    

---

### B. Write

Agents append:

```text
510 D i=0.7 s=0.9 f=0.1
512 F SWITCH
512 B task:2
```

So agents are not external actors—they are:

> deterministic participants in the same event system as humans

---

# 5. Annotation as first-class execution fuel

A key expansion: annotations are not metadata—they are **execution triggers**.

## Human annotation

```text
520 T "this machine feels unstable"
```

## Agent interpretation

This becomes:

- signal bias for Dey sampling
    
- prioritization signal for task routing
    
- trigger for diagnostic sub-agent
    

---

## Agent annotation back into system

```text
521 T "vibration spike correlates with throughput drop"
```

Now the stream contains:

> human perception + machine inference in the same timeline

---

# 6. Multi-project execution without context switching

This is where the system becomes structurally important.

## Traditional problem

- each project requires:
    
    - loading context
        
    - switching mental model
        
    - losing prior state
        

---

## Workwarrior model

All projects coexist in one stream:

```text
B task:build-core
B task:debug-api
B task:factory-monitor
```

Each project is just:

> a binding into the same time field

---

## No context switching required

Because:

- Dey already represents global load
    
- Bundy separates execution intervals
    
- Frick marks transitions
    
- Task bindings attach meaning
    

So switching projects is not a “mode change”:

> it is just a new binding in the same continuous timeline

---

# 7. Handoff compression (critical efficiency gain)

Instead of:

- meetings
    
- status updates
    
- ticket transfers
    
- agent re-prompts
    

You get:

### Single event:

```text
F HANDOFF agent=optimizer task:2 context=stream:480-520
```

Agent resumes via replay:

```text
replay(stream[480:520])
```

No re-explaining required.

---

# 8. Cross-agent collaboration through shared state

Multiple agents can operate concurrently:

- analysis-agent writes Dey interpretations
    
- planner-agent writes TSK restructuring
    
- monitor-agent writes Frick events
    

Conflict is visible as:

- overlapping Bundy intervals
    
- fragmentation spikes in Dey
    
- contradictory annotations
    

So coordination becomes:

> visible in the state field, not negotiated externally

---

# 9. Human + agent symmetry

This is the key conceptual shift:

|Human|Agent|
|---|---|
|emits commands|emits inferred actions|
|annotates intent|annotates inference|
|starts tasks|continues tasks|
|interrupts flow|detects interruptions|

Both are equal producers of stream events.

---

# 10. Eliminating dedicated sessions

Sessions disappear because:

- state is not held in memory
    
- state is reconstructed from stream
    

So instead of:

```text
open session → work → close session
```

you have:

```text
continuous stream → replay at any point → act
```

Agents and humans both “enter” work by:

> selecting a time window, not opening a session

---

# 11. Operational benefit across many projects

Because everything is time-indexed:

- project A can be active at 09:00–10:30
    
- project B overlaps 10:00–12:00
    
- agent analysis spans both
    

Without interference.

This enables:

### parallel cognition model

- no exclusive locking of context
    
- no project isolation overhead
    
- shared temporal substrate
    

---

# 12. System-level outcome

The system becomes:

> a multi-actor, multi-project, time-indexed execution field where meaning is continuously attached to shared behavioral state

---

# 13. Key engineering insight

The breakthrough is not “agents + humans working together”.

It is:

> replacing session-based orchestration with a single replayable event stream that both humans and agents continuously write into and derive state from

---

# 14. One-line summary

> Workwarrior enables human and agent collaboration by replacing session-based task execution with a shared, append-only temporal event stream where handoffs, annotations, and multi-project work are expressed as first-class state transitions rather than external coordination mechanisms.