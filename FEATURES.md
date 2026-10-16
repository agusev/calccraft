# CalcCraft Feature Specification & Agent Tracking Matrix

> **Target audience:** autonomous AI agents and pair-programming assistants working on CalcCraft.
> **Purpose:** catalog of the Windows Calculator features, their dependencies, and how far the
> Rust reimplementation has got.
> **Specification:** the Windows Calculator (`microsoft/calculator`, a local clone is in
> `../win-calculator`). Its behavior wins whenever this file and the real app disagree.
> **Implementation:** CalcCraft, Rust + eframe/egui. Read [AGENTS.md](AGENTS.md) first: it has the
> architecture, the conventions and the Windows behaviors the tests pin down.

---

## 1. Agent Operational Protocol

### 1.1 State Machine
Every feature entry uses these states:
- `[ ] NOT_STARTED`: pending implementation.
- `[~] IN_PROGRESS`: currently being implemented by an active agent.
- `[-] BLOCKED`: blocked by unresolved dependencies or external blockers.
- `[x] COMPLETED`: implemented, tested, and checked against the acceptance criteria.

Every entry has a **Windows ref** (where the original implements it). Completed entries also have
a **Rust** line (where CalcCraft implements it) and, where the two differ, a **Differences** line.
A `Differences` line describes a deliberate choice or a known gap; gaps worth closing have their
own `NOT_STARTED` entry.

### 1.2 "What to Implement Next" Algorithm
1. Scan the feature matrix in **phase order** (Phase 1 → Phase 8).
2. Filter for items with status `[ ] NOT_STARTED`.
3. Check the `Depends On` list of each candidate: every dependency must be `[x] COMPLETED`.
4. Pick the candidate with the highest priority (`P0` > `P1` > `P2` > `P3`).
5. Set it to `[~] IN_PROGRESS` and note the branch before starting work.
6. When the implementation is done and the checks in section 5 pass, set it to `[x] COMPLETED`,
   add the `Rust` line, update the dashboard, and add a row to section 6.

---

## 2. Implementation Progress Dashboard

| Category | Total Features | Completed | In Progress | Blocked | Not Started |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Phase 1: Core Math Engine** | 8 | 8 | 0 | 0 | 0 |
| **Phase 2: Standard Calculator Mode** | 7 | 7 | 0 | 0 | 0 |
| **Phase 3: Scientific Calculator Mode** | 8 | 0 | 0 | 0 | 8 |
| **Phase 4: Programmer Calculator Mode** | 7 | 0 | 0 | 0 | 7 |
| **Phase 5: Date Calculation** | 4 | 4 | 0 | 0 | 0 |
| **Phase 6: Unit Conversion (13 categories)** | 14 | 13 | 0 | 0 | 1 |
| **Phase 7: Graphing Calculator** | 6 | 0 | 0 | 0 | 6 |
| **Phase 8: Application Shell, UX & Accessibility** | 10 | 5 | 0 | 0 | 5 |
| **TOTAL** | **64** | **37** | **0** | **0** | **27** |

*Last updated: 2026-10-15 (CalcCraft 0.1.2)*

---

## 3. Recommended Phased Implementation Roadmap

```mermaid
graph TD
    P1[Phase 1: Core Math Engine] --> P2[Phase 2: Standard Calculator]
    P1 --> P3[Phase 3: Scientific Calculator]
    P1 --> P4[Phase 4: Programmer Calculator]
    P2 --> P8[Phase 8: Application Shell & UI]
    P5[Phase 5: Date Calculation] --> P8
    P6[Phase 6: Unit Converters] --> P8
    P3 --> P7[Phase 7: Graphing Mode]
```

1. **Phase 1: Core engine** (`src/engine.rs`): exact rational arithmetic, a port of the Windows
   engine with and without operator precedence, parentheses, memory, history. *Done.*
2. **Phase 2: Standard mode** (`src/app.rs`): immediate execution calculator and its UI. *Done.*
3. **Phase 3: Scientific mode**: its keypad and UI on top of the precedence engine, trigonometry,
   powers, logarithms.
4. **Phase 4: Programmer mode**: word sizes, radixes, bitwise logic, shifts, rotations, bit keypad.
5. **Phase 5: Date calculation** (`src/dates.rs`, `src/app/date_view.rs`). *Done.*
6. **Phase 6: Unit converters** (`src/converter.rs`, `src/app/converter_view.rs`): 12 converters
   with Windows' units, factors and rounding. *Done, except the Currency converter.*
7. **Phase 7: Graphing mode**: expression parser, plotting, variable sliders, graph analysis.
8. **Phase 8: Shell & system**: navigation, theming, layouts, hotkeys, accessibility, localization.

---

## 4. Detailed Feature Specifications

### Phase 1: Core Math Engine
*Numeric precision and calculation management. In CalcCraft this is `src/engine.rs`, which has no
UI code and is covered by unit tests.*

#### `FEAT-CORE-001` Arbitrary Precision Rational Arithmetic
- **Status:** `[x] COMPLETED`
- **Priority:** `P0`
- **Depends On:** None
- **Windows ref:** `src/CalcManager/Ratpack/rat.cpp`, `trans.cpp`, `itrans.cpp`
- **Rust:** `src/engine.rs` (`Number` = `num_rational::BigRational`, `normalize`, `sqrt`)
- **Description:** Exact rational numbers instead of IEEE-754 floating point, so
  `0.1 + 0.2 − 0.3 = 0` and `1 ÷ 3 × 3 = 1`.
- **Acceptance Criteria:**
  - Addition, subtraction, multiplication and division are exact.
  - The display shows 16 significant digits.
  - Division by zero produces the division-by-zero error.
- **Differences:** Windows keeps 32 digits of precision. CalcCraft is exact until a denominator
  grows beyond 256 bits, then rounds to 40 significant digits. Square roots are exact for perfect
  squares and 40+ digits otherwise.

#### `FEAT-CORE-002` Immediate Execution Calculator Engine
- **Status:** `[x] COMPLETED`
- **Priority:** `P0`
- **Depends On:** `FEAT-CORE-001`
- **Windows ref:** `src/CalcManager/CalculatorManager.cpp`, `CEngine/scicomm.cpp`
- **Rust:** `src/engine.rs` (`Calculator`, a port of `CCalcEngine::ProcessCommandWorker` and
  `CHistoryCollector`)
- **Description:** Left-to-right evaluation used by Standard mode: `3 + 3 × 2 = 12`.
- **Acceptance Criteria:**
  - The next operator press computes the subtotal; the expression line collapses to it (`2 + 3 +` → `5 +`).
  - Pressing another operator replaces the pending one.
  - Repeated `=` repeats the last operator and operand; a new number followed by `=` does too
    (`2 + 3 = 7 =` → `10`).
  - Decimal entry, digit accumulation (16 digits), sign toggle and backspace work.

