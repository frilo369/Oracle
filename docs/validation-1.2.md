# Oracle 1.2 — local acceptance, 25 November 2026

Installer: `C:\FriloCode\Oracle\dist\1.2\Oracle-1.2-Setup.exe` (7,552,000 bytes).

SHA-256: `342a7a0e564ec75bc3a3673a543eb57716065bc617b0daf3d5a9b6237497afc8`.

Portable app: `C:\FriloCode\Oracle\dist\1.2\Oracle.exe`. Source is local Git; nothing was pushed, published or deployed. The reference source was only read/copied.

## Accepted

- Rust formatting and `cargo check --locked --offline`: pass, no warnings.
- 20 Rust tests and 4 frontend tests: pass. Includes cache validation, local-player identity and last-second swap correction, unknown-mode refusal, command-line client discovery, key parsing, exact modifiers, atomic settings and invalid-file preservation.
- Native Windows/WebView2 acceptance: `.test/native-1795634489857/verified.json`. Real keyboard events from an owned borderless game stand-in: F1 with another process reserving the global shortcut, Ctrl+G, refocus without reload, close/reopen, minimize/restore, capture suppression, match-end suppression, Unicode note/favorite persistence, cold tray startup, closing to tray, and second-launch forwarding. No page errors. Screenshots inspected for overview, settings, library, notebook and Mayhem.
- 1.2 setup acceptance: `.test/installer-d8e0c28e51c646f18106ae690d4b49f6/verified.json`. Install, payload SHA-256, same-version reinstall, uninstall and preservation of unrelated user files pass. Only disposable workspace folders were installed into.
- Drawn installer UI/engine acceptance on 1.1 (installer source unchanged in 1.2): `.test/setup-ui-32fffd32a0c24eedb49b8003f446401a`. Install and uninstall pass; a deliberately locked application causes a visible failure without altering the existing executable. Setup screenshots inspected. 1.2 only formats Rust source and changes version metadata.
- Live MetaSRC: `.test/meta-1795634330777/verified.json`. Ahri Arena renders 7 cards and 227 augments. Remote pages cannot call Oracle settings commands in Arena, ARAM or Rift.
- Live Mayhem: `.test/mayhem-live.json`. Annie (115 augments) and Ahri (138 augments) parse with build date **24 November 2026** and all 3 item sections.
- Quit-only launch with no running instance exits 0, without showing an interface.

## Limits

- No actual League match was played by the agent. LCU discovery and match identity are implemented from the reference and tested with recorded state; the foreground/shortcut flow uses a native stand-in. Real in-game acceptance remains open.
- MetaSRC ARAM and Rift asked for human verification. Those checks were recorded as skipped and left visible; no human verification was automated or bypassed. Arena and Mayhem data did load live.
- Optional Windows startup registry writes were skipped in disposable fixture mode. The app uses the standard per-user Run entry when the player enables the setting.
- This local installer has no configured publisher signature and no remote update feed. The Microsoft WebView2 bootstrapper was checked for a valid Microsoft signature before packaging. Future publication needs separate owner approval.
