// STREAM · Now. Every element on this page is a primitive of the log made
// visible: open intervals are begun acts without an end, the palette is the
// verbs, and the phase decides the control. Nothing here holds state of its
// own; each change goes to the engine and the screen is rebuilt from the log.

import { loadEngine, entryLine, nowUs, verbOf, blockOf } from './engine.js';
import { loadLog, saveLog, clearLog, backupLog, prefs } from './storage.js';
import { newLog, ROOT, HOST, AGENT, METER } from './setup.js';
import { createSim } from './sim.js';

const $ = (s, el = document) => el.querySelector(s);
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

const UNITS = { J: 'J', xs: 'CPU-s', ti: 'tokens in', to: 'tokens out', tr: 'cache-read tokens', tw: 'cache-write tokens', h: 'actor-h', USD: 'USD' };
const USE_UNITS = ['J', 'xs', 'ti', 'to'];
const DECIMAL = /^-?\d+(\.\d+)?$/;

const ACTS = [
  { key: 'query', label: 'Query', verb: 'QUERY', phase: 'begin', control: 'begin · end toggle' },
  { key: 'act', label: 'Act', verb: 'ACT', phase: 'begin', control: 'begin · end toggle' },
  { key: 'defer', label: 'Defer to a person', verb: 'DEFER', control: 'instant' },
  { key: 'finish', label: 'Finish task', verb: 'FINISH', control: 'instant' },
  { key: 'note', label: 'Note', verb: 'NOTE', control: 'instant · text' },
  { key: 'estimate', label: 'Estimate', verb: 'ESTIMATE', control: 'instant · value, method' },
  { key: 'pay', label: 'Pay', verb: 'PAY', control: 'instant · balanced legs' },
  { key: 'amend', label: 'Amend', verb: 'AMEND', control: 'instant · supersedes, keeps original' },
];

let engine;
let view;
let me = prefs.get('as', ROOT);
const localPs = {};
const receipts = [];
let sim;

// ----------------------------------------------------------------- engine

function refresh() {
  view = engine.call({ op: 'now' });
  if (view.error) throw new Error(view.error);
  render();
}

// Save after every change. Saves run one at a time; a save asked for while
// one is running waits, then writes the latest state.
let saving = null;
let again = false;
function persist() {
  if (saving) {
    again = true;
    return saving;
  }
  saving = (async () => {
    do {
      again = false;
      const ok = await saveLog(engine.call({ op: 'state' }));
      $('#saved').textContent = ok ? 'Saved in this browser' : 'Not saved: browser storage is unavailable';
    } while (again);
    saving = null;
  })();
  return saving;
}

function nextPs(rec) {
  const ps = Math.max(view?.next_ps?.[rec] ?? 1, localPs[rec] ?? 1);
  localPs[rec] = ps + 1;
  return ps;
}

// A producer sequence number used without a submission: the log will show it as a gap.
function skipPs(rec) {
  nextPs(rec);
}

function submitLine(as, line, label) {
  const r = engine.call({ op: 'submit', as, line, now_us: nowUs() });
  if (r.error) throw new Error(r.error);
  receipts.unshift({ ...r, as, sent: line, label, at: Date.now() });
  receipts.length = Math.min(receipts.length, 30);
  persist();
  refresh();
  return r;
}

function submit(as, type, object, payload, label) {
  const p = { ...payload };
  for (const k of Object.keys(p)) if (p[k] === undefined || p[k] === '') delete p[k];
  if (p.ps === undefined) p.ps = nextPs(as);
  return submitLine(as, entryLine(nowUs(), type, object, p), label);
}

const check = (as, line) => engine.call({ op: 'check', as, line });

// --------------------------------------------------------------- register

const decls = (kind) => (view?.register || []).filter((d) => d.kind === kind);
const actors = () => decls('actor');
const nameOf = (id) => actors().find((a) => a.id === id)?.fields?.name || id;
const classOf = (id) => actors().find((a) => a.id === id)?.fields?.class;

// --------------------------------------------------------------- formatting