#### `FEAT-CORE-003` Order of Operations Engine (Operator Precedence)
- **Status:** `[x] COMPLETED`
- **Priority:** `P0`
- **Depends On:** `FEAT-CORE-001`
- **Windows ref:** `src/CalcManager/CEngine/scicomm.cpp` (`m_fPrecedence`), `src/CalcManager/CEngine/History.cpp`
- **Rust:** `src/engine.rs` (`Calculator::with_precedence`, `binary`, `equals`, `precedence_stack`)
- **Description:** Precedence-based evaluation for Scientific mode: `3 + 3 × 2 = 9`. It's the
  same engine as Standard mode with Windows' precedence flag on.
- **Acceptance Criteria:**
  - `×` and `÷` bind tighter than `+` and `−`; pending operations wait on a stack of up to
    25 entries, as in Windows.
  - The expression line keeps the whole expression (`2 + 3 × 4 =`), and the display shows what
    can be computed so far (`2 + 3 × 4 +` shows `14`).
  - Changing an operator to a tighter one after its left side was computed adds brackets:
    `1 + 2 −` changed to `×` becomes `(1 + 2) ×`.
  - History records the whole expression, and repeated `=` repeats the last operation done.
- **Differences:** no mode uses it yet; the Scientific mode (`FEAT-SCI-*`) will. `^` and `mod`
  come with `FEAT-SCI-001` and `FEAT-SCI-008`.

#### `FEAT-CORE-004` Parentheses Evaluation Stack
- **Status:** `[x] COMPLETED`
- **Priority:** `P0`
- **Depends On:** `FEAT-CORE-003`
- **Windows ref:** `src/CalcManager/CEngine/scicomm.cpp` (`IDC_OPENP`, `IDC_CLOSEP`)
- **Rust:** `src/engine.rs` (`Command::OpenParen`, `Command::CloseParen`, `open_parentheses`)
- **Description:** Nested `(` and `)`, up to 25 levels, with precedence on.
- **Acceptance Criteria:**
  - The open parenthesis count is available for the `(` button, as in Windows.
  - `=` closes all open parentheses: `2 × ((3 + 4 =` → `14`.
  - Expressions inside parentheses evaluate first, and a number next to a parenthesis
    multiplies: `2(3)` and `(2)3` are `6`.
  - `)` without an open `(` is ignored; `()` is `(0)`.
  - Functions apply to a whole group: `(1 + 2) x²` shows `sqr(1 + 2)`.
- **Differences:** like in Windows, Standard mode has no parentheses. The count will be shown
  by the Scientific UI.

#### `FEAT-CORE-005` Percentage Calculation Logic
- **Status:** `[x] COMPLETED`
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-002`
- **Windows ref:** `src/CalcManager/CEngine/scicomm.cpp` (`IDC_PERCENT`)
- **Rust:** `src/engine.rs` (`Calculator::percent`)
- **Description:** Context-aware percentage, verified against calc.exe:
  - After `+` or `−`: a percentage of the left operand. `100 + 10%` → `100 + 10` → `110`.
  - After `×` or `÷`: the value divided by 100. `50 × 10%` → `5`, `50 ÷ 10%` → `500`.
  - **Without a pending operator: `0`** (`25%` shows `0`, not `0.25`).
- **Acceptance Criteria:**
  - Matches Windows Standard mode, including the expression line (`50 + 5`).
  - The same rules apply with precedence on: `50 + 10% =` → `50 + 5 =` → `55`.

#### `FEAT-CORE-006` Memory Storage & Recall System (MC, MR, M+, M−, MS)
- **Status:** `[x] COMPLETED`
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-001`
- **Windows ref:** `src/CalcManager/CalculatorManager.cpp`, `src/Calculator.ViewModels/StandardCalculatorViewModel.cs`
- **Rust:** `src/engine.rs` (`Command::Memory*`, `memory_add`, `memory_recall_item`, `memory_clear_item`)
- **Description:** A memory list, newest first (up to 100 values):
  - `MS` stores the display as a new item.
  - `MR` recalls the newest item.
  - `M+` / `M−` add to or subtract from the newest item, or create one when memory is empty.
  - `MC` clears all items.
- **Acceptance Criteria:**
  - `MC`, `MR` and `M˅` are disabled while memory is empty; `MS`, `M+` and `M−` are always enabled.
  - All memory buttons are disabled in the error state.
  - After a memory operation, the next digit starts a new number.

#### `FEAT-CORE-007` History Tape & Expression Tracking
- **Status:** `[x] COMPLETED`
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-002`
- **Windows ref:** `src/CalcManager/CalculatorHistory.cpp`, `src/CalcManager/CEngine/History.cpp`,
  `src/Calculator.ViewModels/HistoryViewModel.cs`
- **Rust:** `src/engine.rs` (`HistoryLine`, `history`, `recall_history`, `delete_history_item`, `clear_history`)
- **Description:** The last 20 calculations, newest first, as an expression and a result.
- **Acceptance Criteria:**
  - Clicking an entry restores its result and expression line.
  - The trash button clears the history; an entry's context menu deletes it.
  - Entries look like `12 + 5 =` above the result `17`. In Standard mode every completed binary
    operation is recorded, including the intermediate ones of a chain (`1 + 2 +` records
    `1 + 2 =`). With precedence on, the entry is the whole expression (`2 + 3 × 4 =`).
  - As in Windows, `5 =` records `5 =`, and a finished function followed by a new number
    records the function (`√(4)` → `2`).
- **Differences:** the history is not saved when the app closes, as in Windows.

#### `FEAT-CORE-008` Calculation Error & Edge Case Engine
- **Status:** `[x] COMPLETED`
- **Priority:** `P0`
- **Depends On:** `FEAT-CORE-001`
- **Windows ref:** `src/CalcManager/CEngine/scicomm.cpp`
- **Rust:** `src/engine.rs` (`CalcError`, `Calculator::display_error`, `is_enabled`)
- **Description:** Errors and their display strings:
  - `Cannot divide by zero` (`1 ÷ 0`, `1/x` of 0).
  - `Result is undefined` (`0 ÷ 0`).
  - `Invalid input` (square root of a negative number, or pasted text that isn't valid).
  - `Overflow` (decimal exponent beyond ±9999, in both directions).
- **Acceptance Criteria:**
  - In the error state only digits, `.`, `C`, `CE` and `⌫` work; pressing any of them leaves the error state.
  - The expression line shows what Windows shows: `1 ÷` after `=`, `8 ÷ 0 +` after an operator, `1/(0)`.

---

### Phase 2: Standard Calculator Mode
*Daily calculator with immediate execution. UI in `src/app.rs`.*

#### `FEAT-STD-001` Basic Standard Keypad & Operator Set
- **Status:** `[x] COMPLETED`
- **Priority:** `P0`
- **Depends On:** `FEAT-CORE-002`
- **Windows ref:** `src/Calculator/Views/CalculatorStandardOperators.xaml`
- **Rust:** `src/app.rs` (`draw_keypad`, `keypad_label`, `button_flash`)
- **Description:** `0`–`9`, `.`, `+ − × ÷`, `=`, `⌫`, `CE`, `C`, `%`, plus the memory row.
- **Acceptance Criteria:**
  - Same 4 × 6 grid, labels, icons (Segoe Fluent Icons) and colors as Windows 11.
  - A second decimal point in one operand is ignored.
  - The display shrinks long numbers to fit, with en-US grouping (`1,234,567`).

#### `FEAT-STD-002` Unary Math Operators
- **Status:** `[x] COMPLETED`
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-001`, `FEAT-STD-001`
- **Windows ref:** `src/CalcManager/CEngine/scifunc.cpp`
- **Rust:** `src/engine.rs` (`Func`)
- **Description:** `1/x`, `x²`, `√x`, `±`.
- **Acceptance Criteria:**
  - They apply to the displayed operand at once and nest in the expression line:
    `sqr(sqr(4))`, `√(9)`, `1/(5)`, `negate(8)`.
  - `±` while typing flips the sign without changing the expression line.

