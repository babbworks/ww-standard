// The STREAM engine: the Rust log rules compiled to WebAssembly.
// One entry point, a JSON request in and a JSON answer out (see
// crates/stream-wasm/src/lib.rs for the operations).

export async function loadEngine(url = 'stream.wasm') {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`could not load ${url}: ${res.status}`);
  const { instance } = await WebAssembly.instantiate(await res.arrayBuffer(), {});
  const x = instance.exports;
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  return {
    call(req) {
      const bytes = enc.encode(JSON.stringify(req));
      const p = x.stream_alloc(bytes.length);
      new Uint8Array(x.memory.buffer, p, bytes.length).set(bytes);
      const r = x.stream_call(p, bytes.length);
      x.stream_free(p, bytes.length);
      return JSON.parse(dec.decode(new Uint8Array(x.memory.buffer, r, x.stream_out_len())));
    },
  };
}

// Canonical JSON, as the kernel requires: sorted keys, no whitespace,
// integers only, minimal escaping.
export function canon(v) {
  if (v === null) return 'null';
  if (Array.isArray(v)) return '[' + v.map(canon).join(',') + ']';
  switch (typeof v) {
    case 'object':
      return '{' + Object.keys(v).filter((k) => v[k] !== undefined).sort().map((k) => JSON.stringify(k) + ':' + canon(v[k])).join(',') + '}';
    case 'number':
      if (!Number.isInteger(v)) throw new Error('payload numbers must be integers; write quantities as strings');
      return String(v);
    case 'boolean':
      return String(v);
    default:
      return JSON.stringify(String(v));
  }
}

export const nowUs = () => Date.now() * 1000;

export function timeStr(us) {
  const secs = Math.floor(us / 1e6);
  const micros = us % 1e6;
  if (!micros) return String(secs);
  return secs + '.' + String(micros).padStart(6, '0').replace(/0+$/, '');
}

export const entryLine = (us, type, object, payload) => `${timeStr(us)} ${type} ${object} ${canon(payload)}`;

// The 64 verbs by code, from spec/kernel.descriptor.
export const VERBS = `POWER_CHANGE MODE_CHANGE GATE RESTORE CALIBRATE ASSIGN_CLASS SUSPEND RESUME
GRANT REVOKE DELEGATE EXPIRE CREATE BIND ATTEST VERIFY
OPEN CLOSE JOIN LEAVE REMOVE COMMIT AMEND DISPUTE
SEND RECEIVE ACK INVOICE PAY QUOTE AGREE COMPLETE
ASSIGN START PROGRESS FINISH INSPECT REPORT EXPENSE TRAVEL
OBSERVE NOTE JOURNAL CAPTURE MEASURE ESTIMATE COMPARE ALERT
SCHEDULE RESCHEDULE CANCEL REMIND TRIGGER RECUR ATTEND DELEGATE_EVENT
QUERY RESPOND ACT DEFER LEARN TEACH RECOMMEND AUTOMATE`.split(/\s+/);

export const BLOCKS = ['system', 'identity', 'structural', 'exchange', 'work', 'observe', 'schedule', 'learning'];

// The verb named in a type token such as "R:ACT(".
export function verbOf(type) {
  return type.replace(/^[A-Z]:/, '').replace(/[()*]$/, '');
}
export const blockOf = (verb) => BLOCKS[Math.max(0, VERBS.indexOf(verb)) >> 3];
