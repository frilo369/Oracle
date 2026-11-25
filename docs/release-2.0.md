Fixed automatic updates for installed Windows copies.

A path separator mismatch incorrectly treated installed Oracle as a portable copy and displayed a manual-install message. Oracle now recognizes the installed app correctly and uses its existing silent updater.

**Check for updates** and startup checks automatically download and verify newer versions, wait until a match/shortcut recording finishes, then install and restart quietly. Preferences are preserved.

Versions 1.8/1.9 with the incorrect portable-copy message need the 2.0 repair once; future updates run automatically.