#### `FEAT-STD-003` Standard History & Memory Panels
- **Status:** `[x] COMPLETED` (flyouts)
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-006`, `FEAT-CORE-007`, `FEAT-STD-001`
- **Windows ref:** `src/Calculator/Views/HistoryList.xaml`, `Memory.xaml`, `MemoryListItem.xaml`
- **Rust:** `src/app.rs` (`draw_flyout`, `history_item`, `memory_item`, `draw_context_menu`)
- **Description:** History (header button, `Ctrl+H`) and memory (`M˅`) panels.
- **Acceptance Criteria:**
  - The panel covers the keypad; clicking the display or pressing `Esc` closes it.
  - Memory items show `MC`, `M+` and `M−` on hover; clicking an item recalls it.
  - Right-clicking an item opens its context menu (`FEAT-STD-007`).
  - Empty panels show "There's no history yet" / "There's nothing saved in memory".
- **Differences:** Windows docks history and memory in a side panel in wide windows. CalcCraft
  always uses the flyout. Docking is part of `FEAT-SHELL-002`.

#### `FEAT-STD-004` Always-on-Top (Compact Overlay) Mode
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-STD-001`
- **Windows ref:** `src/Calculator/Views/MainPage.xaml.cs`, `src/Calculator/Views/Calculator.xaml`
- **Rust:** `src/app.rs` (`set_compact`)
- **Description:** A small window that stays above other windows.
- **Acceptance Criteria:**
  - The window is always on top and shrinks to 320 × 394; leaving the mode restores the
    previous size.
  - The menu, title, history button and memory row are hidden; a back button returns to full view.
- **Differences:** the keypad rows never hide when the window gets very short.

#### `FEAT-STD-005` Keyboard Input & Hotkeys for Standard Mode
- **Status:** `[x] COMPLETED`
- **Priority:** `P1`
- **Depends On:** `FEAT-STD-001`
- **Windows ref:** `docs/ManualTests.md` (Hotkeys), `src/Calculator/Views/CalculatorStandardOperators.xaml`
- **Rust:** `src/app.rs` (`char_command`, `handle_key`)
- **Description:**
  - `0`–`9`, `.` or `,`, `+`, `-`, `*`, `/`, `%`, `Enter` or `=`
  - `Delete` = `CE`, `Esc` = `C` (or closes an open panel), `Backspace` = `⌫`, `F9` = `±`
  - `R` = `1/x`, `Q` = `x²`, `@` = `√x`
  - `Ctrl+M` (MS), `Ctrl+P` (M+), `Ctrl+Q` (M−), `Ctrl+R` (MR), `Ctrl+L` (MC)
  - `Ctrl+H` (history), `Ctrl+Shift+D` (clear history)
- **Acceptance Criteria:**
  - All hotkeys work without focusing a button, and the pressed key's button flashes.
  - Buttons never take keyboard focus, so `Enter` always means `=` (see AGENTS.md pitfalls).

