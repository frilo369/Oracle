Oracle 2.4 makes the build buttons at the top of ARAM, Rift and Arena guides respond faster.

- Reuses loaded layouts, controls and artwork when returning to a build.
- Cancels superseded selections and keeps your latest choice when responses arrive out of order.
- Applies rune choices to the correct build, including while its source is still loading.
- Keeps a cached build visible if its background refresh fails; expires old session data.
- Includes the Windows installation-path fix for reliable automatic updates after installing this version.

Cached click handling measured 0.6–1.5 ms in isolated tests. First-time loads still depend on MetaSRC and any human verification.

The optional overlay remains **Beta - Overlay**, off by default. Fullscreen overlay work is paused and fullscreen visibility is not validated.

Install **Oracle-2.4-Setup.exe**. Installations already using 2.0 or later can receive this through the existing updater. Users on 1.8/1.9 should run this installer once because those versions can misidentify an installed copy and skip automatic installation. Settings and existing user data are preserved.

Unit tests, headless guide switching checks, and silent installer acceptance passed. Final live-provider and foreground visual checks were deferred.
