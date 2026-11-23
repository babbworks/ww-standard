// Keeps the log's bytes (segments and packs) in the browser between visits.
// The log itself is the only state that matters; everything on screen is
// rebuilt from it by the engine.

const DB = 'stream-web';
const STORE = 'logs';
const KEY = 'current';

function open() {
  return new Promise((resolve, reject) => {
    const r = indexedDB.open(DB, 1);
    r.onupgradeneeded = () => r.result.createObjectStore(STORE);
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error);
  });
}

function run(mode, fn) {
  return open().then(
    (db) =>
      new Promise((resolve, reject) => {
        const tx = db.transaction(STORE, mode);
        const req = fn(tx.objectStore(STORE));
        tx.oncomplete = () => resolve(req.result);
        tx.onerror = () => reject(tx.error);
      }),
  );
}

export async function loadLog() {
  try {
    return (await run('readonly', (s) => s.get(KEY))) || null;
  } catch (e) {
    console.warn('browser storage unavailable', e);
    return null;
  }
}

export async function saveLog(state) {
  try {
    await run('readwrite', (s) => s.put(state, KEY));
    return true;
  } catch (e) {
    console.warn('could not save the log', e);
    return false;
  }
}

// Keep a log that could not be opened, rather than overwrite it.
export async function backupLog(state) {
  const key = 'unopened-' + new Date().toISOString();
  try {
    await run('readwrite', (s) => s.put(state, key));
  } catch (e) {
    console.warn(e);
  }
  return key;
}

export async function clearLog() {
  try {
    await run('readwrite', (s) => s.delete(KEY));
  } catch (e) {
    console.warn(e);
  }
}

// Small per-viewer conveniences, such as who you are recording as.
export const prefs = {
  get(k, d) {
    try {
      return localStorage.getItem('stream.' + k) ?? d;
    } catch {
      return d;
    }
  },
  set(k, v) {
    try {
      localStorage.setItem('stream.' + k, v);
    } catch {}
  },
};
