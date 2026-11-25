# Reference extraction

Oracle was started on 25 November 2026 from the owner's Umbra reference at `C:\Users\fryil\Documents\Rust-Friloverse-Skins`, whose HEAD was `e196315`. The reference contains uncommitted changes and is left in place unchanged.

Copied guide parser/layout/cache code, guide Rust data fetching and tests, keyboard/focus routines, local client connection discovery, fonts with their licence, background and framing assets, and installer drawing UI. Oracle's application state, client watcher, settings, dashboard, packaging and install engine are separate. Internal `umbra` identifiers in inherited guide JavaScript keep those components compatible; user-facing identity is Oracle.

No skin injection, patcher, LTK, party service, shared updater/feed, telemetry upload, helper DLL or Vanguard interaction is included. Original reference licence is retained at `LICENSE.reference`.

The 1.4 phase re-synchronized the three native guide/focus modules with documented Oracle adapters (`scripts/sync-guide-reference.mjs`). It also extracted only the reference's read-only WAD decoder and cached guide-asset reader (`scripts/sync-guide-assets.mjs`). WAD-writing functions, skin transforms, party/chroma code and LTK discovery were excluded. Protocol/cache behaviour is documented in `docs/guide-behaviour.md`.
