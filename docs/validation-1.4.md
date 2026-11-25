# Oracle 1.4 — 25 November 2026

Final setup: `C:\FriloCode\Oracle\dist\1.4\Oracle-1.4-Setup.exe` (7,756,288 bytes).

SHA-256: `3aa9519f281bad0aedf919ff76326bf308ca1d407598010f05207560c1ce7e11`.

Portable app: `C:\FriloCode\Oracle\dist\1.4\Oracle.exe`. Version 1.3 was a local acceptance build before completing the reference's native asset cache. No push, publication, deployment or real installation was performed; the installed 1.2 process/profile remained unchanged.

## Accepted

- `scripts/verify.ps1`: formatting/check and 23 Rust + 5 frontend tests pass without compiler warnings. One explicit live test is ignored by the aggregate suite and was run separately successfully. New tests cover skill ordering at both levels, tie/missing-rate handling, default-tray migration and opt-out, legacy-data preservation, bounded archive reads, plain/gzip/zstd/interleaved decoding, namespace/traversal rejection and archive-index refresh.
- Native Windows/WebView2: `.test/native-1795636831748/verified.json`. Compact layout fits; Notebook/Library are absent; tray default, remembered manual selection and shortcut save/restart pass. F1 under a deliberate global registration conflict, the actual own-process activation helper, a game stand-in that minimizes on deactivation, return to game, no-reload refocus/reopen/restore, custom Ctrl+G, extra-modifier rejection, capture/end suppression and second-launch forwarding all pass. No page errors.
- Native cached/fixture timings: cold F1 129 ms, refocus 129 ms, reopen 79 ms, restore 130 ms, activation helper 126 ms, fullscreen-like F1/return 129 ms. These measure the native path with fixture recommendations, not provider network latency.
- Read-only installed assets: native Cargo test `live_league_guide_assets -- --ignored` passes against League's champion metadata and Annie artwork. Quote the separator when calling native cargo from PowerShell; the helper script's parameter binder otherwise consumes it.
- Actual WebView protocol: `.test/live-assets-1795636875805/verified.json`. A normal Oracle instance isolated to a .test profile decoded the installed League image as 128×128 PNG, HTTP 200, `Cache-Control: private, max-age=60`. It only read the archives. Fixture mode refuses access to real assets.
- Provider flow, unchanged from the accepted 1.3 implementation: `.test/meta-1795636235669/verified.json`. Ahri Arena renders 7 cards/227 augments. Remote settings commands are denied in all three MetaSRC modes. ARAM/Rift human verification is recorded as skipped, never bypassed.
- Final setup: `.test/installer-fc0bf81aca92469e9a55c4719b381270/verified.json`. Payload hash, install/reinstall/uninstall and preservation pass in a disposable destination.
- Drawn setup: `.test/setup-ui-2fbbdffe0bfc40899cd46828f4374578`. Install, a deliberate locked-file failure that preserves the original executable, and uninstall pass. App and setup screenshots were reviewed. Microsoft bootstrapper signature was checked before packaging. Oracle's own local setup remains unsigned.
- Reference adapter guards pass with `node scripts/sync-guide-reference.mjs --check`; source reference is only read.

## Remaining live acceptance

The agent did not play an actual exclusive-fullscreen League match. Real Win32 activation and a minimize-on-focus-loss stand-in exercise the regression, but actual League fullscreen acceptance still requires a player session. Windows startup registry mutation is skipped in fixture mode. The detailed reference-flow map and the precise fullscreen/return-focus fixes are in `docs/guide-behaviour.md`.