function elapsed(fromUs) {
  let s = Math.max(0, Math.floor((nowUs() - fromUs) / 1e6));
  const h = Math.floor(s / 3600);
  s -= h * 3600;
  const m = Math.floor(s / 60);
  s -= m * 60;
  return (h ? `${h}:${String(m).padStart(2, '0')}` : `${m}`) + `:${String(s).padStart(2, '0')}`;
}
const clock = (us) => new Date(us / 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
const num = (q) => {
  const est = String(q).startsWith('~');
  const n = String(q).replace(/^~/, '');
  const [i, f] = n.split('.');
  return (est ? '~' : '') + Number(i).toLocaleString() + (f ? '.' + f : '');
};
const useText = (u) =>
  Object.entries(u || {})
    .map(([k, v]) => `${num(v)} ${UNITS[k] || k}`)
    .join(' · ');

function chip(type) {
  const b = blockOf(verbOf(type));
  return `<span class="chip b-${b}"><i aria-hidden="true"></i>${esc(type)}</span>`;
}

// ------------------------------------------------------------------ render

function render() {
  renderSide();
  renderHeader();
  renderRunning();
  renderActs();
  renderLive();
  renderAttention();
  renderClosed();
  renderReceipts();
  renderRecent();
}

function renderSide() {
  const opts = actors()
    .map((a) => `<option value="${esc(a.id)}" ${a.id === me ? 'selected' : ''}>${esc(a.fields.name || a.id)}</option>`)
    .join('');
  $('#as').innerHTML = opts;
  $('#as-id').textContent = me;
  $('#as-note').textContent = view.roots.includes(me) ? 'Root of this log: may do anything.' : grantNote(me);
  $('#kernel').textContent = view.kernel.slice(0, 12) + '…';
}

// What the log says this actor may do, found by a dry run rather than assumed.
function grantNote(who) {
  const probe = entryLine(nowUs(), 'R:QUERY(', 'probe', { actor: AGENT, run: 'probe' });
  return check(who, probe).ok ? `May record for ${AGENT} (granted).` : 'May record only its own acts.';
}

function renderHeader() {
  $('#log-id').textContent = `log:${view.log}`;
  const seal = view.last_seal ? `last seal ${view.last_seal.addr} at ${clock(view.last_seal.time_us)}` : 'never sealed';
  $('#seg').innerHTML = `Segment ${view.segment} · ${view.sealed ? 'sealed' : `${view.since_seal} entries since seal`} · ${esc(seal)}`;
  $('#seal').disabled = view.sealed;
  $('#count').textContent = `${view.entries} entries · last ${view.last}`;
}

function order(open) {
  const byRun = new Map(open.map((s) => [s.run, s]));
  const kids = new Map();
  const roots = [];
  for (const s of open) {
    if (s.parent && byRun.has(s.parent)) {
      if (!kids.has(s.parent)) kids.set(s.parent, []);
      kids.get(s.parent).push(s);
    } else roots.push(s);
  }
  const out = [];
  const walk = (s, d) => {
    out.push([s, d]);
    (kids.get(s.run) || []).forEach((k) => walk(k, d + 1));
  };
  roots.forEach((r) => walk(r, 0));
  return out;
}

function soFar(s) {
  const u = { ...(s.from_children || {}) };
  const m = (s.from_meters || {})[HOST];
  if (m?.J) u.J = m.J;
  return u;
}

function renderRunning() {
  const rows = order(view.open);
  $('#running-h').textContent = `Running · ${rows.length} open interval${rows.length === 1 ? '' : 's'}`;
  $('#running').innerHTML = rows.length
    ? rows
        .map(([s, d]) => {
          const u = soFar(s);
          return `<div class="run" style="--depth:${d}">
            ${chip(s.type)}
            <div class="grow">
              <div class="title">${esc(s.title || `${s.object} · ${s.run}`)}</div>
              <div class="meta">${esc(s.actor)}${s.rec !== s.actor ? ` · recorded by ${esc(s.rec)}` : ''} · run ${esc(s.run)}${s.parent ? ` · parent ${esc(s.parent)}` : ''} · begun ${esc(s.begin)}</div>
            </div>
            <div class="mono elapsed" data-since="${s.begin_us}">${elapsed(s.begin_us)}</div>
            <div class="meta use">${Object.keys(u).length ? 'so far ' + esc(useText(u)) : 'nothing measured yet'}${s.children_closed ? ` · ${s.children_closed} closed below` : ''}</div>
            <button class="btn" data-end="${esc(s.run)}">End</button>
          </div>`;
        })
        .join('')
    : `<p class="empty">Nothing is running. Begin a Query or an Act, or press Play to run the simulated agent.</p>`;
}

function renderActs() {
  $('#acts').innerHTML = ACTS.map(
    (a) => `<button class="act b-${blockOf(a.verb)}" data-act="${a.key}">
      <span class="act-label"><i aria-hidden="true"></i>${esc(a.label)}</span>
      <span class="act-control">${esc(a.verb)}${a.phase === 'begin' ? '(' : ''} · ${esc(a.control)}</span>
    </button>`,
  ).join('');
}

function renderLive() {
  const ss = view.samples.filter((s) => s.object === METER);
  const gaps = new Set((view.gaps[HOST] || []).map(Number));
  const svg = $('#spark');
  if (!ss.length) {
    svg.innerHTML = `<text x="160" y="64" text-anchor="middle" class="svg-note">No samples yet. Press Play, or Sample once.</text>`;
    $('#live-stats').innerHTML = '';
    return;
  }
  const qs = ss.map((s) => Number(s.q));
  const lo = Math.min(...qs) - 4;
  const hi = Math.max(...qs) + 4;
  const X = (i) => (ss.length === 1 ? 160 : 10 + (i * 300) / (ss.length - 1));
  const Y = (q) => 100 - ((q - lo) / (hi - lo || 1)) * 80;
  let paths = '';
  let seg = [];
  let gapCount = 0;
  const missing = [];
  ss.forEach((s, i) => {
    if (i > 0) {
      const lost = [...gaps].filter((p) => p > ss[i - 1].ps && p < s.ps);
      if (lost.length) {
        paths += `<polyline points="${seg.join(' ')}" class="line"/>`;
        paths += `<polyline points="${X(i - 1)},${Y(qs[i - 1])} ${X(i)},${Y(qs[i])}" class="gapline"/>`;
        gapCount += 1;
        missing.push(...lost);
        seg = [];
      }
    }
    seg.push(`${X(i)},${Y(qs[i])}`);
  });
  paths += `<polyline points="${seg.join(' ')}" class="line"/>`;
  const dots = ss.map((s, i) => `<circle cx="${X(i)}" cy="${Y(qs[i])}" r="2.5" class="dot"><title>${esc(s.addr)} · ${esc(s.q)} J</title></circle>`).join('');
  svg.innerHTML = `<line x1="0" y1="100" x2="320" y2="100" class="axis"/>${paths}${dots}`;
  svg.setAttribute('aria-label', `Energy per sample, joules: ${qs.join(', ')}${gapCount ? `; ${gapCount} gaps` : ''}`);
  const last = ss[ss.length - 1];
  const avgW = qs.reduce((a, b) => a + b, 0) / qs.length / 8;
  $('#live-stats').innerHTML = `<span><strong>${esc(last.q)} J</strong> latest</span><span>${avgW.toFixed(1)} W average</span>${
    missing.length ? `<span class="bad">${missing.length} lost (ps ${missing.slice(-4).join(', ')})</span>` : '<span>no gaps</span>'
  }`;
}

function renderAttention() {
  const items = [];
  for (const r of view.refusals) items.push({ title: 'Refused', addr: r.addr, detail: `${r.rec}: ${r.reason}. Journaled; a retry gets the same answer.` });
  for (const [rec, ps] of Object.entries(view.gaps)) items.push({ title: `Gap: ${rec}`, addr: '—', detail: `Sequence numbers never received: ${ps.slice(-8).join(', ')}${ps.length > 8 ? ` and ${ps.length - 8} more` : ''}.` });
  for (const a of view.anomalies) items.push({ title: 'Pairing', addr: '—', detail: a });
  $('#att-h').textContent = `Attention · ${items.length}`;
  $('#attention').innerHTML = items.length
    ? items
        .slice(0, 8)
        .map((x) => `<div class="item"><div class="row"><strong>${esc(x.title)}</strong><span class="mono meta">${esc(x.addr)}</span></div><div class="detail">${esc(x.detail)}</div></div>`)
        .join('')
    : `<p class="empty">Nothing contradicts history, and nothing is missing.</p>`;
}

function renderClosed() {
  $('#closed').innerHTML = view.closed.length
    ? view.closed
        .slice(0, 8)
        .map(
          (s) => `<div class="item"><div class="row">${chip(s.type.replace('(', ')'))}<span class="grow">${esc(s.title || s.run)}</span><span class="mono meta">${esc(s.end)}</span></div>
          <div class="detail">${esc(s.wall)} s wall${s.use && Object.keys(s.use).length ? ' · ' + esc(useText(s.use)) : ''}${s.pool ? ' · from ' + esc(s.pool) : ''}</div></div>`,
        )
        .join('')
    : `<p class="empty">No interval has closed yet.</p>`;
}

function renderReceipts() {
  $('#receipts').innerHTML = receipts.length
    ? receipts
        .slice(0, 8)
        .map(
          (c, i) => `<div class="receipt"><span class="badge ${c.status}">${c.status}</span><span class="mono meta">${esc(c.addr || '—')}</span>
          <span class="grow">${esc(c.as)} · ${esc(c.sent.split(' ').slice(1, 3).join(' '))}${c.reason ? `<br><span class="meta">${esc(c.reason)}</span>` : ''}</span>
          <button class="link" data-retry="${i}" title="Submit exactly the same entry again">Retry</button></div>`,
        )
        .join('')
    : `<p class="empty">Every submission gets an answer, and a retry gets the same one.</p>`;
}

function renderRecent() {
  $('#recent').innerHTML = view.recent
    .slice(0, 14)
    .map((e) => `<li class="${e.superseded ? 'superseded' : ''}"><span class="mono addr">${esc(e.addr)}</span>${chip(e.type)}<code>${esc(e.line)}</code></li>`)
    .join('');
}

function tick() {
  document.querySelectorAll('[data-since]').forEach((el) => (el.textContent = elapsed(Number(el.dataset.since))));
  $('#asof').textContent = new Date().toLocaleTimeString();
}

// -------------------------------------------------------------------- forms

const dialog = () => $('#form');

function field(f) {
  const id = `f-${f.name}`;
  let input;
  if (f.options) {
    input = `<select id="${id}" name="${f.name}">${f.options.map((o) => `<option value="${esc(o.value)}" ${o.value === f.value ? 'selected' : ''}>${esc(o.label)}</option>`).join('')}</select>`;
  } else if (f.kind === 'textarea') {
    input = `<textarea id="${id}" name="${f.name}" rows="3">${esc(f.value)}</textarea>`;
  } else {
    input = `<input id="${id}" name="${f.name}" value="${esc(f.value)}" ${f.placeholder ? `placeholder="${esc(f.placeholder)}"` : ''} autocomplete="off">`;
  }
  return `<div class="field"><label for="${id}">${esc(f.label)}</label>${input}${f.hint ? `<div class="hint">${esc(f.hint)}</div>` : ''}</div>`;
}

const actorOpts = (filter = () => true) => actors().filter(filter).map((a) => ({ value: a.id, label: `${a.fields.name || a.id} (${a.id})` }));
const defaultActor = () => (classOf(me) === 'machine' ? AGENT : me);
const runId = () => 'r' + Date.now().toString(36).slice(-5);
const lastObject = () => view.open[0]?.object || view.closed[0]?.object || 'task-1';

function spec(key) {
  const openRuns = [{ value: '', label: 'none' }, ...view.open.map((s) => ({ value: s.run, label: `${s.run} · ${s.title || s.object}` }))];
  const unitOpts = Object.entries(UNITS).map(([v, l]) => ({ value: v, label: `${v} · ${l}` }));
  switch (key) {
    case 'query':
    case 'act': {
      const verb = key === 'query' ? 'QUERY' : 'ACT';
      return {
        title: `Begin ${key === 'query' ? 'a query' : 'an act'}`,
        intro: 'Begins an interval. It stays in Running until an End closes it, paired by run.',
        fields: [
          { name: 'object', label: 'Object', value: lastObject(), hint: 'What the act is on, such as task-42.' },
          { name: 'title', label: 'Title', value: '', placeholder: key === 'query' ? 'Model call' : 'Task · what it does' },
          { name: 'actor', label: 'Actor', value: defaultActor(), options: actorOpts() },
          { name: 'parent', label: 'Part of', value: view.open[0]?.run || '', options: openRuns },
          { name: 'run', label: 'Run', value: runId(), hint: 'Pairs the begin with its end.' },
        ],
        build: (v) => ({ type: `R:${verb}(`, object: v.object, payload: { actor: v.actor, run: v.run, parent: v.parent, title: v.title } }),
      };
    }
    case 'defer':
      return {
        title: 'Defer to a person',
        intro: 'Hands a decision to a person. An instant: one entry.',
        fields: [
          { name: 'object', label: 'Object', value: lastObject() },
          { name: 'actor', label: 'Actor', value: defaultActor(), options: actorOpts() },
          { name: 'to', label: 'To', value: ROOT, options: actorOpts((a) => a.fields.class === 'person') },
          { name: 'note', label: 'Why', value: '', kind: 'textarea' },
        ],
        build: (v) => ({ type: 'DEFER', object: v.object, payload: { actor: v.actor, to: v.to, note: v.note } }),
      };
    case 'finish':
      return {
        title: 'Finish task',
        intro: 'Marks a work item complete. An instant.',
        fields: [
          { name: 'object', label: 'Task', value: lastObject() },
          { name: 'actor', label: 'Actor', value: defaultActor(), options: actorOpts() },
          { name: 'status', label: 'Status', value: 'ok', options: [{ value: 'ok', label: 'ok' }, { value: 'failed', label: 'failed' }, { value: 'abandoned', label: 'abandoned' }] },
        ],
        build: (v) => ({ type: 'FINISH', object: v.object, payload: { actor: v.actor, status: v.status } }),
      };
    case 'note':
      return {
        title: 'Note',
        intro: 'An annotation on an object. An instant.',
        fields: [
          { name: 'object', label: 'Object', value: lastObject() },
          { name: 'text', label: 'Note', value: '', kind: 'textarea' },
        ],
        build: (v) => ({ type: 'NOTE', object: v.object, payload: { text: v.text } }),
      };
    case 'estimate':
      return {
        title: 'Estimate',
        intro: 'A value that is estimated, not measured. It is recorded with ~ and the method.',
        fields: [
          { name: 'object', label: 'Object', value: lastObject() },
          { name: 'q', label: 'Value', value: '', placeholder: '470' },
          { name: 'u', label: 'Unit', value: 'J', options: unitOpts },
          { name: 'method', label: 'Method', value: 'judgement' },
          { name: 'basis', label: 'Basis (an address, optional)', value: '', placeholder: '1:31' },
        ],
        build: (v) => ({ type: 'ESTIMATE', object: v.object, payload: { q: v.q ? '~' + v.q.replace(/^~/, '') : '', u: v.u, method: v.method, basis: v.basis } }),
        valid: (v) => (DECIMAL.test(v.q.replace(/^~/, '')) ? null : 'Value must be a decimal number.'),
      };
    case 'pay': {
      const accts = [...decls('pool'), ...actors()].map((d) => ({ value: d.id, label: `${d.id} (${d.kind})` }));
      return {
        title: 'Pay',
        intro: 'A movement with balanced legs: what leaves one account arrives in the other.',
        fields: [
          { name: 'object', label: 'Payment', value: 'payment-' + runId() },
          { name: 'from', label: 'From', value: ROOT, options: accts },
          { name: 'to', label: 'To', value: 'vendor:model-api' },
          { name: 'q', label: 'Amount', value: '', placeholder: '0.09' },
          { name: 'u', label: 'Unit', value: 'USD', options: unitOpts },
        ],
        build: (v) => ({
          type: 'PAY',
          object: v.object,
          payload: { legs: [{ a: v.from, q: '-' + v.q, u: v.u }, { a: v.to, q: v.q, u: v.u }] },
        }),
        valid: (v) => (DECIMAL.test(v.q) && Number(v.q) > 0 ? null : 'Amount must be a positive decimal.'),
      };
    }
    case 'amend': {
      const mine = view.recent.filter((e) => e.rec === me && !e.superseded && !/^(OPEN|COMMIT|ACK|BIND)/.test(e.type));
      return {
        title: 'Amend',
        intro: 'Supersedes an earlier entry. The original stays in the log.',
        fields: [
          { name: 'ref', label: 'Entry', value: mine[0]?.addr || '', options: mine.length ? mine.map((e) => ({ value: e.addr, label: `${e.addr} · ${e.type} ${e.object}` })) : [{ value: '', label: 'No recent entries of yours' }] },
          { name: 'reason', label: 'Reason', value: '', kind: 'textarea' },
        ],
        build: (v) => ({ type: 'AMEND', object: view.recent.find((e) => e.addr === v.ref)?.object || 'entry', payload: { ref: v.ref, reason: v.reason } }),
        valid: (v) => (!v.ref ? 'Choose an entry.' : !v.reason ? 'Give a reason.' : null),
      };
    }
  }
}

function endSpec(run) {
  const s = view.open.find((o) => o.run === run);
  const pools = decls('pool');
  const pool = pools[0]?.id || HOST;
  const meters = (s.from_meters || {})[pool] || {};
  const kids = s.from_children || {};
  const rows = USE_UNITS.map((u) => {
    let v = '';
    let from = 'not measured';
    if (u === 'J' && meters.J) {
      v = meters.J;
      from = `meter, ${METER}`;
    } else if (kids[u]) {
      v = kids[u];
      from = `${s.children_closed} closed below`;
    }
    return { u, v: v.replace(/^~/, ''), est: v.startsWith('~'), from };
  });
  return {
    title: `End ${s.type.replace('(', ')')}`,
    intro: `${s.title || s.object} · run ${s.run} · ${elapsed(s.begin_us)} so far. The quantities move from the pool to ${s.object}.`,
    custom: `
      ${field({ name: 'pool', label: 'Pool', value: pool, options: pools.map((p) => ({ value: p.id, label: p.id })) })}
      <fieldset class="use"><legend>Use</legend>
        ${rows
          .map(
            (r) => `<div class="use-row"><label for="u-${r.u}">${esc(UNITS[r.u])}</label>
              <input id="u-${r.u}" name="use.${r.u}" value="${esc(r.v)}" inputmode="decimal" autocomplete="off">
              <label class="est"><input type="checkbox" name="est.${r.u}" ${r.est ? 'checked' : ''}> estimate</label>
              <span class="hint">${esc(r.from)}</span></div>`,
          )
          .join('')}
      </fieldset>
      ${field({ name: 'status', label: 'Status', value: 'ok', options: [{ value: 'ok', label: 'ok' }, { value: 'failed', label: 'failed' }] })}`,
    build: (v, form) => {
      const use = {};
      for (const u of USE_UNITS) {
        const q = (v[`use.${u}`] || '').trim();
        if (q) use[u] = (form.elements[`est.${u}`].checked ? '~' : '') + q;
      }
      const has = Object.keys(use).length > 0;
      const type = (s.profile ? s.profile + ':' : '') + s.verb + ')';
      return { type, object: s.object, payload: { actor: s.actor, run: s.run, pool: has ? v.pool : undefined, use: has ? use : undefined, status: v.status } };
    },
    valid: (v) => {
      for (const u of USE_UNITS) {
        const q = (v[`use.${u}`] || '').trim();
        if (q && !DECIMAL.test(q)) return `${UNITS[u]} must be a decimal number.`;
      }
      return null;
    },
  };
}

function openForm(sp) {
  const d = dialog();
  d.innerHTML = `<form method="dialog" id="the-form">
    <h2>${esc(sp.title)}</h2>
    <p class="meta">${esc(sp.intro)} Recording as <strong>${esc(me)}</strong>.</p>
    ${sp.custom || sp.fields.map(field).join('')}
    <div class="preview"><div class="meta">Entry</div><code id="preview"></code><div id="verdict" role="status"></div></div>
    <div class="buttons"><button type="button" class="btn" id="cancel">Cancel</button><button type="submit" class="btn primary" id="record">Record</button></div>
  </form>`;
  const form = $('#the-form', d);
  const values = () => Object.fromEntries(new FormData(form).entries());
  let current = null;
  const update = () => {
    const v = values();
    const problem = sp.valid?.(v);
    const b = sp.build(v, form);
    const p = { ...b.payload };
    for (const k of Object.keys(p)) if (p[k] === undefined || p[k] === '') delete p[k];
    p.ps = Math.max(view.next_ps[me] ?? 1, localPs[me] ?? 1);
    let line = '';
    try {
      line = entryLine(nowUs(), b.type, b.object || '?', p);
    } catch (e) {
      line = '';
    }
    $('#preview', d).textContent = line;
    const verdict = $('#verdict', d);
    if (problem) {
      verdict.className = 'bad';
      verdict.textContent = problem;
      current = null;
      return;
    }
    const c = check(me, line);
    verdict.className = c.ok ? 'good' : 'bad';
    verdict.textContent = c.ok ? 'The log will accept this.' : `The log will refuse this: ${c.reason}. You can still submit it; the refusal is journaled.`;
    current = b;
  };
  form.addEventListener('input', update);
  form.addEventListener('change', update);
  $('#cancel', d).onclick = () => d.close();
  form.addEventListener('submit', (ev) => {
    ev.preventDefault();
    update();
    if (!current) return;
    submit(me, current.type, current.object, current.payload, sp.title);
    d.close();
  });
  update();
  d.showModal();
}

// -------------------------------------------------------------------- wiring

function wire() {
  $('#as').addEventListener('change', (e) => {
    me = e.target.value;
    prefs.set('as', me);
    render();
  });
  document.body.addEventListener('click', (e) => {
    const t = e.target.closest('button');
    if (!t) return;
    if (t.dataset.act) openForm(spec(t.dataset.act));
    else if (t.dataset.end) openForm(endSpec(t.dataset.end));
    else if (t.dataset.retry) {
      const c = receipts[Number(t.dataset.retry)];
      submitLine(c.as, c.sent, 'retry');
    }
  });
  $('#seal').onclick = () => {
    const r = engine.call({ op: 'seal', now_us: nowUs() });
    if (r.error) alert(r.error);
    persist();
    refresh();
  };
  $('#verify').onclick = () => {
    const r = engine.call({ op: 'verify' });
    const out = $('#verify-out');
    out.className = r.problems.length ? 'bad' : 'good';
    out.textContent = r.problems.length ? r.problems.join('; ') : `Verified: every seal, the chain and every entry check out (${new Date().toLocaleTimeString()}).`;
  };
  $('#reset').onclick = async () => {
    if (!confirm('Start a new log? The current log in this browser is deleted.')) return;
    sim.pause();
    await clearLog();
    for (const k of Object.keys(localPs)) delete localPs[k];
    receipts.length = 0;
    newLog(engine);
    persist();
    refresh();
  };
  $('#play').onclick = () => (sim.status().playing ? sim.pause() : sim.play());
  $('#step').onclick = () => sim.step();
  $('#sample').onclick = () => sim.sample();
  $('#speed').onchange = (e) => sim.setSpeed(Number(e.target.value));
  sim.onChange((s) => {
    $('#play').textContent = s.playing ? 'Pause' : 'Play';
    $('#play').setAttribute('aria-pressed', String(s.playing));
    $('#sim-status').textContent = `${s.playing ? 'Playing' : 'Paused'} · next: ${s.next}`;
  });
  setInterval(tick, 1000);
}

async function main() {
  try {
    engine = await loadEngine();
  } catch (e) {
    $('#fatal').hidden = false;
    $('#fatal').textContent = `The engine did not load (${e.message}). Serve this folder over http, for example: python3 -m http.server -d stream/web`;
    return;
  }
  const saved = await loadLog();
  let opened = false;
  if (saved) {
    const r = engine.call({ op: 'open', ...saved });
    if (r.error) {
      const key = await backupLog(saved);
      $('#fatal').hidden = false;
      $('#fatal').textContent = `The saved log could not be opened (${r.error}). It was kept in this browser as “${key}”, and a new log was started.`;
    } else {
      opened = true;
      $('#saved').textContent = 'Opened from this browser';
    }
  }
  if (!opened) {
    newLog(engine);
    persist();
  }
  view = engine.call({ op: 'now' });
  if (!actors().some((a) => a.id === me)) me = ROOT;
  sim = createSim({ submit, skipPs, view: () => view });
  wire();
  render();
  tick();
  $('#sim-status').textContent = 'Paused · next: start a new run';
  document.body.classList.add('ready');
}

main();
