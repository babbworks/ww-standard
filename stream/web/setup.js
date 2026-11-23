// A new log for an agent operator: genesis, the two bundled packs, the
// actors, the pool and meter, and one grant. Every step is a real entry,
// checked by the engine like any other.

import { entryLine, nowUs } from './engine.js';

export const ROOT = 'human:ops-lead';
export const HOST = 'host:build-01';
export const AGENT = 'agent:claude-07';
export const OPERATOR = 'human:op-07';
export const METER = 'meter:pdu-4';

const DECLS = [
  [ROOT, { kind: 'actor', class: 'person', name: 'Ops lead' }],
  [OPERATOR, { kind: 'actor', class: 'person', name: 'Operator 07' }],
  [HOST, { kind: 'actor', class: 'machine', name: 'Build host 01' }],
  [AGENT, { kind: 'actor', class: 'agent', name: 'Agent 07' }],
  [HOST, { kind: 'pool', name: 'Build host 01', units: ['J', 'xs', 'ti', 'to'] }],
  [METER, { kind: 'meter', name: 'PDU 4, outlet 3', scope: HOST, u: 'J', every: '8' }],
];

export function newLog(engine) {
  const tz = Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';
  const t = nowUs();
  const ok = (r, what) => {
    if (r.error || (r.status && r.status !== 'accepted')) throw new Error(`${what}: ${r.error || r.reason}`);
  };
  ok(engine.call({ op: 'init', log: 'acme', root: ROOT, clock: 'browser', tz, now_us: t }), 'genesis');
  for (const pack of ['si-units@1', 'agent-ops@1']) ok(engine.call({ op: 'adopt', as: ROOT, pack, now_us: nowUs() }), pack);
  for (const [id, p] of DECLS) {
    ok(engine.call({ op: 'submit', as: ROOT, line: entryLine(nowUs(), 'CREATE', id, p), now_us: nowUs() }), id);
  }
  const grant = { to: HOST, may: 'record', for: 'agent:*' };
  ok(engine.call({ op: 'submit', as: ROOT, line: entryLine(nowUs(), 'GRANT', HOST, grant), now_us: nowUs() }), 'grant');
}
