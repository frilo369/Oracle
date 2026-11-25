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
