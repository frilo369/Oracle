# Oracle dashboard redesign — 1.9

25 November 2026. Owner request: recreate the attached dashboard reference. Reference retained at `docs/design/oracle-reference.png`. The picture is a visual reference; live connection/match states and enabled controls reflect actual app state.

Setup: `C:\FriloCode\Oracle\dist\1.9\Oracle-1.9-Setup.exe` (7,897,600 bytes).

SHA-256: `797a262c8c9ff6a1b849eddb32413553a9df11e0191549f520a376c3d2e3cea7`.

App: `C:\FriloCode\Oracle\dist\1.9\Oracle.exe`.

## Design

- Wider 4:3 composition: 1040×780 logical pixels by default, minimum 720×570. This replaces the owner's earlier compact dashboard layout for the new reference. The theme scales with the available window dimensions.
- Gold chamfered double frames, ruby glass buttons with static highlights, the diamond tab indicator, Cinzel headings and serif secondary text follow the supplied reference. The hero has an eye-and-ruby portrait, gold divider, match information and a large guide action. Shortcut, manual picker and footer retain their working controls.
- Settings uses the same frames and typography; startup preferences, accent control, shortcut toggle and manual update check remain present. Gold trim stays gold; the chosen accent still influences ruby materials and guide appearance.
- The eclipse backdrop is a separate compressed image. Actual text, frames, controls, icons and states are native HTML/CSS/SVG rather than a flattened screenshot. There are no continuous animations or background design timers. Provider guide layouts and the guide/focus engine were not restyled.
- Current match portrait errors fall back to the new native SVG eye portrait; failed remote portrait addresses retain their existing retry suppression.

## Artwork and prompt

Mode: built-in **imagegen** skill/tool, using the owner's screenshot as the editing reference. The resulting PNG was encoded to WebP at quality 92 without resizing. The original generated PNG remains in Codex's generated-image storage.

Workspace artwork: `C:\FriloCode\Oracle\src\ui\backgrounds\oracle-eclipse.webp` (1448×1086, **55,822 bytes**).

Native UI artwork:

- `C:\FriloCode\Oracle\src\ui\oracle-mark.svg`
- `C:\FriloCode\Oracle\src\ui\oracle-portrait.svg`
- `C:\FriloCode\Oracle\src\ui\designs\oracle-panel.svg`
- `C:\FriloCode\Oracle\src\ui\designs\oracle-button.svg`
- `C:\FriloCode\Oracle\src\ui\designs\oracle-chip.svg`

Final generation prompt:

> Edit this reference into the underlying ARTWORK ONLY for a desktop app background, landscape 4:3 composition. Remove ALL UI: every word, text, letters, logos, icons, buttons, panels, rectangular borders, dividers, controls and ornaments. Reconstruct the artwork unobstructed under those removed elements. Closely preserve the reference's exact aesthetic, restrained luminosity and composition: near-black charcoal-violet fantasy atmosphere, a black eclipse with a thin hot red-orange upper-left rim centered about 76% across and 12% down, dim smoky red-purple clouds around it, extremely dark center and left with almost no details where readable controls will sit. In the right third at about 28% down, preserve the dim ruined gothic circular portal and distant spires, fine crimson light down its center. Bottom edge has jagged dark mountains, with elegant thin red magical silk light ribbons flowing diagonally from bottom left toward lower right and a few subtle ribbons at 43% height. Keep most of the image very dark, sophisticated and matte; crimson accents glow softly, do not flood the whole scene red. Preserve the image's dark burgundy/black palette and photographic painterly fantasy detail. No text, no gold lines, no eye symbol, no UI, no cards, no frames, no watermarks. This is a background asset, NOT a finished app screenshot.

## Verification

- `scripts/verify.ps1`: formatting, locked offline check, **34 Rust tests and 11 frontend tests pass**. Two existing explicit live tests remain opt-in. Strict Clippy, JavaScript syntax, guarded reference adapters and Git whitespace checks pass.
- `node scripts/check-design.mjs`: `.test/design-1795642890116/verified.json`. Actual WebView2/native windows at 1040×780, 720×570 and 1280×720. Overview/Settings have no page scroll, horizontal overflow, clipped labels or out-of-window controls. Active/inactive matches, a long Ctrl+Alt+Shift+Super+F24 binding, reset, startup preference controls and the manual update check pass. No page errors. Screenshots were visually reviewed against the reference.
- Windows rounds 570 logical pixels to a 571-pixel WebView viewport at this PC's 125% scale; allow a one-pixel size tolerance. Decorative tab highlights intentionally extend beyond button boxes, so text clipping checks use text range bounds rather than an element's scrollWidth.
- Initial Settings content exceeded the available height slightly. Reduced row padding and bottom spacing; both default/minimum layouts now fit. Header and inactive-match portraits use separate SVG variants to match their distinct reference emblems. Background artwork anchors at the top so wider windows retain the eclipse.
- `node scripts/check-native.mjs`: `.test/native-1795642985289/verified.json`. Full capture/save/manual-selection/prewarm/shortcut/reopen/restore/focus/activation/persistence flow passes. Two earlier attempts hit the recorded Windows foreground-helper/return flake; traces are retained in `.test/native-1795642814337` and `.test/native-1795642897803`. Product focus code was unchanged.
- `node scripts/check-updates.mjs`: `.test/updates-1795642825024/verified.json`. Startup/manual checks, error recovery, deferred installation, duplicate suppression and digest verification continue to pass.
- `node scripts/check-performance.mjs`: `.test/performance-1795642931561/verified.json`. Unchanged dashboard state produces zero body mutations/colour events. Hidden/minimized dashboards make zero refresh requests; resume was 36 ms. Mayhem search retains all 109 existing row nodes; accent propagation passes. No page errors.
- `scripts/check-setup.ps1 -Setup dist/1.9/Oracle-1.9-Setup.exe`: `.test/installer-59bb6cc36a45496b97590dc1de6ffe69/verified.json`. Payload integrity, disposable install/reinstall/uninstall and extra-file preservation pass. The packaged runtime bootstrapper retains a valid Microsoft signature.

Only disposable `.test` profiles/installations were used. Actual exclusive-fullscreen League and ARAM/Rift human checks remain open from earlier phases. The redesign phase built version 1.9 locally; the owner subsequently authorized its publication with a follow-up “publish” request. The real installed app was not changed by verification.

Publication uses `scripts/publish-release.ps1 -NotesFile docs/release-1.9.md`, with the built installer's recorded SHA-256 digest verified against GitHub before publishing. After publication, the opt-in `live_github_release_download` test checks the actual public latest-release API and installer using Oracle's native updater parser, HTTP policy and SHA-256 verifier. The result is retained in `.test/publication-1.9.json`.
