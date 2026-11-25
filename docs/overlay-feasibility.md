# Augment-selection overlay feasibility

25 November 2026. Owner asked whether Oracle could automatically show win/pick percentages under offered augments during a fullscreen match. This records the initial investigation. The subsequent start instruction led to the [2.2 Mayhem beta](augment-overlay.md); the original investigation itself did not change application behaviour.

## Findings

- A transparent, non-activating, click-through desktop overlay is a feasible direction for borderless fullscreen. Labels should appear only during augment selection, follow rerolls and disappear after selection, without taking keyboard/mouse focus from League.
- True exclusive fullscreen is a separate case. Microsoft's [Fullscreen Optimizations explanation](https://devblogs.microsoft.com/directx/demystifying-full-screen-optimizations/) distinguishes desktop-composited borderless/optimized fullscreen from exclusive ownership of the display. An ordinary topmost window is not a reliable promise for genuine exclusive fullscreen. Validate the actual League display mode before committing to this behaviour.
- Riot's current [League developer policy](https://developer.riotgames.com/docs/lol#_game-policy) explicitly lists displaying win rates for augments or Arena-mode items among unapproved use cases, applying to websites, applications and overlays. Do not describe an augment-win-rate overlay as Riot-approved. Static aggregate overlays are listed as an approved use-case category, subject to the other policies and registration.
- Oracle's Mayhem parser currently extracts augment pick rate, grade and ranking; it does not extract individual augment win rate. Equipment/skill/champion win rates elsewhere in a guide are different data and cannot be relabelled as augment win rates.
- Riot's documented Live Client Data API does not list an endpoint for the currently offered augment choices. Before implementation, validate whether an appropriate read-only signal exists; otherwise investigate local screen recognition of the visible cards. Do not promise reliable automatic selection detection before testing it.

## Proposed first prototype

Confirm whether the owner means Mayhem, Arena or both, and whether borderless fullscreen is acceptable. Prototype the overlay using cached champion-specific pick rates and reported tiers/descriptions. Keep screen processing local, show nothing on an uncertain match, handle rerolls, allow a settings toggle and test resolution/UI scaling and multiple monitors. Existing F1 guides remain available.

No application behaviour, League files, installed app, build version or published release changed during this investigation.
