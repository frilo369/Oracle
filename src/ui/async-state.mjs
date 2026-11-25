// Keep cancellation ordered after an in-flight native capture request.
export function captureController(send, changed = () => {}) {
  let desired = false, applied = false, pending = null;
  const controller = {
    get active() { return desired; },
    get pending() { return pending !== null; },
    set(on) {
      desired = Boolean(on);
      if (!pending) {
        pending = Promise.resolve().then(async () => {
          while (applied !== desired) {
            const next = desired;
            try { await send(next); }
            catch (error) {
              if (next) desired = false;
              throw error;
            }
            applied = next;
          }
        }).finally(() => { pending = null; changed(); });
      }
      changed();
      return pending;
    },
  };
  return controller;
}

export function revisionGate() {
  let revision = 0, pending = 0;
  return {
    begin() { revision++; pending++; return () => { pending--; }; },
    snapshot() { return revision; },
    accepts(value) { return pending === 0 && value === revision; },
  };
}

// Compact snapshots omit immutable catalogues and retired preferences. Preserve them,
// and retain object identity when nothing displayed has changed.
export function mergeSnapshot(state, snapshot) {
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const changed = Object.entries(snapshot).some(([key, value]) => key === 'settings'
    ? Object.entries(value).some(([setting, next]) => !same(state.settings[setting], next))
    : !same(state[key], value));
  return changed ? {...state, ...snapshot, settings: {...state.settings, ...snapshot.settings}} : state;
}

// Only one refresh can run at a time, including a hide/show during an IPC request.
export function refreshController(run, interval = 1200, timers = globalThis) {
  let active = false, pending = false, timer = null, rerun = false;
  const clear = () => { if (timer !== null) timers.clearTimeout(timer); timer = null; };
  async function tick() {
    clear();
    if (!active || pending) return;
    pending = true;
    try { await run(); }
    finally {
      pending = false;
      if (active) timer = timers.setTimeout(tick, rerun ? 0 : interval);
      rerun = false;
    }
  }
  return {
    setActive(value) {
      value = Boolean(value);
      if (value === active) return;
      active = value;
      clear();
      if (active) { if (pending) rerun = true; else void tick(); }
    },
  };
}
