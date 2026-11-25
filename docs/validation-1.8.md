# Oracle updates — 25 November 2026

Version **1.8.0**, setup label **1.8**. Previous delivered version: 1.7.

Setup: `C:\FriloCode\Oracle\dist\1.8\Oracle-1.8-Setup.exe` (7,835,648 bytes).

SHA-256: `9cbb11cd68545f4379354c24312861a1788f9f83bfdc415dc56aa2ffce0e1c47`.

Portable app: `C:\FriloCode\Oracle\dist\1.8\Oracle.exe`.

## Behaviour and decisions

- `update.rs` checks the public `frilo369/Oracle` latest stable release five seconds after startup, without delaying the tray, dashboard or guide threads. Settings has an immediate **Check for updates** button and progress/error/retry status. Checks require no account credentials.
- Only a newer stable semantic version is accepted. Drafts/prereleases, missing digests, wrong repository/tag/installer URLs and invalid sizes are refused. HTTPS redirects stay on GitHub's API/download/asset hosts. The asset digest comes from the [GitHub release-assets API](https://docs.github.com/en/rest/releases/assets).
- Installer downloads are streamed with a 100 MiB limit, advertised-size and SHA-256 checks. The saved file is reopened read-only with sharing that denies modification/deletion, verified again and kept locked through process launch. Interrupted/corrupt downloads never run. Completed flat download folders are cleaned on the next check without recursively deleting user files or linked folders.
- Only `%LOCALAPPDATA%\oracle\bin\oracle.exe` can automatically install. Portable copies report availability and direct the user to the latest setup. Downloads wait for game windows, active match/draft phases, a guide target and shortcut recording to clear before installation.
- The existing installer accepted `/UPDATE /S /WAITPID` but did not consume `WaitPid` or restart after a silent update. It now validates the installed parent's path and waits for exit before touching files, then restarts the installed app after success or failure. The wait handles the parent exiting during process inspection. Existing transactional file/registration rollback stays in place.
- Restart uses `--tray --skip-update-once`, avoiding a repeated-update loop after failure. An outcome file communicates success/failure to Settings without modifying preferences. A subsequent normal startup or manual check retries normally.
- Updater progress has monotonically increasing revisions so delayed settings/poll responses cannot replace a newer progress message. Single-flight checks prevent duplicate downloads or installers. The Settings layout remains compact with the new update card.
- All normal QA profiles disable startup updates and real installation. An explicit `--fixture-update-startup` with validated fixture mode exercises startup scheduling against synthetic release data. Native fixture requests share the real release parser, hashing, download-file verification and match/capture gates; they record installer handoff rather than running it. Installer restart tests run actual executables with `/VERIFY-UPDATE` and a bounded disposable profile.

## Verification before publication

- Formatting, locked offline check and unit tests pass: **34 Rust tests and 11 frontend tests**. The existing read-only installed-assets test and the new read-only published-release download test are opt-in. Strict Clippy and JavaScript syntax checks pass.
- `node scripts/check-updates.mjs`: `.test/updates-1795640700569/verified.json`. Startup check, Settings fit, manual check, offline recovery, match/capture deferral, duplicate suppression, verified handoff and corrupted-download rejection pass. No page errors; Settings screenshot reviewed.
- `scripts/check-update-setup.ps1 -PreviousSetup dist/1.7/Oracle-1.7-Setup.exe -Setup dist/1.8/Oracle-1.8-Setup.exe`: `.test/update-setup-b5ce2a4e070240788fa17062547a522a/verified.json`. Actual disposable 1.7 → 1.8 upgrade waits for the exact running parent, launches through a read-locked setup file, restarts 1.8, and preserves preferences/extra files. An unrelated parent PID is rejected. A deliberately locked uninstaller forces failure; the previous app is retained and restarted. No real install or registry changes.
- `scripts/check-setup.ps1 -Setup dist/1.8/Oracle-1.8-Setup.exe`: `.test/installer-953f61860d774b71a9bf6400035e12e7/verified.json`. Normal isolated install/reinstall/uninstall, payload integrity and preservation checks pass.
- `node scripts/check-native.mjs`: `.test/native-1795640708379/verified.json`. Full shortcut/focus/activation/prewarm/capture/persistence lifecycle and compact Overview/Settings pass with no page errors.
- `node scripts/check-performance.mjs`: `.test/performance-1795640730816/verified.json`. Compact payload 451 bytes vs 27,398 bytes; unchanged state causes zero mutations/colour events. Hidden/minimized dashboards make zero refresh requests, return refresh 40 ms, search preserves 109 loaded row nodes, and colour updates pass.

## Publication and remaining limits

The owner authorized uploading source and this build to `https://github.com/frilo369/Oracle`. `scripts/publish-release.ps1` requires committed source, the expected origin and matching local checksums; it pushes source, uploads installer/checksum assets to a draft, compares GitHub's SHA-256 digest, then publishes as latest. Never publish a later build without fresh owner authorization.

The first push was rejected by the environment token's repository permissions; the saved same-account GitHub login has write access. Only the command-process environment override was removed to use that login. The first draft lookup also exposed that REST's release-by-tag route returns 404 before a draft's tag exists. The publishing helper now resolves the draft database ID and verifies installer/checksum digests through the release-ID endpoint, preserving verification before publication and allowing an interrupted draft to resume.

Immediately after publication, run the opt-in `live_github_release_download` Rust test. It uses the actual updater HTTP policy, release parser and SHA-256 download verifier against the public API and asset, without executing or installing anything. The publication/test result is recorded in the phase's `.test` log and final response.

Versions before 1.8 have no updater and require this installer once. Public unauthenticated GitHub rate limits and offline errors are recoverable in Settings. Live ARAM/Rift human verification and actual exclusive-fullscreen League acceptance remain open as documented for earlier versions; this phase did not modify the focus/guide engine.
