# Oracle 2.4: faster build switching

Local build prepared 26 November 2026. No push, publication, installation into the owner's real profile, League capture, or overlay development was performed.

## Changes

The MetaSRC build buttons previously rebuilt their controls and standard guide content on every cached switch. Arena repeatedly updated and reordered hundreds of recommendations. Cached selections also continued waiting on the provider, could revert after a failed refresh, and could be overwritten by older responses.

- Reuse the top buttons, champion selector, unchanged header/equipment markup, tooltip identities and existing layout nodes. Keep up to six standard layouts or three Arena layouts, with decoded artwork, in memory.
- Retain up to 20 parsed models for 15 minutes, scoped to exact build URL and patch. Refreshes invalidate changed content; expired models are not shown as current recommendations.
- Show a cached selection synchronously. A failed source refresh leaves it displayed with an explicit notice.
- Cancel superseded selections with the provider's own [HTMX abort event](https://htmx.org/events/#htmx:abort), except while its human check is visible. Track the current request so a cancelled request's late error cannot fail a newer selection.
- Ignore stale responses until the latest selected URL is available. Explicit history navigation resets that choice.
- Defer rune clicks until the displayed build's source has arrived. Clear or abort context-bound tooltip/rate requests when changing builds.
- Read requested source swaps in the mutation callback rather than adding another 16 ms timer. Keep background mutation coalescing.
- Preserve focus on existing buttons rather than forcing a synchronous layout/scroll after every cached switch.

The caching is bounded and session-only; it does not persist provider HTML, execute downloaded content, or request extra builds in the background. Provider links, filters, challenge handling and native command restrictions remain in use.

## Measurements

`node scripts/check-build-switches.mjs` now runs in **headless Edge by default**, using isolated synthetic source pages and the real guide renderer/styles. It does not open or focus a visible window. `--native` explicitly runs in disposable Oracle WebView2 and opens a test dashboard; do not use it while the owner is playing. `--baseline` loads the renderer from commit `86a4cd6` for comparison.

Each mode warms three builds, then switches 60 times. Standard fixtures include 172 champions, rune alternatives and dozens of equipment options. Arena has 227 augments and 90 prismatic items, with grades/order differing between builds. Artwork is fulfilled locally to exclude provider network timing.

Comparable headless runs on this PC:

| Mode | Previous median click handler | 2.4 median click handler | Previous added nodes / 60 clicks | 2.4 added nodes |
| --- | ---: | ---: | ---: | ---: |
| ARAM | 7.3 ms | 0.7 ms | 11,160 | 120 |
| Rift | 7.9 ms | 0.6 ms | 11,160 | 120 |
| Arena | 12.4 ms | 1.5 ms | 93,960 | 120 |

Final handler p95: 1.1 / 1.2 / 2.9 ms. These are JavaScript click-handler timings, **not full network-to-display timings**. The next animation-frame callback medians were 7.4 / 7.3 / 19.9 ms respectively; p95 was 10.7 / 12.7 / 27.9 ms. This callback is a rendering opportunity rather than proof of a completed GPU presentation. Arena still costs more to lay out and paint. Timings vary with system/game load.

Final headless evidence: `.test/build-switches-1795652329012/verified.json` and per-mode screenshots. Baseline: `.test/build-switches-1795652275822/verified.json`. Intermediate source also passed actual WebView2 fixtures before the owner requested no focus changes; the final Arena layout cache and request-cancellation changes were verified headlessly afterward.

Fixture correction: early runs omitted the native appearance options and inherited undefined colours inside the frame. Both final comparison runs set the real `umbra` theme/ruby colour; final screenshots were inspected with those materials enabled. Keep the renderer options aligned with `match_guide.rs` when extending this check.

## Verification

- Syntax checks and Rust formatting/check passed.
- Frontend unit tests: 11 passed.
- Rust unit tests: 44 passed, 3 explicitly live/interactive tests ignored.
- Final headless checks passed in all three modes: correct recommendations per build, button/layout reuse, latest selection after stale responses, obsolete request cancellation, old error isolation, queued rune choices, failed cached-refresh retention, patch refresh, cache expiry, restored tooltips, item-name preference, and preserved human-check waiting/cancellation behavior. Human-check behavior uses a fixture class/event, never a real verification challenge.
- Release build 2.4.0 compiled successfully.
- Silent installer acceptance passed: install, exact payload hash, reinstall, uninstall, and preservation of unknown application files/user data. Evidence: `.test/installer-bc28b750404a4394b8b60dd05a691618/verified.json`.

First-time build loads still depend on MetaSRC and any human verification. Final live-provider switching and foreground/native visual acceptance were deferred to honor the owner's request to avoid interrupting their game. The paused overlay remains **Beta - Overlay**, default off.

## Local artifacts

- `C:\FriloCode\Oracle\dist\2.4\Oracle-2.4-Setup.exe` (8,080,384 bytes)
- `C:\FriloCode\Oracle\dist\2.4\Oracle.exe`
- `C:\FriloCode\Oracle\dist\2.4\SHA256SUMS.txt`
- Setup SHA-256: `f57f1779d4bc988ed19d996d738532b3e437b648231818e7aeeeaa5dfec7b4a0`

The installed Oracle was left running and unchanged. Publication requires fresh owner authorization.
