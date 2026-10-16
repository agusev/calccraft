# AGENTS.md

Guidance for AI coding agents (Claude Code, Codex, Copilot, Cursor, …) and human contributors.
Read this before changing code. Agent-specific files (such as `CLAUDE.md`) only add to it.

## What this project is

A clone of the Windows 11 Calculator's **Standard mode**, written in Rust with eframe/egui.
**The real Windows Calculator is the specification.** Visual style and behavior should match it,
and the user-facing changes that are welcome are the ones that bring the app closer to the original.

## Commands

| Task | Command |
| --- | --- |
| Build | `cargo build` |
| Run (with a console for logs) | `cargo run` |
| Run (like an end user) | `cargo run --release` |
| Tests | `cargo test` |
| Format | `cargo fmt` |
| Lint | `cargo clippy --all-targets -- -D warnings` |

**Definition of done:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
`cargo test` all pass, and UI changes have been checked by actually running the app
(see "Verifying UI changes"). CI (`.github/workflows/ci.yml`) runs the same three commands on
`windows-latest` for every push and pull request, and a pull request isn't done until it passes.

## Architecture

```
src/main.rs    window options, window icon, eframe::run_native
src/engine.rs  calculator logic, no UI dependencies, unit tests at the bottom
src/app.rs     egui user interface (custom painting), keyboard, fonts, title bar
```

### `engine.rs`

- `Calculator` is the whole state. The UI only calls `execute(Command)`, the memory and
  history methods, `paste`, and the read-only getters (`display`, `expression`, `is_enabled`,
  `memory`, `history`, `copy_text`).
- Input is a small state machine (`Phase`): `Typing`, `Operand` (finished number, replaced
  by the next digit), `AwaitingOperand` (operator just pressed) and `Result` (`=` just pressed).
  Most behavior questions boil down to "what does this key do in each phase".
- Numbers are `BigRational` (`Number`). `normalize` checks every result for overflow and
  rounds results with huge denominators to 40 significant digits. `sqrt` is exact for
  perfect squares.
- `format_number` gives display text (with grouping). `expression_number` gives
  expression-line text (no grouping).
- Tests use the `run("2+3*4=")` helper, a mini key language described above `press` in the
  test module. It returns `(display, expression)`.

### `app.rs`

- Everything is painted by hand. `button_flash` draws every key, and the layout is computed
  from the window rect in `App::ui`. The only stock egui widget is the `ScrollArea` in the
  history and memory panels.
- Color constants at the top were sampled from a Windows 11 screenshot. Reuse them; don't
  add new color literals in drawing code.
- Icons are Segoe Fluent Icons code points (`ICON_*`) drawn through `CalculatorApp::icon`,
  which falls back to plain text when the icon font is missing.
- Keyboard: `char_command` handles text input (digits, operators), and `handle_key` handles
  named keys and `Ctrl` shortcuts.

## Windows behaviors the tests pin down

All of these were checked against the real Windows 11 Calculator. Don't "fix" them:

- Immediate execution with no operator precedence: `2 + 3 × 4 =` → `20`.
- Chained operators collapse in the expression line: `2 + 3 +` shows `5 +`.
- Pressing another operator replaces the pending one.
- `=` with no right operand reuses the display: `5 + =` → `5 + 5 =`.
- Repeated `=` repeats the last operation. A new number followed by `=` also does:
  `2 + 3 = 7 =` → `7 + 3 =` → `10`.
- `%`: after `+` or `−` it gives `left × value / 100`; after `×` or `÷` it gives `value / 100`;
  with no pending operator it gives `0`.
- Functions appear as `sqr(x)`, `√(x)`, `1/(x)` and `negate(x)`, and nest.
- `±` while typing flips the sign silently; on a finished number it shows `negate(x)`.
- The display uses 16 significant digits with en-US grouping (`1,234,567`). The expression
  line has no grouping (`1234567 +`).
- Numbers ≥ 1e16 use scientific notation: `1.e+16`, `1.234567890123456e+18`.
- Numbers < 1 use plain notation if there are at most 2 zeros after the point (then they are
  rounded to 16 decimals, as in `0.0033333333333333`) or if they fit exactly in 16 decimals
  (`0.000125`). Otherwise they use scientific notation (`3.333333333333333e-4`).
- Errors:
  - `Cannot divide by zero`: the expression shows `1 ÷` after `=`, and `8 ÷ 0 +` after an operator.
  - `Result is undefined`: `0 ÷ 0`.
  - `Invalid input`: the square root of a negative number.
  - `Overflow`: the decimal exponent goes beyond ±9999.

  In the error state only digits, `.`, `C`, `CE` and `⌫` work.
- `CE` after `=` clears everything. `⌫` after `=` clears only the expression line.

When you change or add behavior, check it against the real calculator (`calc.exe`) first,
then add or update a test in `engine.rs`.

## Conventions

- **English everywhere:** code, comments, docs, UI strings, commit messages and PR text.
- Keep `engine.rs` free of egui and other UI code. Any new calculator behavior needs a test.
- Comments explain *why*, especially Windows quirks. Public items get `///` doc comments.
- Code is formatted with `rustfmt` (default settings). The keypad table `KEYS` is marked
  `#[rustfmt::skip]` so it keeps its 4-column grid.
- Prefer small, focused changes. Don't reformat or restructure code you aren't changing.

## Pitfalls

- **`num-bigint` must stay on 0.4.x.** `num-rational` 0.4 depends on it, and `BigRational`'s
  integer type has to match ours. `cargo add num-bigint` without a version picks 0.5 and breaks the build.
- **egui 0.36 API:** the app implements `App::ui(&mut self, ui, frame)` (not `update`). Buttons
  use `Sense::CLICK` *without* `FOCUSABLE`. Otherwise a clicked button keeps keyboard focus,
  and pressing `Enter` would press it again on top of `=`.
- The font families `semibold` and `icons` must always be registered in `setup_fonts`, even
  when the font files are missing: egui panics on unknown font families.
- The title bar tint (`tint_title_bar`) calls `DwmSetWindowAttribute` and exists only under
  `#[cfg(windows)]`.
- Release builds use `windows_subsystem = "windows"`, so `println!` output goes nowhere.
  Debug with `cargo run`.

## Verifying UI changes

Unit tests don't cover the UI. After visual or interaction changes:

1. Run the app (`cargo run`) and click through what you changed: hover, press, disabled
   states (enter `1/0=` for the error state), the history and memory panels, the menu,
   keep-on-top mode, and resizing.
2. Compare it side by side with the real Windows Calculator.
3. If the look changed, update `docs/screenshot.png`.

## Git and pull requests

- Work on a branch and open a pull request against the default branch.
- Write commit subjects in the imperative mood (`Add memory list flyout`), and keep each
  commit to one topic.
- In the PR description, say what changed, why, and how you verified it (tests and manual
  checks). Include before/after screenshots for UI changes.
- Pushing a `v*` tag publishes a GitHub release (`.github/workflows/release.yml`, see "Releasing"
  in the README). The tag must match `version` in `Cargo.toml`. Don't create or push tags unless
  a maintainer asks.
