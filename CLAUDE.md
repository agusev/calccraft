# CLAUDE.md

@AGENTS.md

## Notes for Claude Code

- `AGENTS.md` (imported above) is the single source of truth shared with other agents.
  Put project guidance there, not here. This file is only for Claude-specific workflow.
- On Windows you can verify UI changes with computer use. Launch `target\debug\calculator.exe`
  and the real calculator (`calc.exe`), then compare them side by side with screenshots.
- Before sending keystrokes to the app from a script (`SendKeys` and similar), check that the
  calculator window really is in the foreground, and stop if it isn't. Otherwise the keys land
  in whatever window the user is working in.
- To refresh `docs/screenshot.png`, capture the window with `PrintWindow` (flag
  `PW_RENDERFULLCONTENT`), cropped to the DWM frame bounds. A screen capture would also pick up
  overlapping windows and on-screen overlays.
