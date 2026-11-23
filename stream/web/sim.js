// The simulated agent: a scripted run played through the real engine.
// host:build-01 records the agent's task, model calls and tool calls, and
// the meter on its power outlet. Now and then a meter sample is lost (a
// real gap) and the operator tries to record for the agent (a real refusal).

import { AGENT, HOST, METER, OPERATOR } from './setup.js';

const TITLES = ['fix ledger import', 'add pool to run header', 'speed up seal verification', 'write importer test', 'tidy register output'];
const rnd = (a, b) => a + Math.random() * (b - a);
const fx = (x, d = 1) => x.toFixed(d).replace(/\.0+$/, '');

export function createSim(app) {
  let n = Math.floor(Date.now() / 1000) % 100;
  let speed = 1;
  let playing = false;
  let simT = 0;
  let queue = [];
  let nextMeter = 0;
  let timer = null;
  let current = null;
  const listeners = new Set();

  const emit = () => listeners.forEach((f) => f(status()));

  function status() {
    return {
      playing,
      speed,
      run: current,
      next: queue.length ? queue[0].label : 'start a new run',
      left: queue.length,
    };
  }

  function sumKids(run) {
    const s = app.view().open.find((o) => o.run === run);
    return s ? { kids: s.from_children || {}, meters: (s.from_meters || {})[HOST] || {} } : { kids: {}, meters: {} };
  }

  const begin = (verb, run, parent, title) => () =>
    app.submit(HOST, `R:${verb}(`, current.object, { actor: AGENT, run, parent, title });
  const end = (verb, run, use) => () =>
    app.submit(HOST, `R:${verb})`, current.object, { actor: AGENT, run, pool: HOST, status: 'ok', use: use() });

  function plan() {
    n += 1;
    const k = `k${n}`;
    current = { run: k, object: `task-${n}`, title: TITLES[n % TITLES.length] };
    const at = (dt, label, fn) => ({ dt, label, fn });
    const model = (m) => [
      at(2, `begin model call ${m}`, begin('QUERY', `${k}.${m}`, k, `Model call ${m}`)),
      at(rnd(3, 6), `end model call ${m}`, end('QUERY', `${k}.${m}`, () => ({ ti: String(Math.round(rnd(4000, 9000))), to: String(Math.round(rnd(200, 900))) }))),
    ];
    queue = [
      at(0, `begin task ${k}`, begin('ACT', k, undefined, `Task ${k} · ${current.title}`)),
      ...model('m1'),
      at(1, 'begin tool call t1', begin('ACT', `${k}.t1`, k, 'Tool call t1 · run test suite')),
      at(rnd(4, 7), 'operator tries to record for the agent', () =>
        app.submit(OPERATOR, 'R:QUERY(', current.object, { actor: AGENT, run: `${k}.x`, parent: k, title: 'Recorded by the operator' })),
      at(rnd(6, 10), 'end tool call t1', end('ACT', `${k}.t1`, () => {
        const xs = rnd(20, 35);
        return { xs: fx(xs), J: '~' + Math.round(xs * 14) };
      })),
      ...model('m2'),
      at(1, `end task ${k}`, end('ACT', k, () => {
        const { kids, meters } = sumKids(k);
        const use = {};
        for (const [u, v] of Object.entries(kids)) use[u] = v.replace(/^~/, '');
        use.xs = fx(Number(use.xs || 0) + rnd(1, 4));
        if (meters.J) use.J = meters.J;
        else if (kids.J) use.J = kids.J;
        return use;
      })),
    ];
    let t = simT;
    for (const e of queue) e.due = t += e.dt;
  }

  function fire(e) {
    try {
      e.fn();
    } catch (err) {
      console.error(err);
    }
    if (!queue.length) current = null;
  }

  function meter() {
    if (Math.random() < 0.12) {
      app.skipPs(HOST); // a sample that never arrives
    } else {
      app.submit(HOST, 'MEASURE*', METER, { q: String(Math.round(rnd(33, 46))), u: 'J', scope: HOST });
    }
  }

  function tick(dt) {
    simT += dt * speed;
    if (simT >= nextMeter) {
      meter();
      nextMeter = simT + 8;
    }
    if (!queue.length) {
      plan();
      queue.forEach((e) => (e.due += 3));
    }
    while (queue.length && queue[0].due <= simT) fire(queue.shift());
    emit();
  }

  return {
    status,
    onChange(f) {
      listeners.add(f);
    },
    play() {
      if (playing) return;
      playing = true;
      if (!nextMeter) nextMeter = simT;
      timer = setInterval(() => tick(0.25), 250);
      emit();
    },
    pause() {
      playing = false;
      clearInterval(timer);
      emit();
    },
    step() {
      if (!queue.length) plan();
      const e = queue.shift();
      simT = Math.max(simT, e.due);
      queue.forEach((q) => (q.due = Math.max(q.due, simT)));
      fire(e);
      emit();
    },
    sample: meter,
    setSpeed(s) {
      speed = s;
      emit();
    },
  };
}
