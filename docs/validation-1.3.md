# Oracle 1.3 — local acceptance build, 25 November 2026

This build was not delivered. Final 1.4 adds the reference's native artwork/catalogue cache; see `docs/validation-1.4.md`.

Setup: `C:\FriloCode\Oracle\dist\1.3\Oracle-1.3-Setup.exe`, 7,559,168 bytes.

SHA-256: `388cf8d7e27e22acf2b10ac70afd686585735a1c67b14855c6d84cb8fa4a83cc`.

Portable executable: `C:\FriloCode\Oracle\dist\1.3\Oracle.exe`. Nothing was pushed, published or deployed. The installed 1.2 process and its profile were left unchanged.

## Results

- `scripts/verify.ps1`: Rust formatting/check, 21 Rust tests and 5 frontend tests pass with no compiler warnings. New regressions cover ranked priority/upgrade plans, missing rates and pick-rate ties, the one-time tray default, retained retired data and a later explicit tray opt-out.
- Native Windows/WebView2: `.test/native-1795636291030/verified.json`. All scenarios pass: cold F1 with another process holding the shortcut, the actual Oracle activation helper, minimize-on-focus-loss game stand-in, close-to-game, reopen/refocus/minimize-restore without reload, custom Ctrl+G, extra-modifier rejection, recording/end suppression, cold tray startup, compact layout, removal of retired pages, saved manual selection and second-launch forwarding. No page errors.
- Native timings to the shown guide: cold fixture F1 127 ms; refocus 130 ms; hidden reopen 125 ms; restore 130 ms; activation helper 142 ms; fullscreen-like F1/return 137 ms. These use cached/fixture recommendations and measure the native path, not provider network latency.
- Overview and Settings fit the default 580×610 window. Native screenshots were reviewed. The actual Mayhem selection is asserted against the ranked parsed fixture.
- Live MetaSRC: `.test/meta-1795636235669/verified.json`. Ahri Arena renders 7 cards and 227 augments. All three remote modes refuse Oracle settings commands. ARAM/Rift ask for human verification and are recorded as skipped, never bypassed. Arena's source skill section contains one table with no alternative selection controls.
- Installer: `.test/installer-24e8833b8ff3473cb588c615de3e858f/verified.json`. Payload checksum, fixture install, reinstall, uninstall and unrelated-data preservation pass.
- Drawn installer: `.test/setup-ui-81c77fec4ea2437a9b9a40467a23b089`. Install/uninstall and a deliberately locked-file failure preserve the original executable. UI screenshots were reviewed. The bootstrapper has a valid Microsoft signature; Oracle's local setup remains unsigned.

## Limits

An actual exclusive-fullscreen League match was not played by the agent. The helper and foreground transitions are tested with real Win32 windows and a stand-in that minimizes on deactivation; actual League acceptance still needs a player session. Windows startup registry mutation is skipped in fixture mode. No installed profile or League files were used as a test destination.
