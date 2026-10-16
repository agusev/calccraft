# AGENTS.md

Guidance for AI coding agents (Claude Code, Codex, Copilot, Cursor, …) and human contributors.
Read this before changing code. Agent-specific files (such as `CLAUDE.md`) only add to it.

## What this project is

A clone of the Windows 11 Calculator's **Standard** and **Date calculation** modes, written in
Rust with eframe/egui.
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
src/main.rs              window options, window icon, eframe::run_native
src/engine.rs            calculator logic, no UI dependencies, unit tests at the bottom
src/dates.rs             date calculation logic (chrono), no UI dependencies, unit tests
src/app.rs               egui user interface (custom painting), modes, menu, keyboard, fonts, title bar
src/app/date_view.rs     the date calculation mode's UI (a child module of app.rs)
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
- All colors come from `Palette` in `app/palette.rs`, which has a `DARK` and a `LIGHT` constant.
  `App::ui` picks one from `ctx.theme()` at the start of every frame. Drawing methods read it
  with `let p = self.palette;`, and free functions take `p: &Palette`. Don't put color
  literals in drawing code: add a field to `Palette` and fill it in both themes.
- Keypad colors follow the opacity table in the Windows Calculator's `App.xaml`: white at a fixed
  opacity over the background, computed with `white()` / `black()` / `over()`. Examples: number
  keys are 12.5 % in dark and 100 % in light; operators are 8.52 % in dark and 50 % in light.
  A unit test checks that the dark values still match the colors sampled from the reference
  screenshot.
- The App theme choice (`Theme`) maps to egui's `ThemePreference`. With `System`, egui follows
  the Windows app mode, including live changes. `TitleBar::apply` recolors the system title bar
  whenever the effective theme changes. The choice is saved through eframe storage under
  Windows' own key and values (`SelectedAppTheme` = `Light` / `Dark` / `Default`).
- Icons are Segoe Fluent Icons code points (`ICON_*`) drawn through `CalculatorApp::icon`,
  which falls back to plain text when the icon font is missing.
- Keyboard: `char_command` handles text input (digits, operators), and `handle_key` handles
  named keys and `Ctrl` shortcuts. The date mode has its own `handle_date_keyboard`, so
  calculator keys do nothing there.
- `Mode` (`Standard`, `DateCalculation`) is picked in the ☰ menu (`draw_navigation`). `App::ui`
  hands the date mode over to `draw_date_mode` and returns early. `Page::Settings` replaces
  either mode, and going back returns to the mode that was open.

### `dates.rs` and `app/date_view.rs`

- `dates.rs` is a port of `DateCalculationEngine` and `DateCalculatorViewModel` from the
  Windows Calculator source: `difference`, `describe_difference`, `add`, `subtract`, the date
  formats, and `month_page_start` for the calendar.
- `date_view.rs` adds `impl CalculatorApp` methods and keeps its state in `DateState`. Only one
  popup is open at a time (`Popup`: the page list, the calendar, or a 0–999 list). It's drawn
  last, over a full-window "light dismiss" backdrop that closes it when clicked.
- Its colors are `Palette` fields too (`popup_*`, `calendar_*`, `control_*`, `other_month`).
  The Add/Subtract choice uses the same `radio_button` as the Settings page.
- Sizes come from `DateCalculator.xaml`: 16 px page margin, 32 px controls, 12 px captions,
  14 px values, results at 20 px from a 480 px window width and 14 px below that, and 84 px
  wide offset boxes.

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

### Date calculation

These come from the Windows Calculator source and are pinned by the tests in `dates.rs`:

- The difference ignores the order of the dates. It counts whole years, then whole months from
  where the years ended, then weeks (7 days), and the rest is days.
- Month and year steps clamp to the end of the month: January 31 + 1 month = February 28, and
  February 29 + 1 year = February 28. Later units continue from the clamped date.
- Output:
  - Equal dates give `Same dates`.
  - Under 7 days, only the days are shown (`5 days`) and there is no second line.
  - Otherwise the non-zero units are joined with `, ` (`1 year, 2 months, 1 week, 3 days`),
    and a second line shows the total (`10 days`). That total has no thousands separators.
  - A unit is singular only for exactly 1 (`1 day`, `2 days`).
- Add goes years → months → days. Subtract goes the other way: days → months → years.
  For example, March 31 − 1 month 1 day = February 28.
- Supported dates are 1601-01-01 to 9999-12-31. A result outside that range shows
  `Date out of Bound`. The offsets go from 0 to 999.
- Formats: pickers show `October 15, 2026`, and the add/subtract result shows
  `Thursday, October 15, 2026`.

## Conventions

- **English everywhere:** code, comments, docs, UI strings, commit messages and PR text.
- Keep `engine.rs` and `dates.rs` free of egui and other UI code. Any new calculator or date
  behavior needs a test.
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
- The title bar tint (`TitleBar`) calls `DwmSetWindowAttribute` and only does something under
  `#[cfg(windows)]`. It keeps the window handle as an `isize`, so the app struct doesn't hold a
  raw pointer.
- eframe's `persistence` feature saves state in `%APPDATA%\CalcCraft\data\app.ron` (the folder
  is named after the app name passed to `run_native`). `persist_window` is off, so the window
  always opens at its default size.
- Release builds use `windows_subsystem = "windows"`, so `println!` output goes nowhere.
  Debug with `cargo run`.
- egui 0.36 has no `raw_scroll_delta`. The calendar reads `Event::MouseWheel` events itself
  (`wheel_delta`) and flips one page per 40 points of wheel movement.
- `chrono` is used without default features (`clock`, `std` only). Use `checked_*` methods for
  date math and keep results inside `MIN_DATE..=MAX_DATE`.

## Verifying UI changes

Unit tests don't cover the UI. After visual or interaction changes:

1. Run the app (`cargo run`) and click through what you changed: hover, press, disabled
   states (enter `1/0=` for the error state), the history and memory panels, the menu,
   keep-on-top mode, and resizing. Check both themes (Settings → App theme). In the date mode,
   check the page list, both date pickers with the month, year and decade views, the 0–999
   lists, `Esc`, and both a narrow (< 480 px) and a wide window.
2. Compare it side by side with the real Windows Calculator.
3. If the look changed, update `docs/screenshot.png` (dark), `docs/screenshot-light.png` or
   `docs/date-calculation.png`.

## Git and pull requests

- Work on a branch and open a pull request against the default branch.
- Write commit subjects in the imperative mood (`Add memory list flyout`), and keep each
  commit to one topic.
- In the PR description, say what changed, why, and how you verified it (tests and manual
  checks). Include before/after screenshots for UI changes.
- Pushing a `v*` tag publishes a GitHub release (`.github/workflows/release.yml`, see "Releasing"
  in the README). The tag must match `version` in `Cargo.toml`. Don't create or push tags unless
  a maintainer asks.
