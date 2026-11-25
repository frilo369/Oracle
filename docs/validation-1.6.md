# Oracle audit — 25 November 2026

Delivered version: **1.6.0**, setup label **1.6**. Version 1.5 was an internal acceptance build; the previous delivered version was 1.4.

Setup: `C:\FriloCode\Oracle\dist\1.6\Oracle-1.6-Setup.exe` (7,777,792 bytes).

SHA-256: `df6256a911b5ab1b618508b2d325a9602e6a06b32a42440af1f61480f7098e55`.

Portable app: `C:\FriloCode\Oracle\dist\1.6\Oracle.exe`.

## Findings fixed

| Area | Bug and resulting fix |
| --- | --- |
| Reinstall, high impact | Replacing the entire bin directory deleted unrecognized user files in its backup. Preserve those files and directories during replacement, and move them back if replacement fails. |
| Uninstall, high impact | A locked Uninstall.exe could leave oracle.exe deleted after a failed uninstall. Stage managed files transactionally; restore them on failure. Unknown user files remain. |
| Installer registration | File rollback did not restore registry metadata or the Start menu shortcut after a later registration failure. Snapshot and restore exact registry values/types and shortcut bytes. Stop rewriting an unrelated Oracle startup entry during installation. |
| Installer failure handling | Reject downgrades before closing the app or installing prerequisites. Cleanup failures retain their path in the log and do not mask the primary failure or turn a committed install into a reported failure. |
| Match state | The next draft could inherit the last match's mode; draft queue information was ignored. Reset finished-match state, preserve a known lobby queue, accept draft queues, and let explicit gameflow queues take precedence. |
| Player identity | Conflicting PUUIDs could match via a shared legacy ID, and the cached current summoner survived account changes. Prefer PUUID, refresh at a new draft or unmatched active roster, and reset state on a changed client connection. |
| League event connection | Unchanged four-second discovery results notified the watch channel and restarted its WebSocket. Publish actual changes only. |
| Shortcut recording | Cancel/tab/blur actions could be dropped while starting native capture. Serialize desired capture state, reflect cancellation immediately, and finish the native cancellation after the pending start. Retry after errors and prevent duplicate key saves. |
| UI state | A poll started before a completed save could overwrite the UI with old preferences. Reject snapshots from an earlier revision. Dismissing an error no longer replaces all preferences. A manual-open click retains its champion/mode while saving. Failed portrait URLs are not retried on every repaint. |
| Guide lifecycle | Tray/current-guide opens were outside the opening mutex; match cleanup could race window creation. Serialize these operations, recheck an active target before showing it, and let already-locked shortcut paths use an internal unlocked function. Preserve the fullscreen helper's focus rules. |
| Hidden preparation | A disabled key, active capture, busy lock or failed load was marked permanently prepared. Mark only successful preparation, retry with a two-second bound, avoid duplicate workers, and revalidate the target/settings before loading. |
| Windows startup | Normal disposable profiles could change the real Run entry because only fixture mode was excluded. Isolate every validated QA profile. Roll back a failed preference save to the exact previous raw startup value; disabling startup removes only this executable's command. A second --tray launch stays quiet. |
| Maintenance | Strict Clippy found needless borrows and a helper after the test module. Fix these and preserve the audited native changes through a guarded reference-sync patch. |

## Verification

- `scripts/verify.ps1`: formatting, locked offline check, **28 Rust tests and 9 frontend tests pass**. The explicit installed-asset Rust test remains opt-in; this phase exercised the actual image protocol instead.
- `cargo clippy --locked --offline --all-targets -- -D warnings`: passes. Use native cargo with a quoted separator, or pass an explicit argument array to the PowerShell wrapper.
- Native Windows/WebView2: `.test/native-1795638081091/verified.json`. Delayed capture/tab cancellation, a changed picker during delayed saving, quiet second tray launch, hidden preparation after capture ends, compact UI, retired-page removal, preference persistence, exact modifier/capture/end suppression, global-conflict F1, custom Ctrl+G, actual activation helper, minimize-on-blur stand-in, return focus and no-reload reopening/restoring pass. No page errors.
- Fixture timings: hidden preparation resumed in 133 ms; prepared F1 128 ms; refocus 129 ms; reopen 63 ms; restore 129 ms; activation helper 144 ms; fullscreen-style F1 126 ms. These measure the native fixture flow, not provider latency.
- Actual installed League image: `.test/live-assets-1795638114281/verified.json`. Normal isolated profile, PNG 128×128, HTTP 200, private 60-second cache. Toggling its startup preference leaves the real Windows startup entry unchanged. League archives were only read.
- Live providers: `.test/meta-1795638116781/verified.json`. Ahri Arena renders seven cards/227 augments. Remote commands are denied in all three MetaSRC modes. ARAM and Rift require human verification and were skipped. Live Tencent Annie/Ahri responses dated 25 November renderable with 115/138 augments.
- Final installer: `.test/installer-85ec2a8052b5402cb34d0fc40ac3ad81/verified.json`. Payload hash, install/reinstall/uninstall and preservation of a root sentinel, a file inside bin and a nested directory pass.
- Installer UI/faults: `.test/setup-ui-1c2be3b85a6b4ea9a46d4c009bf232ab`. Install/uninstall pass. Locked main-executable reinstall fails safely. A deliberate failure after replacement restores the old folder and its extra files. Locking Uninstall.exe restores oracle.exe after failed uninstall. Registration snapshots restore values/types and shortcut bytes in a private temporary test key, never the real app registration. The key is removed afterward.
- App/setup screenshots reviewed. The Microsoft runtime bootstrapper signature was verified during packaging. `sync-guide-reference.mjs --check`, JS syntax and Git whitespace checks pass.

## Acceptance limits

An actual exclusive-fullscreen League match was not played. Native activation and a stand-in that minimizes on focus loss pass; final acceptance inside an actual League session remains open. ARAM/Rift human verification was not automated or bypassed. No real installation, push, publication or deployment was performed. Only disposable install/profile paths and a temporary test registry key were written.

## Maintenance notes

- Keep `open`/`manual_open` serialized; shortcut callbacks already own the mutex and must call `open_current` to avoid locking it twice.
- `scripts/guide-audit.patch` applies after formatting the reference adapters. Update that patch deliberately if the audited native functions change. Exact context checks reject an incompatible source refresh before writing anything.
- Tauri's exported core module is frozen. Fault-injection tests replace `window.__TAURI__.core` with a wrapped object; assigning `core.invoke` silently fails in non-strict browser evaluation.
