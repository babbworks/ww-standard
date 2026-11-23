# STREAM · Now (web)

The Now and Act screens from the interface mock-up, working in a browser.
The page runs the same log rules as the `stream` CLI, compiled to
WebAssembly from `crates/stream-wasm`, and keeps the log in the browser.

```sh
rustup target add wasm32-unknown-unknown   # once
./web/build.sh                             # builds web/stream.wasm
python3 -m http.server -d web 8000         # then open http://localhost:8000
```

`web/stream.wasm` is committed, so the last step alone is enough if you
don't change the Rust code. The page must be served over http; opening
`index.html` as a file can't load the engine.

## What it does

- **A new log on first visit.** It has genesis, the `si-units@1` and
  `agent-ops@1` packs, four actors (the ops lead as root, an operator, the
  build host and an agent), the host's pool, its meter, and one grant: the
  host may record for `agent:*`. Every step is a real entry.
- **Recording as.** Choose who you are. Authority comes from the log: the
  host can record for the agent, the operator can't, and the root can do
  anything.
- **Act.** Eight verbs. A begin opens an interval and an instant is one
  entry. Each form shows the exact entry and a dry run of whether the log
  will accept it. A refused entry can still be sent, and the refusal is
  journaled.
- **Running and End.** Open intervals are paired by run. End asks for `use`,
  prefilled from the meter and from the child acts already closed, and each
  value can be marked as an estimate.
- **Simulated agent.** Play, Pause, Step and Speed drive a scripted run: a
  task, model calls, a tool call and meter samples every 8 seconds. A sample
  is sometimes lost, which shows as a real gap, and the operator tries to
  record for the agent, which is a real refusal.
- **Seal and Verify.** Seal the open segment, then recompute every seal and
  the chain.
- **Storage.** The segments and packs are kept in IndexedDB and reopened on
  the next visit. New log starts again. A saved log that can't be opened is
  kept under another key, not overwritten.

## Files

| File | What it is |
| --- | --- |
| `index.html`, `app.css` | The page |
| `app.js` | Screens and forms. Holds no state of its own: each change goes to the engine and the screen is rebuilt from the log. |
| `engine.js` | Loads the engine, and writes canonical JSON and entry lines |
| `setup.js` | The entries that start a new log |
| `sim.js` | The simulated agent |
| `storage.js` | IndexedDB for the log, and localStorage for who you are recording as |
| `stream.wasm` | The engine, built by `build.sh` |

Timeline, Books, Things, Attention and This app are not built yet.