#### `FEAT-STD-006` Clipboard Copy and Paste (numbers)
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-STD-001`
- **Windows ref:** `src/Calculator.ViewModels/Common/CopyPasteManager.cs`
- **Rust:** `src/engine.rs` (`copy_text`, `paste`)
- **Description:** `Ctrl+C` copies the displayed value; `Ctrl+V` pastes a number.
- **Acceptance Criteria:**
  - Copy leaves out the grouping commas (`1234.5`); nothing is copied in the error state.
  - Paste ignores spaces, grouping commas, quotes and currency signs, and accepts up to 16
    digits: `"1,234.5"` → `1,234.5`, `"-007"` → `-7`, `"$12"` → `12`. Invalid text shows
    `Invalid input` (see `FEAT-STD-007`).

#### `FEAT-STD-007` Expression Paste & Panel Context Menus
- **Status:** `[x] COMPLETED`
- **Priority:** `P3`
- **Depends On:** `FEAT-STD-006`
- **Windows ref:** `src/Calculator.ViewModels/Common/CopyPasteManager.cs`,
  `src/Calculator.ViewModels/StandardCalculatorViewModel.cs` (`OnPaste`),
  `src/Calculator/Views/HistoryList.xaml`, `Memory.xaml` (context menus)
- **Rust:** `src/engine.rs` (`paste`, `paste_is_valid`, `extract_operands`), `src/app.rs` (`draw_context_menu`)
- **Description:** Paste `"12 + 5 ="` and evaluate it. History items have a context menu with
  Copy and Delete; memory items have one with "Clear memory item", "Add to memory item" and
  "Subtract from memory item". As in Windows, memory items can't be copied.
- **Acceptance Criteria:**
  - Pasted text follows the rules and limits of `CopyPasteManager`: at most 512 characters,
    100 operands and 16 digits per operand (32 with precedence on, where parentheses are allowed).
  - Valid text is typed in key by key after a `CE`: `"12 + 5 ="` → `17`, `"2*-3="` → `-6`
    (a `-` at the start or after an operator becomes `±` after the first non-zero digit).
  - Invalid clipboard text shows `Invalid input`, as in Windows.
  - Copy in the history menu copies the result as the list shows it (`1,236`).
  - The menu opens at the pointer; a click outside it or `Esc` closes it.
- **Differences:** Windows also accepts numbers in e-notation (`1e5`); CalcCraft rejects them
  until the engine has exponent entry (`FEAT-SCI-006`). The memory menu shows `MC`, `M+` and
  `M−` as text, since CalcCraft doesn't ship the Calculator's own icon font.

---

### Phase 3: Scientific Calculator Mode
*Scientific operations, trigonometry and logarithms.*

> **Rust notes for the whole phase:** `BigRational` has no `sin`, `ln` or real powers. Either
> implement series (Taylor/AGM) at about 40 digits on top of `num-bigint`, or pick a
> multi-precision crate that builds on Windows MSVC, and decide this before writing
> `FEAT-SCI-001`. Keep `engine.rs` free of UI code and pin every behavior with a test.
> The Scientific mode runs on `Calculator::with_precedence()`, which already has operator
> precedence, parentheses (`open_parentheses` for the `(` counter) and the Scientific paste rules.

#### `FEAT-SCI-001` Expanded Powers and Roots
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-003`
- **Windows ref:** `src/CalcManager/CEngine/scifunc.cpp`, `Ratpack/exp.cpp`
- **Description:** $x^y$, $10^x$, $e^x$, $2^x$, $\sqrt[y]{x}$, $\sqrt[3]{x}$, $x^3$, $x^2$.
- **Acceptance Criteria:**
  - Accurate for positive, negative and fractional exponents.
  - Invalid operations (an even root of a negative number) show `Invalid input`.

#### `FEAT-SCI-002` Trigonometric Functions & Angle Modes (DEG, RAD, GRAD)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-003`
- **Windows ref:** `src/CalcManager/Ratpack/trans.cpp`
- **Description:** `sin`, `cos`, `tan`, `csc`, `sec`, `cot`, with Degrees, Radians and Gradians.
- **Acceptance Criteria:**
  - $\sin(90^\circ) = 1$, $\sin(\pi/2 \text{ rad}) = 1$, $\sin(100 \text{ grad}) = 1$.
  - Asymptotes such as $\tan(90^\circ)$ give `Invalid input`.

#### `FEAT-SCI-003` Inverse and Hyperbolic Trigonometry (2nd & hyp)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-SCI-002`
- **Windows ref:** `src/Calculator/Views/CalculatorScientificOperators.xaml`
- **Description:** `2nd` gives $\sin^{-1}$ … $\cot^{-1}$; `hyp` gives $\sinh$ … $\coth$; both give $\sinh^{-1}$ ….
- **Acceptance Criteria:**
  - Button labels and actions switch while `2nd` or `hyp` is on.

#### `FEAT-SCI-004` Logarithms and Exponentials
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-003`
- **Windows ref:** `src/CalcManager/Ratpack/logic.cpp`, `exp.cpp`
- **Description:** $\ln(x)$, $\log_{10}(x)$, $\log_y(x)$, $e^x$.
- **Acceptance Criteria:**
  - $\ln(e) = 1$, $\log_{10}(1000) = 3$.
  - $x \le 0$ shows `Invalid input`.

#### `FEAT-SCI-005` Mathematical Constants & Special Unary Functions
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-001`
- **Windows ref:** `src/CalcManager/Ratpack/support.cpp`, `fact.cpp`
- **Description:** $\pi$, $e$, factorial $n!$, absolute value $|x|$.
- **Acceptance Criteria:**
  - Constants are entered with full internal precision.
  - Factorial of non-integers follows Windows (gamma function) and of negative numbers shows `Invalid input`.

#### `FEAT-SCI-006` Notation & Formatting Toggles (F-E, exp)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CORE-001`
- **Windows ref:** `src/Calculator.ViewModels/StandardCalculatorViewModel.cs`
- **Description:** `F-E` toggles fixed and scientific notation; `exp` enters an exponent (`3.5e+4`).
- **Acceptance Criteria:**
  - Scientific notation uses the existing Windows format (`1.234e+18`), see `format_number` in `engine.rs`.

#### `FEAT-SCI-007` DMS (Degrees, Minutes, Seconds) Converter
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-SCI-002`
- **Windows ref:** `src/CalcManager/CEngine/scifunc.cpp`
- **Description:** `dms` converts decimal degrees to D.MMSS; `deg` converts back.
- **Acceptance Criteria:**
  - $30.5^\circ \rightarrow 30.30$ ($30^\circ 30'$).

#### `FEAT-SCI-008` Modulo Operator (mod)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-003`
- **Windows ref:** `src/CalcManager/CEngine/scioper.cpp`
- **Description:** Binary remainder operator.
- **Acceptance Criteria:**
  - $10 \text{ mod } 3 = 1$; the sign of the result follows Windows for negative operands.

---

### Phase 4: Programmer Calculator Mode
*Binary, hex, bitwise logic and word sizes.*

> **Rust notes for the whole phase:** values are integers of a fixed word size, so use `i64`/`u64`
> with masking rather than `BigRational`. Keep this logic in its own module without UI code.

#### `FEAT-PROG-001` Multi-Radix Number Bases (HEX, DEC, OCT, BIN)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P0`
- **Depends On:** `FEAT-CORE-001`
- **Windows ref:** `src/Calculator/Views/CalculatorProgrammerOperators.xaml`, `src/CalcManager/CEngine/scicomm.cpp`
- **Description:** The value shown in all four radixes at once, typed in the selected one.
- **Acceptance Criteria:**
  - Digits that don't exist in the radix are disabled (A–F outside HEX, 8–9 in OCT, 2–9 in BIN).
  - Entry lengths are limited by the word size, as in Windows.

#### `FEAT-PROG-002` Word Sizes (QWORD, DWORD, WORD, BYTE)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P0`
- **Depends On:** `FEAT-PROG-001`
- **Windows ref:** `src/CalcManager/CalculatorManager.cpp`
- **Description:** 64, 32, 16 and 8 bit two's complement values.
- **Acceptance Criteria:**
  - Switching to a smaller size truncates (`0x1FFFF` in WORD becomes `0xFFFF`).
  - Correct sign extension and two's complement display in DEC.

#### `FEAT-PROG-003` Bitwise Logic Operations
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P1`
- **Depends On:** `FEAT-PROG-002`
- **Windows ref:** `src/CalcManager/CEngine/scioper.cpp`
- **Description:** `AND`, `OR`, `NOT`, `XOR`, `NAND`, `NOR`, `XNOR`.
- **Acceptance Criteria:**
  - `0b1010 AND 0b1100 = 0b1000`.
  - `NOT` respects the word size (in BYTE, `NOT 0` is `0xFF`).

#### `FEAT-PROG-004` Bit Shifts (Logical & Arithmetic)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P1`
- **Depends On:** `FEAT-PROG-002`
- **Windows ref:** `src/CalcManager/CEngine/scioper.cpp`
- **Description:** Left shift, logical right shift, arithmetic right shift (the shift type is chosen as in Windows).
- **Acceptance Criteria:**
  - In BYTE: `0b10000000` arithmetic right shift 1 = `0b11000000`.

#### `FEAT-PROG-005` Bit Rotations (RoL, RoR, rotate through carry)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P1`
- **Depends On:** `FEAT-PROG-002`
- **Windows ref:** `src/CalcManager/CEngine/scioper.cpp`
- **Description:** Circular rotation within the word size.
- **Acceptance Criteria:**
  - In BYTE: `0b10000001 RoL 1 = 0b00000011`.

#### `FEAT-PROG-006` Bit Toggling Keypad
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-PROG-001`, `FEAT-PROG-002`
- **Windows ref:** `src/Calculator/Views/CalculatorProgrammerBitFlipPanel.xaml` (and `.xaml.cs`)
- **Description:** 64 clickable bits in groups of four.
- **Acceptance Criteria:**
  - Clicking a bit flips it and updates every radix at once.
  - Bits above the word size are disabled.

#### `FEAT-PROG-007` Hotkeys for Programmer Mode
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-PROG-001`
- **Windows ref:** `docs/ManualTests.md`, `src/Calculator/Views/CalculatorProgrammerOperators.xaml`
- **Description:**
  - `F5` (HEX), `F6` (DEC), `F7` (OCT), `F8` (BIN)
  - `F12` (QWORD), `F2` (DWORD), `F3` (WORD), `F4` (BYTE)
  - `&` (AND), `|` (OR), `~` (NOT), `^` (XOR), `<` / `>` (shifts), `%` (mod)
- **Acceptance Criteria:**
  - Check every key against the Windows source before implementing; the mode's own keys
    override the Standard ones (`%` is mod here).

---

### Phase 5: Date Calculation
*Date difference and date arithmetic. Logic in `src/dates.rs` (port of `DateCalculationEngine`),
UI in `src/app/date_view.rs`. Detailed rules: `../win-calculator/date_calculation.md` and AGENTS.md.*

#### `FEAT-DATE-001` Date Difference Engine
- **Status:** `[x] COMPLETED`
- **Priority:** `P1`
- **Depends On:** None
- **Windows ref:** `src/Calculator.ViewModels/Common/DateCalculator.cs`, `DateCalculatorViewModel.cs`
- **Rust:** `src/dates.rs` (`difference`, `describe_difference`, `days_between`)
- **Description:** Years → months → weeks → days, each counted from where the previous unit
  ended, with month-end clamping.
- **Acceptance Criteria:**
  - Equal dates show `Same dates`.
  - Under 7 days, only the days are shown and the second line is hidden.
  - Otherwise the first line lists the non-zero units (`1 week, 3 days`) and the second the total (`10 days`).
  - The order of the two dates doesn't matter; leap years and month ends match Windows.

#### `FEAT-DATE-002` Add or Subtract Date Offsets
- **Status:** `[x] COMPLETED`
- **Priority:** `P1`
- **Depends On:** None
- **Windows ref:** `src/Calculator.ViewModels/Common/DateCalculator.cs`
- **Rust:** `src/dates.rs` (`add`, `subtract`)
- **Description:** Years, months and days (0–999 each) added to or subtracted from a date.
- **Acceptance Criteria:**
  - Add goes years → months → days, subtract days → months → years, with month-end clamping.
  - Results outside 1601-01-01 … 9999-12-31 show `Date out of Bound`.
  - The result reads `Thursday, October 15, 2026`.

#### `FEAT-DATE-003` Singular / Plural Formatting
- **Status:** `[x] COMPLETED` (English)
- **Priority:** `P2`
- **Depends On:** `FEAT-DATE-001`, `FEAT-DATE-002`
- **Windows ref:** `src/Calculator/Resources/en-US/Resources.resw` (`Date_Day`, `Date_Days`, …)
- **Rust:** `src/dates.rs` (`count`)
- **Description:** `1 year`, `2 years`; zero units are left out; the total in days has no grouping (`1587 days`).
- **Differences:** English only. Other languages belong to `FEAT-SHELL-007`.

#### `FEAT-DATE-004` Date Calculation UI
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-DATE-001`, `FEAT-DATE-002`
- **Windows ref:** `src/Calculator/Views/DateCalculator.xaml`
- **Rust:** `src/app/date_view.rs`
- **Description:** A drop-down switches between "Difference between dates" and "Add or subtract
  days". Date pickers open a calendar with month, year and decade views. Add/Subtract are radio
  buttons, and the years/months/days boxes open 0–999 lists.
- **Acceptance Criteria:**
  - Sizes from `DateCalculator.xaml`; the result font is 20 px from a 480 px window width, 14 px below.
  - The calendar works with its arrows and the mouse wheel; `Esc` closes popups; `Ctrl+C` copies the result.
  - Both themes.
- **Differences:** always the Gregorian calendar, en-US date formats and weeks starting on Sunday.
  There is no keyboard navigation inside the calendar and lists yet (`FEAT-SHELL-005`).

---

### Phase 6: Unit Conversion (13 categories)
*Converters with live two-way conversion. In CalcCraft the logic is `src/converter.rs` (no UI
code, unit tests at the bottom) and the UI is `src/app/converter_view.rs`.*

> **Rust notes for the whole phase:** the units, their order, the default units per region and
> the factors are copied from Windows' `src/Calculator.ViewModels/DataLoaders/UnitConverterDataLoader.cs`.
> Windows converts with `double` and rounds the result with its own rules, so `converter.rs` uses
> `f64` rather than `BigRational`: that way the digits match Windows. The everyday comparison units
> that the same file defines (paperclips, bananas, whales, …) are in their own tables
> (`*_COMPARISONS`) and only appear in the "About equal to" results (`FEAT-SHELL-004`).

#### `FEAT-CONV-001` Core Unit Converter Pipeline & Two-Way Binding
- **Status:** `[x] COMPLETED`
- **Priority:** `P1`
- **Depends On:** `FEAT-CORE-001`
- **Windows ref:** `src/CalcManager/UnitConverter.cpp`, `src/CalcManager/NumberFormattingUtils.cpp`,
  `src/Calculator.ViewModels/UnitConverterViewModel.cs`, `src/Calculator/Views/UnitConverter.xaml`
- **Rust:** `src/converter.rs` (`Converter`, `Region`), `src/app/converter_view.rs`
- **Description:** Converts between a source and a target unit; either field can be the active one; converter keypad.
- **Acceptance Criteria:**
  - Results are rounded like Windows: at least 7 significant digits (more when more were typed,
    up to 15), 15 decimals below 0.000001, and scientific notation (`8.000000e+24`) beyond 15
    integer digits or below 1e-14. Up to 15 characters can be typed, the minus sign included.
  - Clicking the other value makes it the active one (shown in semibold). Its value stays until
    the next key, which starts a new number. A new source unit also makes the next key start over.
  - Categories that don't support negative values hide `±` (Temperature, Power and Angle allow negatives).
  - The default units depend on the Windows region, as in Windows: regions with US customary units
    convert from metric units (centimeters → inches), the others to them, and Fahrenheit regions
    start at Celsius → Fahrenheit.
  - Each category keeps its units while the app runs. The last category and its units are saved
    as `UnitConverterPreferences`.
  - Keyboard: digits, `.` or `,`, `Backspace`, `Delete` (`CE`), `F9` or `-` (`±`). `Ctrl+C`
    copies the active value, and `Ctrl+V` pastes a number with Windows' checks (one plain number,
    at most 16 characters). Invalid text shows `Invalid input` in both fields.
  - A unit drop-down opens with the selected unit over its picker; `Esc` or a click outside closes it.
  - In a landscape window the keypad sits next to the values, and from 640 px on the values, the
    unit pickers and the keys get larger, as in `UnitConverter.xaml`.
- **Differences:** the unit lists don't wrap around (Windows uses a looping `CarouselPanel`).
  The "About equal to" results under the values are `FEAT-SHELL-004`.

#### `FEAT-CONV-002` Length Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`LENGTH`)
- **Units:** Ångströms, Nanometers, Microns, Millimeters, Centimeters, Meters, Kilometers, Inches, Feet, Yards, Miles, Nautical miles.
- **Acceptance Criteria:** 5 kilometers = 3.106856 miles.

#### `FEAT-CONV-003` Volume Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`VOLUME`)
- **Units:** Milliliters, Cubic centimeters, Liters, Cubic meters, Cubic inches, Cubic feet, Cubic yards, Cups (US), and Teaspoons, Tablespoons, Fluid ounces, Pints, Quarts and Gallons, each in a US and a UK version.
- **Acceptance Criteria:** 1 US gallon = 3.785412 liters.

#### `FEAT-CONV-004` Weight and Mass Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`WEIGHT`)
- **Units:** Carats, Milligrams, Centigrams, Decigrams, Grams, Decagrams, Hectograms, Kilograms, Metric tonnes, Ounces, Pounds, Stone, Short tons (US), Long tons (UK).
- **Acceptance Criteria:** 1 kilogram = 2.204623 pounds.

#### `FEAT-CONV-005` Temperature Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`TEMPERATURE`: ratio, offset and order per pair, as in Windows)
- **Units:** Celsius, Fahrenheit, Kelvin. Negative values allowed.
- **Acceptance Criteria:** 0 °C = 32 °F = 273.15 K.

#### `FEAT-CONV-006` Energy Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`ENERGY`)
- **Units:** Electron volts, Joules, Kilojoules, Thermal calories, Food calories, Foot-pounds, British thermal units, Kilowatt-hours.
- **Acceptance Criteria:** 1 food calorie = 4,184 joules.

#### `FEAT-CONV-007` Area Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`AREA`)
- **Units:** Square millimeters, Square centimeters, Square meters, Hectares, Square kilometers, Square inches, Square feet, Square yards, Acres, Square miles, and Pyeong in the Japan, Korea and Taiwan regions.
- **Acceptance Criteria:** 1 acre = 43,560 square feet.

#### `FEAT-CONV-008` Speed Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`SPEED`)
- **Units:** Centimeters per second, Meters per second, Kilometers per hour, Feet per second, Miles per hour, Knots, Mach.
- **Acceptance Criteria:** 1 Mach = 1,225.08 km/h (Windows defines Mach as 340.3 m/s). Windows'
  factors also make a mile per hour 44.7 cm/s and a knot 51.44 cm/s, so 1 mph shows 1.6092 km/h.

#### `FEAT-CONV-009` Time Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`TIME`)
- **Units:** Microseconds, Milliseconds, Seconds, Minutes, Hours, Days, Weeks, Years.
- **Acceptance Criteria:** 1 week = 168 hours = 10,080 minutes.

#### `FEAT-CONV-010` Power Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`POWER`)
- **Units:** Watts, Kilowatts, Horsepower, Foot-pounds per minute, BTUs per minute. Negative values allowed.
- **Acceptance Criteria:** 1 horsepower = 745.6999 watts.

#### `FEAT-CONV-011` Data Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`DATA`)
- **Units:** Bits, Nibbles, Bytes, and the decimal (kilo, mega, giga, tera, peta, exa, zetta, yotta) and binary (kibi … yobi) bit and byte units.
- **Acceptance Criteria:** 1 KB = 1,000 bytes and 1 KiB = 1,024 bytes are kept apart.

#### `FEAT-CONV-012` Pressure Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`PRESSURE`)
- **Units:** Atmospheres, Bars, Kilopascals, Millimeters of mercury, Pascals, Pounds per square inch.
- **Acceptance Criteria:** 1 atmosphere = 101,325 pascals = 14.69595 psi.

#### `FEAT-CONV-013` Angle Converter
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Rust:** `src/converter.rs` (`ANGLE`)
- **Units:** Degrees, Radians, Gradians. Negative values allowed.
- **Acceptance Criteria:** 180° = π radians = 200 gradians (π shows as 3.141593).

#### `FEAT-CONV-014` Currency Converter
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Windows ref:** `src/Calculator.ViewModels/DataLoaders/CurrencyDataLoader.cs`
- **Description:** Exchange rates between currencies, updated online and cached for offline use.
- **Acceptance Criteria:**
  - "Update rates" refreshes the rates and shows when they were updated.
  - Without a connection, the cached rates are used.
- **Rust notes:** Windows uses a Microsoft-internal rate service. CalcCraft needs its own source,
  an HTTP client and a cache file next to the settings (`%APPDATA%\CalcCraft`). Pick a source
  whose terms allow this use before writing code. In Windows, Currency is the first converter in
  the menu; `Category` in `src/converter.rs` and `UnitConverterDataLoader.cs` show where it goes.

---

### Phase 7: Graphing Calculator
*2D plotting, variables and graph analysis.*

> **Rust notes for the whole phase:** needs an expression parser (with implicit multiplication
> and variables) and a plot view. `egui_plot` is a candidate; custom painting matches the rest of
> the app. Graphing depends on the Scientific math functions.

#### `FEAT-GRAPH-001` 2D Cartesian Function Plotting
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CORE-003`, `FEAT-SCI-002`
- **Windows ref:** `src/GraphControl`, `src/GraphingImpl`
- **Description:** Plot $y = f(x)$ on auto-scaled axes.
- **Acceptance Criteria:**
  - Several equations at once, each in its own color (Windows' `EquationBrush1`–`14`, see `app_theme.md`).
  - Hiding an equation hides its curve.

#### `FEAT-GRAPH-002` Viewport Navigation (Pan, Zoom, Reset)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-GRAPH-001`
- **Windows ref:** `src/GraphControl`
- **Description:** Pan by dragging, zoom with the wheel and buttons, reset the view.
- **Acceptance Criteria:**
  - "Reset view" restores the default origin and scale.
  - `Ctrl` + `+` / `Ctrl` + `-` zoom.

#### `FEAT-GRAPH-003` Interactive Curve Tracer
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P3`
- **Depends On:** `FEAT-GRAPH-001`
- **Windows ref:** `src/GraphControl`
- **Description:** Move a cursor along a curve to read $(x, y)$.
- **Acceptance Criteria:** the cursor snaps to the nearest visible curve and shows its coordinates.

#### `FEAT-GRAPH-004` Variable Sliders ($y = mx + b$)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P3`
- **Depends On:** `FEAT-GRAPH-001`
- **Windows ref:** `src/Calculator/Views/GraphingCalculator`
- **Description:** Free variables get sliders.
- **Acceptance Criteria:** value, minimum, maximum and step can be set; the curve updates while dragging.

#### `FEAT-GRAPH-005` Key Graph Features Analysis
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P3`
- **Depends On:** `FEAT-GRAPH-001`
- **Windows ref:** `docs/ManualTests.md` (Key Graph Features), `src/GraphingImpl`
- **Description:** Domain, range, intercepts, minima and maxima, inflection points, asymptotes,
  parity, monotonicity, periodicity.
- **Acceptance Criteria:**
  - Correct for $y = x^2$, $y = 1/x$, $y = \sin(x)$.
  - Unsupported functions show a message instead of wrong results.
- **Rust notes:** Windows uses a proprietary math engine for this; it is the largest item in the file.

#### `FEAT-GRAPH-006` Graph Export & Sharing
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P3`
- **Depends On:** `FEAT-GRAPH-001`
- **Windows ref:** `src/Calculator/Views/GraphingCalculator`
- **Description:** Copy the graph as an image.
- **Acceptance Criteria:** the image matches the current view and equations.
- **Differences:** Windows also shares through the Windows share sheet; a desktop Rust app can copy to the clipboard or save a PNG.

---

### Phase 8: Application Shell, UX, Theming & Accessibility
*Navigation, settings, theming, layouts and accessibility. Mostly `src/app.rs`.*

#### `FEAT-SHELL-001` Navigation Menu & Mode Switching
- **Status:** `[x] COMPLETED` (for the implemented modes)
- **Priority:** `P0`
- **Depends On:** None
- **Windows ref:** `src/Calculator.ViewModels/Common/NavCategory.cs`, `src/Calculator/Views/MainPage.xaml`
- **Rust:** `src/app.rs` (`Mode`, `draw_navigation`, `nav_item`, `set_mode`)
- **Description:** The ☰ menu lists the modes under a **Calculator** and a **Converter** heading,
  with Settings at the bottom. The list scrolls when the window is too short for it.
- **Acceptance Criteria:**
  - Switching modes keeps each mode's state; the selected mode has the accent pill.
  - Every new mode adds itself to `Mode` and to the menu.
- **Differences:** Windows lists 18 modes: Standard, Scientific, Graphing, Programmer, Date
  calculation, and 13 converters. CalcCraft lists only the implemented modes (Standard, Date
  calculation and 12 converters, without Currency). Windows draws six converter icons (Volume,
  Weight, Temperature, Speed, Data, Angle) from the Calculator's own icon font, whose license
  limits it to Windows; CalcCraft uses similar Segoe Fluent Icons glyphs. The Alt hotkeys are
  `FEAT-SHELL-010`.

#### `FEAT-SHELL-002` Adaptive Layouts (docked panels)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P1`
- **Depends On:** `FEAT-SHELL-001`
- **Windows ref:** `docs/ApplicationArchitecture.md` (VisualStates), `src/Calculator/Views/Calculator.xaml`
- **Description:** Layouts for small and wide windows.
- **Acceptance Criteria:**
  - In wide windows, history and memory dock in a side panel with tabs (`FEAT-STD-003` uses flyouts only).
  - Converters switch between vertical and horizontal layouts.
  - Nothing clips or overlaps at the minimum window size.
- **Already there:** the keypad and fonts scale with the window, the date page switches result
  sizes at 480 px, and the converters switch to the horizontal layout in landscape windows and to
  larger sizes from 640 px.

#### `FEAT-SHELL-003` App Theme & Color Palettes
- **Status:** `[x] COMPLETED`
- **Priority:** `P1`
- **Depends On:** `FEAT-SHELL-001`
- **Windows ref:** `../win-calculator/app_theme.md`, `src/Calculator/Views/Settings.xaml`, `src/Calculator/App.xaml`
- **Rust:** `src/app/palette.rs` (`DARK`, `LIGHT`), `src/app.rs` (`Theme`, `TitleBar`)
- **Description:** Light, Dark and "Use system setting", with the keypad opacities from `App.xaml`.
- **Acceptance Criteria:**
  - The choice is saved as `SelectedAppTheme` (`Light` / `Dark` / `Default`) in `%APPDATA%\CalcCraft`.
  - Switching repaints at once, title bar included; "Use system setting" follows Windows live.
- **Differences:** no Mica material and no system accent color. Both palettes use a fixed purple
  tint and accent, and the light background is an estimate. There is no high contrast theme.

#### `FEAT-SHELL-004` Supplementary Results (everyday comparisons)
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** `FEAT-CONV-001`
- **Windows ref:** `src/Calculator/Views/SupplementaryResults.xaml`, `DelighterUnitStyles.xaml`,
  `src/Calculator/Controls/HorizontalNoOverflowStackPanel.cs`, `UnitConverter::CalculateSuggested`
- **Rust:** `src/converter.rs` (`Converter::suggestions`, `*_COMPARISONS`), `src/app/converter_view.rs` (`draw_suggestions`)
- **Description:** "About equal to" under the converter values: the value in the category's
  other units, then the best everyday comparison ("👋 5.36 hands").
- **Acceptance Criteria:**
  - They update with the converted value. The other units come closest to 1 first (by the size
    of log10), rounded to 2 decimals below 100, 1 below 1,000 and none above; results that round
    to 0 are left out, except in Temperature, Power and Angle.
  - The everyday comparison is the one closest to 1 that isn't 0, shown with its Segoe UI Symbol
    picture. Temperature, Time, Pressure and Angle have none.
  - Results stay on one line: those that don't fit are skipped (a narrower one after them can
    still fit), and with more than two results the room for the comparison at the end is kept.
- **Differences:** for negative values Windows sorts by the log10 of a negative number (NaN),
  so its order is undefined; CalcCraft sorts by the absolute value.

#### `FEAT-SHELL-005` Keyboard Focus Navigation
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P1`
- **Depends On:** `FEAT-SHELL-001`
- **Windows ref:** `docs/ManualTests.md`
- **Description:** `Tab`, `Shift+Tab` and arrow keys move focus through buttons, menus, lists, the calendar and Settings.
- **Acceptance Criteria:**
  - Visible focus rings and a logical tab order in every mode.
- **Rust notes:** buttons are deliberately not focusable today, so `Enter` always means `=`
  (AGENTS.md pitfalls). Focus navigation must keep that: `Enter` on the keypad is still `=`.

#### `FEAT-SHELL-006` Screen Reader Support (Narrator)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-SHELL-001`
- **Windows ref:** `docs/ManualTests.md` (Ease of Access), `AutomationProperties` in the XAML views
- **Description:** Names and live regions for every control and result.
- **Acceptance Criteria:**
  - Narrator reads results and button names (`±` as "positive negative").
- **Rust notes:** eframe ships with AccessKit, but the hand-painted controls register no names.
  Give each control `Response::widget_info` with a label.

#### `FEAT-SHELL-007` Localization & Right-to-Left Support
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-SHELL-001`
- **Windows ref:** `src/Calculator/Resources/*/Resources.resw`, `docs/ManualTests.md` (Localization)
- **Description:** Translated strings, mirrored layouts for right-to-left languages, regional number and date formats.
- **Acceptance Criteria:**
  - Decimal and grouping separators and date formats follow the system region.
  - Right-to-left layouts mirror without overflow.
- **Rust notes:** today everything is English with en-US formats: number formatting in
  `engine.rs` (`format_with`, `format_input`), date formats in `dates.rs`.

#### `FEAT-SHELL-008` Settings Page
- **Status:** `[x] COMPLETED`
- **Priority:** `P1`
- **Depends On:** `FEAT-SHELL-001`
- **Windows ref:** `src/Calculator/Views/Settings.xaml`
- **Rust:** `src/app.rs` (`draw_settings`, `settings_content`)
- **Description:** Opened from the bottom of the menu; replaces the current mode until you go back (`Esc` or the back button).
- **Acceptance Criteria:**
  - Appearance: the expandable App theme card (`FEAT-SHELL-003`).
  - About: icon, name and version; "Send feedback" and GitHub links.
- **Differences:** no license or privacy statement links in About.

#### `FEAT-SHELL-009` App Identity (icons, title bar)
- **Status:** `[x] COMPLETED`
- **Priority:** `P2`
- **Depends On:** None
- **Windows ref:** `src/Calculator/Package.appxmanifest`, `src/Calculator/Views/TitleBar.xaml.cs`
- **Rust:** `src/icon.rs`, `build.rs`, `src/main.rs`, `src/app.rs` (`TitleBar`)
- **Description:** Window title "CalcCraft"; one icon drawing used as the window icon and embedded
  in `calculator.exe` (16–256 px) with version information; title bar tinted with the theme background.
- **Acceptance Criteria:**
  - Explorer, the taskbar and Task Manager show the icon and "CalcCraft".
- **Differences:** Windows' title bar is part of the app (Mica, custom buttons). CalcCraft keeps
  the system title bar and only recolors it.

#### `FEAT-SHELL-010` Mode Hotkeys (Alt+1 … Alt+5)
- **Status:** `[ ] NOT_STARTED`
- **Priority:** `P2`
- **Depends On:** `FEAT-SHELL-001`
- **Windows ref:** `src/Calculator.ViewModels/Common/NavCategory.cs` (`VirtualKey`)
- **Description:** `Alt+1` Standard, `Alt+2` Scientific, `Alt+3` Graphing, `Alt+4` Programmer, `Alt+5` Date calculation.
- **Acceptance Criteria:**
  - Keys of modes that don't exist yet do nothing; `Alt+1` and `Alt+5` work today.

---

## 5. Agent Quick-Check Commands

Run these before marking anything completed (the same checks run in CI):

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Then run the app (`cargo run`) and check the changed UI in both themes, as described in
"Verifying UI changes" in AGENTS.md. Compare uncertain behavior with the real calc.exe or the
Windows source in `../win-calculator`.

---

## 6. Revision & Work History

| Date | Agent / Contributor | Feature ID | Change Description |
| :--- | :--- | :--- | :--- |
| 2026-10-15 | Antigravity AI | ALL | Created initial comprehensive tracking matrix & specifications |
| 2026-10-15 | Claude Code | ALL | Rewrote for the Rust implementation (CalcCraft 0.1.2): marked 20 completed features with their Rust modules and differences; split the gaps into `FEAT-STD-007` and `FEAT-SHELL-010`; added `FEAT-SHELL-008` (Settings) and `FEAT-SHELL-009` (app identity); fixed the standalone `%` result (0), the mode count (18), the Alt hotkey order, the converter unit lists and the Mach factor (from `UnitConverterDataLoader.cs`) and the source paths; replaced the `dotnet` commands with `cargo` ones |
| 2026-10-15 | Claude Code | `FEAT-CORE-003`, `FEAT-CORE-004`, `FEAT-STD-007` | Ported `CCalcEngine` and `CHistoryCollector` to `src/engine.rs` with the precedence flag (operator precedence, parentheses, precedence inversion brackets), completing `FEAT-CORE-005` and `FEAT-CORE-007` for precedence mode; added Windows' paste validation and key-by-key expression paste, and the history and memory context menus |
| 2026-10-15 | Claude Code | `FEAT-CONV-001`–`FEAT-CONV-013` | Added the 12 offline unit converters: `src/converter.rs` ports `UnitConverter`, the units, factors and region defaults of `UnitConverterDataLoader.cs` and the field logic of `UnitConverterViewModel.cs`; `src/app/converter_view.rs` draws `UnitConverter.xaml` (portrait and landscape). Updated `FEAT-SHELL-001` (Converter menu group) and `FEAT-SHELL-002`; Currency (`FEAT-CONV-014`) stays open |
| 2026-10-15 | Claude Code | `FEAT-SHELL-004` | Added the converters' "About equal to" results: `Converter::suggestions` ports `CalculateSuggested` with the everyday comparison units of `UnitConverterDataLoader.cs`, and `draw_suggestions` lays them out like `SupplementaryResults.xaml` and `HorizontalNoOverflowStackPanel` |
