# Guide behaviour — 1.4

The owner requires the reference's complete guide-opening lifecycle, including fullscreen behaviour. Reference: `C:\Users\fryil\Documents\Rust-Friloverse-Skins`, HEAD `e196315`, inspected with its existing unrelated dirty state preserved.

## Preserved from the reference

- `WM_HOTKEY` registration only while a match target exists and shortcut capture is off; 8 ms polling fallback, exact modifier matching, and the `WM_SETHOTKEY` activation helper. Keep the source's match/capture gates. A foreground-game-only check rejects the helper that Windows just activated.
- Window creation happens off the hotkey/WebView callback. A 350 ms repeat guard and opening mutex prevent duplicate creation; an F1 press during prewarm waits for that preparation.
- After the player has opened a guide once, the `guide-used` marker enables hidden preparation for the next champion/mode. The label/champion/mode cache distinguishes an already-loaded page from a different match target.
- First opening maximizes within the originating monitor's work area, with the taskbar visible. An already-visible matching guide only gets focus. A closed matching guide is shown without navigation; minimized-from-maximized restores correctly.
- Closing hides a guide and returns to its origin, validating both the window handle and owning process. Alt+Tab's single-switch correction and deliberate multi-switch behaviour come from the reference's `Switch` state machine.
- At match end, Mayhem is hidden and the separate MetaSRC browser window is destroyed. Mayhem switches champions in place, retains the catalogue, uses the last-good native/page caches and shows stale/source-error notices.
- MetaSRC receives the same factory invocation, themed loading view, early parsing, in-memory build cache, readable human-check fallback, navigation/download restrictions and request allowlist. Only MetaSRC and its Cloudflare challenge host load. Advertising/analytics hosts are refused; native command capabilities are never granted to the remote page.
- Dark native backgrounds prevent a white flash. Provider choices, filters and page position survive repeated F1 for the same guide.
- Mayhem artwork and English game JSON use the reference's read-only local asset path. `AssetArchive` retains the indexes/handles for the two default-assets WADs, invalidates them on file stamps/root changes, and bounds decoding of plain/gzip/zstd/chunked members. Discovery is cached for 2 seconds; protocol reads run in background workers and WebView images have a 60-second cache. Only known game-image namespaces/extensions are served to the bundled guide. CommunityDragon/Tencent fallbacks and the native/page last-good recommendation caches remain.

## Oracle-specific fixes and adapters

1. **Fullscreen activation regression:** 1.2 added `hotkey_context` checks. When Windows focused the helper, those checks said it was not the game and refused to open. 1.3 re-syncs the reference modules and removes those gates. The native test now proves the own-process activation path actually executes, instead of testing polling alone.
2. **Return-to-game permission:** the stronger test exposed `SetForegroundWindow` refusing the game during close, followed by Windows activating the helper and reopening the guide. Oracle reuses the reference's responsive-window focus routine for returning, temporarily links both the foreground and target input queues, detaches them immediately, and has the source-style 80 ms delayed retry. Both queues must exist; the worker uses PeekMessage to create its queue, as required by [Microsoft's AttachThreadInput documentation](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-attachthreadinput). Foreground and target responsiveness checks use 50 ms bounds.
3. **Origin bookkeeping:** `PREVIOUS` represents an external window. The guide itself is excluded alongside other Oracle windows and the Windows task switcher, so a hidden guide cannot become its own return target.
4. **Application adapters:** Oracle identity/icon, its independent data directory and Desktop state, manual guide opening, and shortcut status for the compact panel. No skin engine, LTK, patcher, party or Vanguard module is imported.
5. **Acceptance only:** a validated workspace profile gets a separate instance identifier, an owned game stand-in and fixture-only activation/focus traces. Production instance identity and profile remain unchanged. The stand-in can minimize on focus loss; it is not an actual exclusive DirectX League session.

`scripts/sync-guide-reference.mjs` records the exact three-module sync and these adapters. `scripts/guide-audit.patch` preserves the subsequent lifecycle fixes after formatting, with exact context checks before writing files. Keep both when refreshing reference code. UI and parser changes are separate and are not overwritten by the sync.

The 1.6 audit serializes tray/manual/current opens and match-end cleanup with shortcut/prewarm operations. Shortcut paths already holding the opening mutex call the internal unlocked current-guide function. An active target is checked again after loading and before showing. Hidden preparation records success only after loading, retries after capture/disabled-key/busy-lock cases with a two-second bound, and validates the current target before it starts. A second `--tray` invocation remains quiet. See `validation-1.6.md` for regressions and measured fixture timings.

`scripts/sync-guide-assets.mjs` extracts the read-only decoder and guide asset cache. It excludes the reference's WAD-writing/skin functions, party icons and chroma logic, and preserves Oracle's protocol/test adapter tail. The installed League files are never modified. No LTK dependency is introduced.

## Skill-order ranking

Mayhem exposes several priority groups and detailed level-up plans. Both lists now sort by descending reported win rate, then descending pick rate; missing rates go last. The first priority and its first detailed plan are selected by default. Options expose both rates and label the leading priority Best; the shown sequence's rates accompany the sequence. A player's explicit priority is keyed by champion/priority, not a stale array index, so refreshes do not silently switch it.

The live Arena MetaSRC skill section inspected in this phase contains one reported table and no alternative controls. Oracle preserves that reported plan; it does not invent unavailable alternatives or rates.
