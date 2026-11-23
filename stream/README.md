# STREAM

One append-only log of acts by people, machines and AI agents, with what each
act consumed. Balances, boards, traces and the applications themselves are
functions of that log.

This is the first Rust implementation. It is a self-contained Cargo workspace,
so it can move to its own repository unchanged.

## Kernel

```
kernel = sha256:fed5b36e80bad10c0545aa82ad5996175cebeb56e0af166162ddb5ff0c09202b
```

[`spec/kernel.descriptor`](spec/kernel.descriptor) is the kernel: about one
page, and its hash is its identity. Every log starts with
`STREAM/1 kernel=sha256:<hash>` and is read under exactly that kernel.
[`spec/conformance/`](spec/conformance/) holds sample logs that every
implementation must judge the same way.

## Crates

| Crate | What it does |
| --- | --- |
| `stream-kernel` | Entry grammar, canonical JSON, exact decimals, the 64 verbs and the phase (one type byte), addresses, hashing, seals. `no_std`: builds for servers, `wasm32-unknown-unknown` and `thumbv7em-none-eabihf`. Depends only on `sha2`. |
| `stream-log` | Append-only segments in any `Store` (a directory, or memory), receipts, retries by `(rec, ps)`, journaled rejections, authority from genesis roots and grants, the Register, pack adoption, seals, verification, capture gaps. |
| `stream-books` | Built-in views as pure folds: balances from explicit legs and implied `use`, Close by hash, span pairing, attribution with remainders, utilisation and parallelism. |
| `stream-codec` | Run form: shared context once, then deltas. Decodes byte for byte to canonical lines. |
| `stream-cli` | The `stream` command. |
| `stream-wasm` | The log's rules for the browser: the same `Log` over memory, behind one JSON entry point. Used by [`web/`](web/). |

## Quick start

```sh
cargo build --release
S=target/release/stream
$S init ./demo --log demo --root actor:founder --tz Europe/London
$S adopt ./demo --as actor:founder packs/si-units@1.pack
$S adopt ./demo --as actor:founder packs/agent-ops@1.pack
$S append ./demo --as actor:founder '1752300002 GRANT host:build-01 {"for":"agent:*","may":"record","to":"host:build-01"}'
echo '1752300000 R:ACT( task-42 {"actor":"agent:claude-07","ps":1,"run":"k42"}' | $S append ./demo --as host:build-01
echo '1752300060 R:ACT) task-42 {"actor":"agent:claude-07","pool":"host:build-01","ps":2,"run":"k42","use":{"J":"610","xs":"41.0"}}' | $S append ./demo --as host:build-01
$S seal ./demo
$S close ./demo --as actor:founder --book -
$S view ./demo balances
$S verify ./demo
```

## An entry

```
<time> [P:]VERB[phase] <object> <canonical JSON payload>
1752300000.1 R:QUERY( task-42 {"actor":"agent:claude-07","parent":"k42","ps":2,"rec":"host:build-01","run":"k42.m1"}
```

The phase marks are `(` begin, `)` end, `*` sample, and nothing for an
instant. The profile is an optional uppercase letter. The journal sets `rec`
from the authenticated submitter.

## Decisions made in code

- **Unique verb names.** The Codebook uses ASSIGN for both 0x05 and 0x20, and
  DELEGATE for both 0x0A and 0x37. Here 0x05 is `ASSIGN_CLASS` and 0x37 is
  `DELEGATE_EVENT`. Mapping to ZAKO is by code.
- **Four fields, not five.** Verb and phase replace v1's OP and action. v1
  logs come in through an importer, not unchanged.
- **Integers only in canonical JSON.** Quantities are decimal strings, and `~`
  marks an estimate.
- **Rejections are ACK entries carrying `refused`.** A retry gets the same
  answer, and capture gaps count them as received.
- **Implied postings.** An end-phase entry with `use` and `pool` moves each
  quantity from the pool to the activity. Nothing extra is written.
- **Amending a declaration** carries `set`, which is merged into the Register entry.
- **Authority.** CREATE, BIND, GRANT, REVOKE and COMMIT need a root or an admin
  grant. Recording for another actor needs a `record` grant whose `for`
  pattern matches.

## Status

- **Built and tested:** kernel, log, Register, packs, Books, run form, CLI,
  and the browser engine. There are 21 tests, including the conformance
  cases. Clippy is clean.
- **In the browser:** [`web/`](web/) has the Now and Act screens working
  locally, with the simulated agent. See its README.
- **Next:**
  1. A WebAssembly view host (wasmtime, no imports), with `stream-books`
     moving onto it.
  2. Positional payloads and template rows in the run form. The trial reaches
     about 28 times smaller than canonical on meter data; this version reaches
     about 3.2 times.
  3. Packed binary and control-character framing.
  4. A declarative view language.
  5. The web shell.
  6. Importers for Workwarrior `stream.log` v1, Taskwarrior, Timewarrior and hledger.
  7. A C interface for ZAKO devices.
  8. The Python trial as a second implementation, run against the conformance logs.
- **Limits:**
  - One writer per log directory.
  - The full Register and index are held in memory.
  - Closes are verified by recomputing from the start of the log, not from a
    checkpoint.
