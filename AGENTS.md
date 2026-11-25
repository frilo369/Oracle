# Oracle

Windows tray companion for League of Legends. Rust/Tauri 2 with a buildless HTML/CSS/JavaScript interface. Version 1.0.0, setup label 1.0.

- Reference: `C:\Users\fryil\Documents\Rust-Friloverse-Skins` (Umbra). Read-only reference; it has unrelated uncommitted work. Oracle copies guide assets/code only, never its skin engine, LTK, party, updater or Vanguard code.
- Visual direction: Umbra Default, silver frames, ruby accents, eclipse artwork, Cinzel headings. Guides preserve the original layouts and providers (MetaSRC for Rift/ARAM/Arena, Tencent for Mayhem).
- Commands: `scripts/cargo.ps1 check`; `scripts/verify.ps1`; `scripts/build.ps1`; `scripts/build-setup.ps1`. Build output: `dist/<version>/Oracle-<version>-Setup.exe`. Every delivered build needs a new version. Never push/publish/deploy without explicit owner authorization.
- `src-tauri/src/shell.rs`: state, commands, dashboard and Mayhem window. `league.rs`: read-only LCU polling and live-client fallback. `match_guide.rs`, `switcher.rs`, `guide_keys.rs`: inherited guide/focus behavior. `settings.rs`: atomic persistent settings and notebook. `src/ui/guide/`: inherited guide layouts/parsers; internal Umbra names retained for compatibility.
- Use only disposable `.test` profiles for verification (`ORACLE_DATA_DIR`). Never write to League, LTK or Umbra data. Local League credentials stay in memory on loopback; never log them.
- Remote MetaSRC windows receive no Tauri command capabilities. Never automate human verification; leave Cloudflare checks visible. WebView creation must happen off WebView callbacks.
- Keep short phase notes here; longer decisions and ideas belong in `docs/`.
