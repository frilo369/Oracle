# Oracle

A Windows system tray companion for League of Legends. Press **F1** in a supported match to open your champion's analytics. Change the key in Settings. Closing Oracle's main window keeps the tray companion running; choose **Quit Oracle** from its tray menu to exit.

Guides: Summoner's Rift, ARAM and Arena through the inherited themed MetaSRC layout; ARAM Mayhem through the inherited Tencent build guide. Includes a champion browser, favorites and private notes saved on this PC. A guide needs internet for fresh data; Mayhem can reuse previously cached recommendations with their age shown.

Run `scripts/cargo.ps1 run` for development. Check with `scripts/verify.ps1`, build with `scripts/build.ps1`, package with `scripts/build-setup.ps1`. Requires Windows x64, Rust/MSVC and WebView2. No Riot API key is needed for local match detection. `scripts/prepare-tools.ps1` reuses the signed runtime bootstrapper and Playwright Core package from the owner's reference for installer and native acceptance checks.

Settings live under `%LOCALAPPDATA%\com.frilo369.oracle`; installation lives under `%LOCALAPPDATA%\oracle\bin`. These are independent of Umbra. Installer upgrades preserve settings and notebook. Oracle never installs skin tooling or modifies League files.

Independent project by FrYiLo; not affiliated with Riot Games. League artwork belongs to Riot Games. Provider pages and availability are controlled by their owners. See `docs/ideas.md` for proposed future features and `docs/provenance.md` for reused source.
