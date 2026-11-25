# Oracle performance — 25 November 2026

Version **1.7.0**, setup label **1.7**. Previous delivered version: 1.6.

Setup: `C:\FriloCode\Oracle\dist\1.7\Oracle-1.7-Setup.exe` (7,784,960 bytes).

SHA-256: `6877e9cc99ed649c70af79f3ca1167bfa125ccdcd5a3849f5fa91e5f146dc6f5`.

Portable app: `C:\FriloCode\Oracle\dist\1.7\Oracle.exe`.

## Changes and measurements

- Dashboard refresh formerly cloned and transferred the entire champion catalogue and all retained preferences every 1.2 seconds. Compact refresh snapshots transfer only live dashboard fields and relevant preferences. In a fresh native fixture, this reduced UTF-8 JSON from **27,274 to 327 bytes (98.8%)**. The initial picker load and action responses remain compatible. Retired notebook data stays on disk and in full state.
- Unchanged snapshots retain frontend state identity and skip DOM painting. Appearance code also skips redundant CSS writes, computed-style reads and colour events. The native idle sample made two normal refreshes over 2.6 seconds with **zero body mutations and zero colour events**.
- Hiding or minimizing the dashboard stops its refresh timer. Separate 2.6-second samples recorded **zero dashboard refresh requests** in both states. Native visibility notifications resume it immediately; a changed champion appeared **40 ms** after showing the hidden dashboard. League observation and guide shortcuts keep running independently.
- Refresh requests remain serialized across hide/show transitions. Existing revision checks still reject snapshots that predate settings saves. Compact merges preserve the static champion catalogue and omitted retired preferences.
- Mayhem search now toggles existing rows instead of replacing their HTML for each keystroke. Forty alternating searches preserved the same **109 row nodes**, correctly hid all unmatched rows and restored every row. Loaded image nodes and their fallback state survive filtering. The script records search handler time; a reported median of 0 ms is timer resolution, not a claim of zero cost.
- Native custom-colour propagation and repeated same-colour application pass. The guide layout and ranking choices are preserved; the Mayhem screenshot was reviewed.

Measurements describe disposable local fixtures, not internet-provider latency or whole-process CPU/memory benchmarks.

## Verification

- `scripts/verify.ps1`: formatting, locked offline check, **29 Rust tests and 11 frontend tests pass**. One explicit installed-asset Rust test remains opt-in.
- Strict Clippy (`--all-targets -- -D warnings`), JavaScript syntax, Git whitespace and guarded reference adapters pass.
- Release executable built from the checked source with Rust's release profile.
- `node scripts/check-performance.mjs`: `.test/performance-1795639700949/verified.json`. Compact payload, unchanged paints, hidden/minimized timer suspension, immediate resume, search node preservation and colour propagation pass. No page errors. This reusable script needs only the existing Playwright Core package and actual WebView2, with a disposable `.test` profile.
- `node scripts/check-native.mjs`: `.test/native-1795639722087/verified.json`. Full fixture lifecycle passes: pending capture cancellation, saved manual-click target, compact layout, remembered preferences, shortcut conflict fallback, custom Ctrl+G, activation helper, fullscreen-style minimize/return, no-reload reopening/restoring and match-end suppression. Idle and search assertions are included. No page errors.
- Native fixture timings: prepared F1 130 ms, refocus 126 ms, reopen 76 ms, restore 129 ms, activation helper 142 ms, fullscreen-style F1 131 ms, Ctrl+G 129 ms. These are fixture observations with polling overhead; they do not establish a latency improvement over 1.6.
- `scripts/check-setup.ps1 -Setup dist/1.7/Oracle-1.7-Setup.exe`: `.test/installer-806aa6370c6149099f45e37fee4e67fd/verified.json`. Payload hash, install/reinstall/uninstall and preservation of extra root/bin/nested files pass in a disposable installation. The packaged WebView2 bootstrapper has a valid Microsoft signature.

## Decisions and acceptance limits

- A first implementation paused refresh on focus. Native checks showed that the WebView2/Tauri focus flag can remain false while the dashboard is visible. Use actual native visibility/minimized state and explicit show/hide notifications. Listen before the initial visibility query and reject its result if a newer visibility event arrives.
- Two exploratory full-harness runs failed to foreground the fixture activation helper. A later full run passed with the product's activation/focus code unchanged; preserve the traces rather than treating a passing fixture as exclusive-fullscreen proof.
- An actual exclusive-fullscreen League match was not played. ARAM/Rift human verification and live providers were not rechecked in this performance phase; their prior acceptance limits remain. No real installation, publication, push or deployment was performed.
