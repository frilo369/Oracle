# Oracle

A compact Windows system tray companion for League of Legends. Oracle starts in the tray by default. Click its tray icon to open the control panel. Press **F1** in a supported match for your champion's analytics, or click the shortcut in Overview to change it. Closing the panel keeps Oracle running; choose **Quit Oracle** to exit.

Download the Windows installer from [the latest Oracle release](https://github.com/frilo369/Oracle/releases/latest). Install version 1.8 once to enable automatic updates from that version onward.

Oracle checks for updates in the background at startup. **Settings → Check for updates** checks immediately and shows progress or retry messages. A newer stable release downloads and verifies automatically, waits for matches and shortcut recording to finish, then installs and restarts quietly. A failed installation preserves the previous app and restarts it. Portable copies report available versions; automatic installation requires the standard installed copy. Public update checks need no GitHub account or token.

Guides: Summoner's Rift, ARAM and Arena through the inherited themed MetaSRC layout; ARAM Mayhem through the inherited Tencent guide. Overview contains the current match, editable shortcut and manual guide picker, which remembers your last champion and mode. Notebook and Champion Library were removed for this compact redesign. Mayhem skill priorities and detailed upgrade plans sort by highest reported win rate, then pick rate. Mayhem reads artwork and English metadata directly from installed League assets, with cached archive indexes and network fallbacks. Fresh recommendations need internet; cached recommendations show their age.

Run `scripts/cargo.ps1 run` for development. On a fresh clone, run `scripts/cargo.ps1 fetch` before offline checks/builds. Check with `scripts/verify.ps1`, build with `scripts/build.ps1`, package with `scripts/build-setup.ps1`. Run `node scripts/check-performance.mjs` and `node scripts/check-updates.mjs` against the workspace release executable for isolated native acceptance. Requires Windows x64, Rust/MSVC and WebView2. No Riot API key is needed for local match detection. `scripts/prepare-tools.ps1` reuses the signed runtime bootstrapper and Playwright Core package from the owner's reference for installer and native acceptance checks.

Settings live under `%LOCALAPPDATA%\com.frilo369.oracle`; installation lives under `%LOCALAPPDATA%\oracle\bin`. These are independent of Umbra. Upgrades preserve preferences, retired data and extra files in the app folder. Failed updates/uninstalls restore the previous files and registration. The tray default migrates once; a later opt-out is respected. Oracle never installs skin tooling or modifies League files. See `docs/guide-behaviour.md` for the reference opening/focus flow and `docs/validation-1.6.md` for audit fixes and acceptance results.

Version 1.7 reduces dashboard refresh data by about 99%, skips unchanged paints and stops dashboard polling while hidden or minimized. It refreshes immediately when shown. Mayhem search reuses loaded artwork while filtering. See `docs/validation-1.7.md` for measurements and validation limits.

Version 1.8 adds automatic startup updates, the Settings update check, verified GitHub downloads and installer wait/restart handling. See `docs/validation-1.8.md` for acceptance results and the release workflow.

Independent project by FrYiLo; not affiliated with Riot Games. League artwork belongs to Riot Games. Provider pages and availability are controlled by their owners. See `docs/ideas.md` for proposed future features and `docs/provenance.md` for reused source.
