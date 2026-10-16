# CLAUDE.md

@AGENTS.md

## Notes for Claude Code

- `AGENTS.md` (imported above) is the single source of truth shared with other agents.
  Put project guidance there, not here. This file is only for Claude-specific workflow.
- On Windows you can verify UI changes with computer use. Launch `target\debug\calculator.exe`
  and the real calculator (`calc.exe`), then compare them side by side with screenshots.
  Start the app with `Start-Process` from the PowerShell tool itself: one started from a child
  `pwsh -File` script is ended with that script. Computer use's `type` sends text, not key
  presses, so letter shortcuts (`S` for sin, `Ctrl+S`, …) need its `key` action. Copying a
  result (`Ctrl+C`) and reading the clipboard compares long numbers digit by digit.
- Before sending keystrokes to the app from a script (`SendKeys` and similar), check that the
  calculator window really is in the foreground, and stop if it isn't. Otherwise the keys land
  in whatever window the user is working in.
- To refresh `docs/screenshot.png`, capture the window with `PrintWindow` (flag
  `PW_RENDERFULLCONTENT`), cropped to the DWM frame bounds. A screen capture would also pick up
  overlapping windows and on-screen overlays.
