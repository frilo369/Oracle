# Oracle silent-update repair — 2.0

25 November 2026. Owner request: an available update must install quietly from Check for updates rather than sending the owner to a download page.

Setup: `C:\FriloCode\Oracle\dist\2.0\Oracle-2.0-Setup.exe` (7,899,648 bytes).

SHA-256: `d26d488c2bd56f079f4b03b20421fc4baeb97eb70e53a833026bcddce7b9c350`.

## Cause and fix

The actual running copy was **1.8.0** at `%LOCALAPPDATA%\oracle\bin\oracle.exe`. The installed-copy predicate compared that backslash path to `local.join("oracle/bin/oracle.exe")` as display strings. Windows accepts the forward slashes as path separators, but they remain different characters in the displayed strings. Consequently the predicate returned false for a genuine installation and stopped before downloading or launching the silent installer.

The predicate now canonicalizes paths where possible, then compares case-insensitive Windows components. Native backslashes, forward slashes, mixed case and verbatim canonical paths are accepted for the same installed executable. A different folder remains a different copy. Existing update download/digest checks, single-flight requests, match/capture deferral and silent installer/restart flow are retained.

The old test constructed its candidate using the same join expression as the implementation, masking the bug. Regression tests now use literal backslash paths and mixed separators/case, plus a real temporary executable path and its verbatim canonical form. Genuine portable copies retain their previous policy; the owner's installed copy was never portable.

## Verification

- `scripts/verify.ps1`: formatting, locked offline check, **35 Rust tests and 11 frontend tests pass**. Strict Clippy passes. Two explicit live-network/installed-assets checks remain opt-in.
- `scripts/check-update-setup.ps1 -PreviousSetup dist/1.9/Oracle-1.9-Setup.exe -Setup dist/2.0/Oracle-2.0-Setup.exe`: `.test/update-setup-b35745fb932d4b67b83b0786225ec2c3/verified.json`. Actual disposable upgrade/restart, parent wait, SHA-256, preferences/extra files, unrelated-parent rejection and forced-failure recovery pass. The installer has no visible main window; the quiet restarted fixture creates no dashboard WebView.
- `node scripts/check-updates.mjs`: `.test/updates-1795643652763/verified.json`. Startup/manual checks, offline recovery, verified automatic handoff, match/capture waits, duplicate suppression and corrupted-download rejection pass. No page errors.
- A nonzero app MainWindowHandle alone does not prove that a dashboard opened: Oracle has small native activation/single-instance helper windows. The first added assertion caught such a helper. The fixture now verifies that no Dashboard-WebView2 directory was created on the cold quiet restart instead.

## Owner-requested quiet repair

The owner requested silent updating; after the disposable checks passed, `scripts/apply-local-update.ps1 -Setup dist/2.0/Oracle-2.0-Setup.exe` applied the checked build to the real installed Oracle. This was the requested maintenance action, not a QA run against the real profile.

- League was confirmed in **EndOfGame**, with no game process, using the fixed read-only loopback gameflow endpoint. Credentials were read from the League lock only into memory; no League file was written. The lock requires cooperative ReadWrite/Delete file sharing; plain ReadAllText can fail while League holds it open.
- The local setup checksum was checked, then `/UPDATE /S /WAITPID=<installed-pid>` was launched hidden. The installed app exited normally through its own `--quit` path; setup updated it and restarted with `--tray --skip-update-once`.
- Actual version changed **1.8.0 → 2.0.0**. The installed executable hash matches the checked workspace build. The settings-file hash is unchanged. No installer or dashboard was shown.
- Result: `.test/applied-update-1795643697305.json`. Installed executable: `C:\Users\fryil\AppData\Local\oracle\bin\oracle.exe`. A later read-only process check confirms the restarted 2.0.0 process.

The repair helper requires PowerShell 7, only targets the standard per-user install, removes QA-specific environment overrides from child processes, and refuses an active or unconfirmed League state. Its `-CheckOnly` mode performs read-only readiness inspection. Normal QA continues to use disposable fixture helpers only.

Version 2.0 was built and applied locally. This request did not authorize a new push/public release; release notes are prepared in `docs/release-2.0.md` for a later owner-authorized publication.
