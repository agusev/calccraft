//! Calculator engine: a port of the Windows Calculator's `CCalcEngine`
//! (`CalcManager/CEngine/scicomm.cpp`) and `CHistoryCollector` (`History.cpp`),
//! independent of the user interface.
//!
//! Standard mode applies operations immediately, without precedence:
//! `2 + 3 × 4 =` gives 20. With precedence on (`Calculator::with_precedence`,
//! for the Scientific mode) the same engine keeps a stack of pending operations
//! and parentheses: `2 + 3 × 4 =` gives 14. Numbers are exact fractions
//! (`BigRational`), so `0.1 + 0.2 − 0.3` is exactly 0 and `1 ÷ 3 × 3` is exactly 1.
//!
//! The state and the order of the steps follow the Windows source closely, so
//! the display and the expression line behave like Windows, quirks included.
//! The names of the Windows fields are given in the comments.

use std::cmp::Ordering;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

pub type Number = BigRational;

/// How many digits can be typed and how many significant digits the display shows.
const MAX_DIGITS: i64 = 16;
/// How many zeros after the decimal point plain notation allows when the number
/// does not fit completely (0.0033… stays plain, 0.00033… becomes 3.3…e-4).
const MAX_ZEROS_AFTER_DECIMAL: i64 = 2;
/// When a result's denominator grows beyond this many bits, the result is rounded
/// to `INTERNAL_DIGITS` significant digits so fractions don't grow without bound.
const MAX_DENOM_BITS: u64 = 256;
const INTERNAL_DIGITS: i64 = 40;
/// Largest decimal exponent; anything beyond it is "Overflow" (as in Windows).
const MAX_EXPONENT: i64 = 9999;
const HISTORY_LIMIT: usize = 20;
const MEMORY_LIMIT: usize = 100;
/// Deepest nesting of pending operations and of parentheses (`MAXPRECDEPTH`).
const MAX_DEPTH: usize = 25;
/// Paste limits from Windows' `CopyPasteManager`.
const MAX_PASTE_LENGTH: usize = 512;
const MAX_PASTE_OPERANDS: usize = 100;
const MAX_PASTE_EXPONENT_DIGITS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl Op {
    /// The symbol in the expression line: Windows' engine strings, where
    /// subtraction is a plain hyphen.
    fn symbol(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Subtract => "-",
            Op::Multiply => "×",
            Op::Divide => "÷",
        }
    }

    /// Higher binds tighter (`NPrecedenceOfOp`).
    fn precedence(self) -> u8 {
        match self {
            Op::Add | Op::Subtract => 2,
            Op::Multiply | Op::Divide => 3,
        }
    }
}

/// Precedence of a pending operation; no operation counts as the lowest (0).
fn precedence(op: Option<Op>) -> u8 {
    op.map_or(0, Op::precedence)
}

/// Unary functions applied to the current number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Func {
    Reciprocal,
    Square,
    SquareRoot,
    Negate,
}

impl Func {
    /// The function's name in the expression line, as in `sqr(5)`.
    fn name(self) -> &'static str {
        match self {
            Func::Reciprocal => "1/",
            Func::Square => "sqr",
            Func::SquareRoot => "√",
            Func::Negate => "negate",
        }
    }

    fn apply(self, v: &Number) -> Result<Number, CalcError> {
        match self {
            Func::Reciprocal if v.is_zero() => Err(CalcError::DivideByZero),
            Func::Reciprocal => normalize(v.recip()),
            Func::Square => normalize(v * v),
            Func::SquareRoot => sqrt(v),
            Func::Negate => Ok(-v),
        }
    }
}

/// Every command a button or a key can send to the calculator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Digit(u8),
    Decimal,
    Operator(Op),
    Function(Func),
    Percent,
    Equals,
    ClearEntry,
    Clear,
    Backspace,
    /// Parentheses work only with precedence on (Scientific mode).
    OpenParen,
    CloseParen,
    MemoryClear,
    MemoryRecall,
    MemoryAdd,
    MemorySubtract,
    MemoryStore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalcError {
    DivideByZero,
    Undefined,
    InvalidInput,
    Overflow,
}

impl CalcError {
    pub fn message(self) -> &'static str {
        match self {
            CalcError::DivideByZero => "Cannot divide by zero",
            CalcError::Undefined => "Result is undefined",
            CalcError::InvalidInput => "Invalid input",
            CalcError::Overflow => "Overflow",
        }
    }
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub expression: String,
    pub result: Number,
}

/// The kind of the previous command (`m_nLastCom`); many decisions depend on it.
/// `⌫` and the memory buttons except MR are "GUI settings" in Windows and
/// don't count as commands here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    None,
    Digit,
    Point,
    Operator(Op),
    Unary,
    Sign,
    OpenParen,
    CloseParen,
    Equals,
    Clear,
    ClearEntry,
    Recall,
}

impl Key {
    fn of(command: Command) -> Option<Key> {
        Some(match command {
            Command::Digit(_) => Key::Digit,
            Command::Decimal => Key::Point,
            Command::Operator(op) => Key::Operator(op),
            Command::Function(Func::Negate) => Key::Sign,
            Command::Function(_) | Command::Percent => Key::Unary,
            Command::Equals => Key::Equals,
            Command::ClearEntry => Key::ClearEntry,
            Command::Clear => Key::Clear,
            Command::OpenParen => Key::OpenParen,
            Command::CloseParen => Key::CloseParen,
            Command::MemoryRecall => Key::Recall,
            Command::Backspace
            | Command::MemoryClear
            | Command::MemoryAdd
            | Command::MemorySubtract
            | Command::MemoryStore => return None,
        })
    }
}

/// The number being typed (`CalcInput`, without exponents).
#[derive(Clone, Debug, Default)]
struct Input {
    negative: bool,
    /// Digits and at most one decimal point; empty means 0.
    digits: String,
}

impl Input {
    fn clear(&mut self) {
        *self = Self::default();
    }

    /// Adds a digit unless the 16-digit limit is reached. A leading zero is
    /// never stored, and `0.` doesn't count its zero against the limit.
    fn try_add_digit(&mut self, digit: u8) -> bool {
        let mut max = MAX_DIGITS as usize;
        if self.digits.contains('.') {
            max += 1;
        }
        if self.digits.starts_with('0') {
            max += 1;
        }
        if self.digits.is_empty() && digit == 0 {
            return true;
        }
        if self.digits.len() < max {
            self.digits.push(char::from(b'0' + digit));
            true
        } else {
            false
        }
    }

    fn try_add_point(&mut self) -> bool {
        if self.digits.contains('.') {
            return false;
        }
        if self.digits.is_empty() {
            self.digits.push('0');
        }
        self.digits.push('.');
        true
    }

    /// Flips the sign; an empty input (0) stays positive.
    fn toggle_sign(&mut self) {
        self.negative = !self.digits.is_empty() && !self.negative;
    }

    fn backspace(&mut self) {
        self.digits.pop();
        if self.digits == "0" {
            self.digits.pop();
        }
        if self.digits.is_empty() {
            self.clear();
        }
    }

    fn text(&self) -> String {
        let sign = if self.negative { "-" } else { "" };
        let digits = if self.digits.is_empty() {
            "0"
        } else {
            &self.digits
        };
        format!("{sign}{digits}")
    }

    fn value(&self) -> Number {
        parse_decimal(&self.text())
    }

    /// The input with thousands separators: `-1234.50` → `-1,234.50`.
    fn display(&self) -> String {
        let text = self.text();
        let (sign, unsigned) = match text.strip_prefix('-') {
            Some(rest) => ("-", rest),
            None => ("", text.as_str()),
        };
        match unsigned.split_once('.') {
            Some((int, frac)) => format!("{sign}{}.{frac}", group_thousands(int)),
            None => format!("{sign}{}", group_thousands(unsigned)),
        }
    }
}

/// A value and an operation waiting on one of the stacks. On the precedence
/// stack, `op: None` marks where a parenthesis was opened.
#[derive(Clone, Debug)]
struct Pending {
    value: Number,
    op: Option<Op>,
}

/// The expression line being built, as a list of tokens (`CHistoryCollector`).
/// Indices point into `tokens`; like in Windows, they are not adjusted when a
/// token is inserted before them.
#[derive(Clone, Debug, Default)]
struct HistoryLine {
    tokens: Vec<String>,
    /// Where the last operand starts (`m_lastOpStartIndex`).
    last_operand_start: Option<usize>,
    /// Where the last binary operator starts (`m_lastBinOpStartIndex`).
    last_bin_op_start: Option<usize>,
    /// Operand starts saved for precedence and parentheses (`m_operandIndices`).
    operand_starts: Vec<Option<usize>>,
    /// The last operand is a parenthesized group (`m_bLastOpndBrace`).
    last_operand_brace: bool,
}

impl HistoryLine {
    fn push(&mut self, token: impl Into<String>) -> usize {
        self.tokens.push(token.into());
        self.tokens.len() - 1
    }

    fn operand_added(&self) -> bool {
        self.last_operand_start.is_some()
    }

    fn add_operand(&mut self, text: String) {
        self.last_operand_start = Some(self.push(text));
        self.last_operand_brace = false;
        self.last_bin_op_start = None;
    }

    fn remove_last_operand(&mut self) {
        if let Some(start) = self.last_operand_start.take() {
            self.tokens.truncate(start);
        }
    }

    fn add_bin_op(&mut self, op: Op) {
        self.last_bin_op_start = Some(self.push(" "));
        self.push(op.symbol());
        self.push(" ");
        self.last_operand_start = None;
    }

    fn change_last_bin_op(&mut self, op: Op, precedence_inversion: bool) {
        if let Some(start) = self.last_bin_op_start {
            self.tokens.truncate(start);
        }
        if precedence_inversion {
            self.enclose_in_brackets();
        }
        self.add_bin_op(op);
    }

    fn push_operand_start(&mut self, start: Option<usize>) {
        if self.operand_starts.len() < MAX_DEPTH {
            self.operand_starts.push(start.or(self.last_operand_start));
        }
    }

    fn pop_operand_start(&mut self) {
        if let Some(start) = self.operand_starts.pop() {
            self.last_operand_start = start;
        }
    }

    fn add_open_brace(&mut self) {
        let start = self.push("(");
        self.push_operand_start(Some(start));
        self.last_bin_op_start = None;
    }

    fn add_close_brace(&mut self) {
        self.push(")");
        self.pop_operand_start();
        self.last_bin_op_start = None;
        self.last_operand_brace = true;
    }

    /// Puts brackets around the last operand group, when a precedence
    /// inversion turns `1 + 2 -` into `(1 + 2) ×`.
    fn enclose_in_brackets(&mut self) {
        let start = self.operand_starts.last().copied().flatten().unwrap_or(0);
        self.tokens.insert(start.min(self.tokens.len()), "(".into());
        self.push(")");
    }

    /// Wraps the last operand: `5` becomes `sqr(5)`, and a group `(1 + 2)` becomes `sqr(1 + 2)`.
    fn add_unary_op(&mut self, name: &str) {
        let text = if self.last_operand_brace {
            name.to_string()
        } else {
            format!("{name}(")
        };
        let at = self.last_operand_start.unwrap_or(self.tokens.len());
        self.tokens.insert(at.min(self.tokens.len()), text);
        if !self.last_operand_brace {
            self.push(")");
        }
        self.last_operand_brace = false;
        self.last_bin_op_start = None;
    }

    fn render(&self) -> String {
        let text: String = self
            .tokens
            .iter()
            .map(|token| if token == "=" { " =" } else { token })
            .collect();
        text.trim_end().to_string()
    }
}

#[derive(Clone, Debug)]
pub struct Calculator {
    /// Operator precedence and parentheses (`m_fPrecedence`, Scientific mode).
    precedence: bool,
    input: Input,
    /// A number is being typed into `input` (`m_bRecord`).
    record: bool,
    /// `m_currentVal`, `m_lastVal` and `m_holdVal` (the right operand kept for repeated `=`).
    current: Number,
    last: Number,
    hold: Number,
    /// The pending operation (`m_nOpCode`) and the last one done (`m_nPrevOpCode`).
    op: Option<Op>,
    prev_op: Option<Op>,
    /// `current` is the result of an operation and `op` is pending (`m_bChangeOp`).
    change_op: bool,
    /// No `=` since the last operator (`m_bNoPrevEqu`).
    no_prev_equ: bool,
    /// The previous command and the current one (`m_nLastCom`, `m_nTempCom`).
    last_key: Key,
    this_key: Key,
    precedence_stack: Vec<Pending>,
    paren_stack: Vec<Pending>,
    error: Option<CalcError>,
    line: HistoryLine,
    /// The expression line as last shown; Windows refreshes it only at certain steps.
    shown: String,
    memory: Vec<Number>,
    history: Vec<HistoryEntry>,
}

impl Default for Calculator {
    fn default() -> Self {
        Self {
            precedence: false,
            input: Input::default(),
            record: true,
            current: Number::zero(),
            last: Number::zero(),
            hold: Number::zero(),
            op: None,
            prev_op: None,
            change_op: false,
            no_prev_equ: true,
            last_key: Key::None,
            this_key: Key::None,
            precedence_stack: Vec::new(),
            paren_stack: Vec::new(),
            error: None,
            line: HistoryLine::default(),
            shown: String::new(),
            memory: Vec::new(),
            history: Vec::new(),
        }
    }
}

impl Calculator {
    /// An engine with operator precedence and parentheses, for the Scientific mode.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_precedence() -> Self {
        Self {
            precedence: true,
            ..Self::default()
        }
    }

    // --------------------------------------------------------------- queries

    /// The big number on the display (or the error message).
    pub fn display(&self) -> String {
        if let Some(error) = self.error {
            error.message().to_string()
        } else if self.record {
            self.input.display()
        } else {
            format_number(&self.current)
        }
    }

    /// The expression line above the display, e.g. `5 + sqr(3)`.
    pub fn expression(&self) -> String {
        self.shown.clone()
    }

    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    /// How many parentheses are open (the `(` counter of the Scientific mode).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn open_parentheses(&self) -> usize {
        self.paren_stack.len()
    }

    pub fn memory(&self) -> &[Number] {
        &self.memory
    }

    /// Calculation history, newest first.
    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    /// Whether a button is enabled (most buttons are disabled in the error state).
    pub fn is_enabled(&self, command: Command) -> bool {
        match command {
            Command::Digit(_)
            | Command::Decimal
            | Command::Clear
            | Command::ClearEntry
            | Command::Backspace => true,
            Command::MemoryClear | Command::MemoryRecall => {
                self.error.is_none() && !self.memory.is_empty()
            }
            Command::OpenParen | Command::CloseParen => self.precedence && self.error.is_none(),
            _ => self.error.is_none(),
        }
    }

    /// Text to copy to the clipboard (without thousands separators).
    pub fn copy_text(&self) -> Option<String> {
        if self.error.is_some() {
            None
        } else {
            Some(self.display().replace(',', ""))
        }
    }

    // -------------------------------------------------------------- commands

    pub fn execute(&mut self, command: Command) {
        if !self.is_enabled(command) {
            return;
        }
        if self.error.is_some() {
            // Windows' view model clears the error first; only digits and the
            // decimal point then go on to start a new number.
            self.process(Command::Clear);
            if !matches!(command, Command::Digit(_) | Command::Decimal) {
                return;
            }
        }
        match command {
            Command::MemoryStore => self.memory_store(),
            Command::MemoryAdd => self.memory_add(0, false),
            Command::MemorySubtract => self.memory_add(0, true),
            Command::MemoryRecall => self.memory_recall_item(0),
            Command::MemoryClear => {
                self.process(command);
                self.memory.clear();
            }
            _ => self.process(command),
        }
    }

    /// One command through the engine (`ProcessCommandWorker`).
    fn process(&mut self, command: Command) {
        self.begin(command);
        match command {
            Command::Digit(digit) => self.digit(digit),
            Command::Decimal => self.point(),
            Command::Operator(op) => self.binary(op),
            Command::Function(Func::Negate) => self.negate(),
            Command::Function(func) => self.unary(func),
            Command::Percent => self.percent(),
            Command::Equals => self.equals(),
            Command::Clear => self.clear(),
            Command::ClearEntry => self.clear_entry(),
            Command::Backspace => {
                if self.record {
                    self.input.backspace();
                }
            }
            Command::OpenParen => self.open_paren(),
            Command::CloseParen => self.close_paren(),
            // Only the steps in `begin` (finishing the typed number) apply here.
            Command::MemoryClear
            | Command::MemoryRecall
            | Command::MemoryAdd
            | Command::MemorySubtract
            | Command::MemoryStore => {}
        }
    }

    /// The steps every command starts with.
    fn begin(&mut self, command: Command) {
        if let Some(key) = Key::of(command) {
            self.last_key = self.this_key;
            self.this_key = key;
        }
        // Any action hides the expression shown after `=`.
        if !self.no_prev_equ {
            self.shown.clear();
        }
        let ends_typing = matches!(
            command,
            Command::Operator(_)
                | Command::Percent
                | Command::Equals
                | Command::OpenParen
                | Command::CloseParen
                | Command::MemoryClear
                | Command::MemoryRecall
                | Command::MemoryAdd
                | Command::MemorySubtract
                | Command::MemoryStore
        ) || matches!(command, Command::Function(func) if func != Func::Negate);
        if self.record {
            if ends_typing {
                self.record = false;
                self.current = self.input.value();
            }
        } else if matches!(command, Command::Digit(_) | Command::Decimal) {
            self.record = true;
            self.input.clear();
            // `(8)2` continues the equation instead of starting a new one.
            if self.last_key != Key::CloseParen {
                self.check_and_add_last_bin_op_to_history(true);
            }
        }
    }

    /// An invalid command is ignored as if it never happened (`HandleErrorCommand`).
    fn forget_command(&mut self) {
        self.this_key = self.last_key;
    }

    fn digit(&mut self, digit: u8) {
        if !self.input.try_add_digit(digit) {
            return self.forget_command();
        }
        if self.last_key == Key::CloseParen {
            self.implicit_multiplication();
        }
    }

    fn point(&mut self) {
        if self.last_key == Key::CloseParen {
            self.implicit_multiplication();
        }
        if !(self.record && self.input.try_add_point()) {
            self.forget_command();
        }
    }

    /// A number right after `)` multiplies: `(8)2` is `(8) × 2`.
    fn implicit_multiplication(&mut self) {
        self.op = Some(Op::Multiply);
        self.last = self.current.clone();
        self.hold = Number::zero();
        self.no_prev_equ = true;
        if !self.line.operand_added() {
            self.line.add_open_brace();
            self.line.add_operand(self.number_text());
            self.line.add_close_brace();
        }
        self.add_bin_op(Op::Multiply, true);
        self.change_op = true;
        self.prev_op = None;
        self.precedence_stack.clear();
    }

    fn binary(&mut self, op: Op) {
        // Another operator right after one replaces it.
        if let Key::Operator(replaced) = self.last_key {
            self.op = Some(op);
            // `1 + 2 -` changed to `×`: 1 + 2 was already computed, so it gets brackets.
            let mut inversion = false;
            if self.precedence && self.prev_op.is_some() {
                let previous = precedence(self.prev_op);
                if replaced.precedence() <= previous && op.precedence() > previous {
                    inversion = true;
                    self.prev_op = None;
                }
            }
            self.line.change_last_bin_op(op, inversion);
            self.refresh();
            return;
        }

        if !self.line.operand_added() {
            self.line.add_operand(self.number_text());
        }
        if self.change_op {
            loop {
                let new = op.precedence();
                let pending = precedence(self.op);
                if new > pending && self.precedence {
                    // The new operator binds tighter: keep the pending one for later.
                    if self.precedence_stack.len() < MAX_DEPTH {
                        self.precedence_stack.push(Pending {
                            value: self.last.clone(),
                            op: self.op,
                        });
                        self.line.push_operand_start(None);
                    } else {
                        self.forget_command();
                    }
                    break;
                }
                self.current = self.operate(self.op);
                self.prev_op = self.op;
                if self.error.is_none() && !self.precedence {
                    // Standard mode logs every step and continues from its result.
                    self.complete_equation();
                    self.line.add_operand(self.number_text());
                }
                match self.precedence_stack.last() {
                    Some(Pending { op: Some(_), .. }) => {
                        let Pending { value, op: popped } = self.precedence_stack.pop().unwrap();
                        self.op = popped;
                        self.last = value;
                        if pending <= precedence(popped) {
                            self.line.enclose_in_brackets();
                        }
                        self.line.pop_operand_start();
                    }
                    _ => break,
                }
            }
        }
        self.last = self.current.clone();
        self.op = Some(op);
        self.add_bin_op(op, true);
        self.no_prev_equ = true;
        self.change_op = true;
    }

    fn unary(&mut self, func: Func) {
        // `5 + √` works on the 5 (so `5 + 1/x` isn't a division by zero).
        if matches!(self.last_key, Key::Operator(_)) {
            self.current = self.last.clone();
        }
        if !self.line.operand_added() {
            self.line.add_operand(self.number_text());
        }
        self.line.add_unary_op(func.name());
        self.refresh();
        match func.apply(&self.current) {
            Ok(result) => self.current = result,
            Err(error) => self.display_error(error),
        }
    }

    fn negate(&mut self) {
        // While typing, ± just flips the sign of the number being typed.
        if self.record {
            self.input.toggle_sign();
            return;
        }
        if matches!(self.last_key, Key::Operator(_)) {
            self.current = self.last.clone();
        }
        if !self.line.operand_added() {
            self.line.add_operand(self.number_text());
        }
        self.current = -self.current.clone();
        self.line.add_unary_op(Func::Negate.name());
        self.refresh();
    }

    /// `%` as in Windows: after `×` or `÷` the value divided by 100, otherwise a
    /// percentage of the left operand (0 when there is none). The expression
    /// line shows the result, not a `%` sign.
    fn percent(&mut self) {
        if matches!(self.last_key, Key::Operator(_)) {
            self.current = self.last.clone();
        }
        let hundred = Number::from_integer(100.into());
        let result = if matches!(self.op, Some(Op::Multiply | Op::Divide)) {
            &self.current / hundred
        } else {
            &self.current * (&self.last / hundred)
        };
        match normalize(result) {
            Ok(result) => self.current = result,
            Err(error) => return self.display_error(error),
        }
        self.check_and_add_last_bin_op_to_history(true);
        self.line.add_operand(self.number_text());
        self.refresh();
    }

    fn equals(&mut self) {
        // `=` closes all open parentheses first.
        while !self.paren_stack.is_empty() {
            if self.error.is_some() {
                break;
            }
            self.this_key = self.last_key;
            self.process(Command::CloseParen);
            self.last_key = self.this_key;
            self.this_key = Key::Equals;
        }
        if !self.no_prev_equ {
            self.last = self.current.clone();
        }
        // `5 + =` repeats the left operand: 5 + 5.
        if matches!(self.last_key, Key::Operator(_)) {
            self.current = self.last.clone();
        }
        if !self.line.operand_added() {
            self.line.add_operand(self.number_text());
        }
        self.resolve_pending();
        while self.precedence {
            let Some(Pending { value, op }) = self.precedence_stack.pop() else {
                break;
            };
            self.op = op;
            self.last = value;
            if precedence(self.prev_op) <= precedence(op) {
                self.line.enclose_in_brackets();
            }
            self.line.pop_operand_start();
            self.no_prev_equ = true;
            self.resolve_pending();
        }
        if self.error.is_none() {
            self.complete_equation();
            self.last = self.current.clone();
            self.prev_op = None;
            self.precedence_stack.clear();
        }
        self.change_op = false;
    }

    /// Does the pending operation; repeated `=` reuses the right operand
    /// (`ResolveHighestPrecedenceOperation`).
    fn resolve_pending(&mut self) {
        let Some(op) = self.op else {
            return;
        };
        if self.no_prev_equ {
            self.hold = self.current.clone();
        } else {
            self.current = self.hold.clone();
            self.line.add_bin_op(op);
            self.line.add_operand(self.number_text());
        }
        self.current = self.operate(Some(op));
        self.prev_op = Some(op);
        self.last = self.current.clone();
        self.no_prev_equ = false;
    }

    fn open_paren(&mut self) {
        let stack_full = matches!(
            self.precedence_stack.last(),
            Some(Pending { op: Some(_), .. })
        ) && self.precedence_stack.len() >= MAX_DEPTH;
        if self.paren_stack.len() >= MAX_DEPTH || stack_full {
            return self.forget_command();
        }
        // A number or `)` before `(` multiplies: `2(3)` is `2 × (3)`.
        if matches!(
            self.last_key,
            Key::Digit | Key::Unary | Key::Point | Key::CloseParen
        ) {
            self.process(Command::Operator(Op::Multiply));
        }
        self.check_and_add_last_bin_op_to_history(true);
        self.line.add_open_brace();
        self.refresh();
        self.paren_stack.push(Pending {
            value: self.last.clone(),
            op: if self.change_op { self.op } else { None },
        });
        if self.precedence_stack.len() < MAX_DEPTH {
            self.precedence_stack.push(Pending {
                value: Number::zero(),
                op: None,
            });
        }
        self.last = Number::zero();
        // `1 + (` starts the group at 0, but `1 + 3 (` starts it at 3.
        if matches!(self.last_key, Key::Operator(_)) {
            self.current = Number::zero();
        }
        self.this_key = Key::None;
        self.op = None;
        self.change_op = false;
    }

    fn close_paren(&mut self) {
        if self.paren_stack.is_empty() {
            return self.forget_command();
        }
        if matches!(self.last_key, Key::Operator(_)) {
            self.current = self.last.clone();
        }
        if !self.line.operand_added() {
            self.line.add_operand(self.number_text());
        }
        self.current = self.operate(self.op);
        self.prev_op = self.op;
        // Resolve everything pending inside the parentheses.
        while let Some(Pending {
            value,
            op: Some(op),
        }) = self.precedence_stack.pop()
        {
            if precedence(self.prev_op) <= op.precedence() {
                self.line.enclose_in_brackets();
            }
            self.line.pop_operand_start();
            self.op = Some(op);
            self.last = value;
            self.current = self.operate(Some(op));
            self.prev_op = Some(op);
        }
        self.line.add_close_brace();
        self.refresh();
        let Pending { value, op } = self.paren_stack.pop().expect("checked above");
        self.last = value;
        self.op = op;
        self.change_op = op.is_some();
    }

    /// Resets the calculation; memory and history are kept (`IDC_CLEAR`).
    fn clear(&mut self) {
        if !self.change_op {
            self.check_and_add_last_bin_op_to_history(false);
        }
        self.last = Number::zero();
        self.change_op = false;
        self.paren_stack.clear();
        self.precedence_stack.clear();
        self.last_key = Key::None;
        self.this_key = Key::None;
        self.op = None;
        self.prev_op = None;
        self.no_prev_equ = true;
        self.shown.clear();
        self.line = HistoryLine::default();
        self.clear_entry();
    }

    /// Clears the number being entered (`ClearTemporaryValues`).
    fn clear_entry(&mut self) {
        self.input.clear();
        self.record = true;
        self.check_and_add_last_bin_op_to_history(true);
        self.error = None;
    }

    /// `current op` applied to `last` (`DoOperation`); on an error `current` stays.
    fn operate(&mut self, op: Option<Op>) -> Number {
        let Some(op) = op else {
            return self.current.clone();
        };
        match apply(&self.last, op, &self.current) {
            Ok(result) => result,
            Err(error) => {
                self.display_error(error);
                self.current.clone()
            }
        }
    }

    /// Shows an error. The expression line stays as it is.
    fn display_error(&mut self, error: CalcError) {
        self.error = Some(error);
    }

    /// The current value as it appears in the expression line.
    fn number_text(&self) -> String {
        expression_number(&self.current)
    }

    // -------------------------------------------------------- expression line

    fn refresh(&mut self) {
        self.shown = self.line.render();
    }

    fn add_bin_op(&mut self, op: Op, refresh: bool) {
        self.line.add_bin_op(op);
        if refresh {
            self.refresh();
        }
    }

    /// Ends the equation with `=` and logs it.
    fn complete_equation(&mut self) {
        self.line.push("=");
        self.refresh();
        self.complete_history_line();
    }

    /// Logs the expression so far with the current result and starts a new one.
    fn complete_history_line(&mut self) {
        let expression = self.line.render();
        self.history.insert(
            0,
            HistoryEntry {
                expression,
                result: self.current.clone(),
            },
        );
        self.history.truncate(HISTORY_LIMIT);
        self.line = HistoryLine::default();
    }

    /// Makes the expression line end with a binary operator again when a new
    /// operand starts (`CheckAndAddLastBinOpToHistory`). After `1 + √(4)`, typing
    /// replaces `√(4)`; after a finished `√(4)`, typing logs it and starts anew.
    fn check_and_add_last_bin_op_to_history(&mut self, add_to_history: bool) {
        if self.change_op {
            if self.line.operand_added() {
                self.line.remove_last_operand();
                self.refresh();
            }
        } else if self.line.operand_added() && self.error.is_none() {
            let finished = matches!(self.last_key, Key::Unary | Key::Sign | Key::CloseParen)
                && self.paren_stack.is_empty();
            if finished {
                if add_to_history {
                    self.complete_history_line();
                }
            } else {
                self.line.remove_last_operand();
                self.refresh();
            }
        }
    }

    // ---------------------------------------------------- memory and history

    fn memory_store(&mut self) {
        self.process(Command::MemoryStore);
        self.memory.insert(0, self.current.clone());
        self.memory.truncate(MEMORY_LIMIT);
    }

    /// Puts memory slot `index` on the display (MR, or a click in the memory list).
    pub fn memory_recall_item(&mut self, index: usize) {
        let Some(value) = self.memory.get(index).cloned() else {
            return;
        };
        if self.error.is_some() {
            return;
        }
        self.begin(Command::MemoryRecall);
        self.current = value;
        self.check_and_add_last_bin_op_to_history(true);
    }

    /// M+ (or M− when `subtract`) for memory slot `index`. With an empty memory,
    /// M+ stores the display and M− stores its negative.
    pub fn memory_add(&mut self, index: usize, subtract: bool) {
        if self.error.is_some() {
            return;
        }
        if self.memory.is_empty() {
            self.memory_store();
            if subtract {
                self.memory[0] = -&self.memory[0];
            }
            return;
        }
        let command = if subtract {
            Command::MemorySubtract
        } else {
            Command::MemoryAdd
        };
        self.process(command);
        if let Some(cell) = self.memory.get_mut(index) {
            let sum = if subtract {
                &*cell - &self.current
            } else {
                &*cell + &self.current
            };
            if let Ok(sum) = normalize(sum) {
                *cell = sum;
            }
        }
    }

    pub fn memory_clear_item(&mut self, index: usize) {
        if index < self.memory.len() {
            self.memory.remove(index);
        }
    }

    /// Shows a history entry's result and expression, as after `=`.
    pub fn recall_history(&mut self, index: usize) {
        let Some(entry) = self.history.get(index).cloned() else {
            return;
        };
        self.process(Command::Clear);
        self.current = entry.result.clone();
        self.last = entry.result;
        self.record = false;
        self.shown = entry.expression;
        // The next key clears the expression line, as after `=`.
        self.no_prev_equ = false;
        self.this_key = Key::Equals;
    }

    pub fn delete_history_item(&mut self, index: usize) {
        if index < self.history.len() {
            self.history.remove(index);
        }
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Pastes text the way Windows does: the text is checked with the rules of
    /// `CopyPasteManager`, then typed in key by key after a `CE`, so `12 + 5 =`
    /// calculates 17. Invalid text shows `Invalid input`. Returns whether the
    /// text was accepted.
    pub fn paste(&mut self, text: &str) -> bool {
        if !paste_is_valid(text, self.precedence) {
            self.display_error(CalcError::InvalidInput);
            return false;
        }
        self.execute(Command::ClearEntry);

        // A `-` at the start or after an operator is a sign. Windows presses ±
        // after the first digit that isn't 0, and remembers a sign before `(`
        // until the matching `)`.
        let mut first = true;
        let mut previous_operator = false;
        let mut negate = false;
        let mut negate_stack = Vec::new();
        for c in text.chars() {
            let (command, mut can_negate) = match c {
                '0'..='9' => (Command::Digit(c as u8 - b'0'), true),
                '.' => (Command::Decimal, false),
                '+' => (Command::Operator(Op::Add), false),
                '-' => (Command::Operator(Op::Subtract), false),
                '*' => (Command::Operator(Op::Multiply), false),
                '/' => (Command::Operator(Op::Divide), false),
                '=' => (Command::Equals, false),
                '(' => (Command::OpenParen, false),
                ')' => (Command::CloseParen, false),
                _ => continue,
            };
            let mut send = true;
            if first || previous_operator {
                first = false;
                previous_operator = false;
                match command {
                    Command::Operator(Op::Subtract) => {
                        negate = true;
                        send = false;
                    }
                    Command::Operator(Op::Add) => send = false,
                    _ => {}
                }
            }
            match command {
                Command::OpenParen => {
                    negate_stack.push(negate);
                    negate = false;
                }
                Command::CloseParen => match negate_stack.pop() {
                    Some(saved) => {
                        negate = saved;
                        can_negate = true;
                    }
                    None => send = false,
                },
                Command::Operator(_) => previous_operator = true,
                _ => {}
            }
            if send {
                self.execute(command);
                if negate {
                    if can_negate {
                        self.execute(Command::Function(Func::Negate));
                    }
                    if !matches!(command, Command::Digit(0) | Command::Decimal) {
                        negate = false;
                    }
                }
            }
        }
        true
    }
}

// ----------------------------------------------------------------- paste

/// Checks pasted text like Windows' `CopyPasteManager::ValidatePasteExpression`.
/// Numbers in e-notation are rejected: the engine has no exponent entry yet.
fn paste_is_valid(text: &str, scientific: bool) -> bool {
    const UNWANTED: [char; 19] = [
        ' ', ',', '"', '¥', '¤', '₵', '$', '₡', '₩', '₪', '₦', '₹', '£', '€', '\u{202a}',
        '\u{202b}', '\u{202c}', '\u{202d}', '\u{a0}',
    ];
    if text.chars().count() > MAX_PASTE_LENGTH {
        return false;
    }
    let mut expression: String = text.chars().filter(|c| !UNWANTED.contains(c)).collect();
    if expression.ends_with('=') {
        expression.pop();
    }
    if scientific && !expression.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    let operands = extract_operands(&expression, scientific);
    let max_length = if scientific { 32 } else { MAX_DIGITS as usize };
    !operands.is_empty()
        && operands.iter().all(|operand| {
            operand_matches(operand, scientific) && operand_length(operand) <= max_length
        })
}

/// Splits pasted text into operands at the binary operators
/// (`CopyPasteManager::ExtractOperands`). A sign is not an operator.
fn extract_operands(expression: &str, scientific: bool) -> Vec<String> {
    let valid = if scientific {
        "0123456789+-.e*/()^%"
    } else {
        "0123456789+-.e*/"
    };
    let chars: Vec<char> = expression.chars().collect();
    let mut operands = Vec::new();
    let mut last_index = 0;
    let mut have_operator = false;
    let mut counting_exponent = false;
    let mut exponent_length = 0;
    let mut start_of_expression = true;
    let mut previous_open_paren = false;
    let mut previous_operator = false;
    for (i, &c) in chars.iter().enumerate() {
        if !valid.contains(c) {
            continue;
        }
        if operands.len() >= MAX_PASTE_OPERANDS {
            return Vec::new();
        }
        if c.is_ascii_digit() {
            if counting_exponent {
                exponent_length += 1;
                if exponent_length > MAX_PASTE_EXPONENT_DIGITS {
                    return Vec::new();
                }
            }
            previous_operator = false;
        } else if c == 'e' {
            counting_exponent = true;
            previous_operator = false;
        } else if "+-*/^%".contains(c) {
            let is_sign = (c == '+' || c == '-')
                && (previous_open_paren
                    || start_of_expression
                    || previous_operator
                    || i == 0
                    || chars[i - 1] == 'e');
            if is_sign {
                previous_operator = false;
                continue;
            }
            counting_exponent = false;
            exponent_length = 0;
            have_operator = true;
            previous_operator = true;
            operands.push(chars[last_index..i].iter().collect());
            last_index = i + 1;
        } else {
            previous_operator = false;
        }
        previous_open_paren = c == '(';
        start_of_expression = false;
    }
    if have_operator {
        operands.push(chars[last_index..].iter().collect());
    } else {
        operands = vec![expression.to_string()];
    }
    operands
}

/// Whether an operand is a number: `\s*[-+]?(\d+(\.\d*)?|\.\d+)\s*`, and in the
/// Scientific mode with opening parentheses (each may be signed) before it and
/// closing ones after it, or just an optional sign.
fn operand_matches(operand: &str, scientific: bool) -> bool {
    let mut rest = operand.trim_start();
    if scientific {
        if matches!(rest.trim_end(), "" | "+" | "-") {
            return true;
        }
        while let Some(after) = rest
            .strip_prefix(['+', '-'])
            .unwrap_or(rest)
            .strip_prefix('(')
        {
            rest = after;
        }
        rest = rest.trim_start();
    }
    rest = rest.strip_prefix(['+', '-']).unwrap_or(rest);
    let integer = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    rest = &rest[integer..];
    let mut fraction = 0;
    if let Some(after) = rest.strip_prefix('.') {
        fraction = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        rest = &after[fraction..];
    }
    if integer == 0 && fraction == 0 {
        return false;
    }
    if scientific {
        rest = rest.trim_start().trim_start_matches(')');
    }
    rest.trim().is_empty()
}

/// Digits that count towards the operand length limit
/// (`StandardScientificOperandLength` after `SanitizeOperand`).
fn operand_length(operand: &str) -> usize {
    let sanitized: String = operand
        .chars()
        .filter(|c| !matches!(c, '\'' | '_' | '`' | '(' | ')' | '-' | '+'))
        .collect();
    let length = sanitized.chars().count();
    if sanitized.contains('.') && length >= 2 {
        if sanitized.starts_with("0.") {
            length - 2
        } else {
            length - 1
        }
    } else {
        length
    }
}

// ------------------------------------------------------------- arithmetic

/// `left op right`, with Windows' division errors.
fn apply(left: &Number, op: Op, right: &Number) -> Result<Number, CalcError> {
    let result = match op {
        Op::Add => left + right,
        Op::Subtract => left - right,
        Op::Multiply => left * right,
        Op::Divide if right.is_zero() && left.is_zero() => return Err(CalcError::Undefined),
        Op::Divide if right.is_zero() => return Err(CalcError::DivideByZero),
        Op::Divide => left / right,
    };
    normalize(result)
}

/// √(p/q) = √(p·q)/q. Scaling by 10^k keeps at least `INTERNAL_DIGITS` correct
/// digits, and perfect squares (√9, √0.25) give exact results.
fn sqrt(v: &Number) -> Result<Number, CalcError> {
    if v.is_negative() {
        return Err(CalcError::InvalidInput);
    }
    let (p, q) = (v.numer(), v.denom());
    let k = INTERNAL_DIGITS as u32 + (q.bits() / 3) as u32 + 1;
    let root = (p * q * pow10(2 * k as i64)).sqrt();
    normalize(Number::new(root, q * pow10(k as i64)))
}

/// Checks for overflow and keeps the denominator from growing without bound.
fn normalize(v: Number) -> Result<Number, CalcError> {
    if v.is_zero() {
        return Ok(v);
    }
    let magnitude = v.numer().abs();
    let e = exponent(&magnitude, v.denom());
    if e.abs() > MAX_EXPONENT {
        return Err(CalcError::Overflow);
    }
    if v.denom().bits() <= MAX_DENOM_BITS {
        return Ok(v);
    }
    let k = INTERNAL_DIGITS - 1 - e;
    let mut digits = scaled_round(&magnitude, v.denom(), k);
    if v.is_negative() {
        digits = -digits;
    }
    Ok(if k >= 0 {
        Number::new(digits, pow10(k))
    } else {
        Number::from_integer(digits * pow10(-k))
    })
}

fn pow10(n: i64) -> BigInt {
    BigInt::from(10u8).pow(n as u32)
}

/// Decimal exponent of a/q (a > 0): the e with 10^e ≤ a/q < 10^(e+1).
fn exponent(a: &BigInt, q: &BigInt) -> i64 {
    // Compares a/q with 10^e without dividing.
    let compare = |e: i64| {
        if e >= 0 {
            a.cmp(&(q * pow10(e)))
        } else {
            (a * pow10(-e)).cmp(q)
        }
    };
    let bits = a.bits() as f64 - q.bits() as f64;
    let mut e = (bits * std::f64::consts::LOG10_2).floor() as i64;
    while compare(e) == Ordering::Less {
        e -= 1;
    }
    while compare(e + 1) != Ordering::Less {
        e += 1;
    }
    e
}

/// Rounds a/q · 10^k to an integer (halves away from zero).
fn scaled_round(a: &BigInt, q: &BigInt, k: i64) -> BigInt {
    let (n, d) = if k >= 0 {
        (a * pow10(k), q.clone())
    } else {
        (a.clone(), q * pow10(-k))
    };
    (n * 2 + &d) / (d * 2)
}

// ------------------------------------------------------------- formatting

/// A number for the display, with thousands separators.
pub fn format_number(v: &Number) -> String {
    format_with(v, true)
}

/// A number for the expression line; Windows writes it without separators.
fn expression_number(v: &Number) -> String {
    format_with(v, false)
}

/// Formats a number like Windows: up to 16 significant digits, scientific notation
/// for large numbers and for small ones that don't fit into 16 decimal places.
fn format_with(v: &Number, grouping: bool) -> String {
    if v.is_zero() {
        return "0".to_string();
    }
    let sign = if v.is_negative() { "-" } else { "" };
    let a = v.numer().abs();
    let q = v.denom();
    let mut e = exponent(&a, q);
    let mut digits = scaled_round(&a, q, MAX_DIGITS - 1 - e);
    // Rounding may have added a digit (9.99…96 → 10.00…0).
    if digits >= pow10(MAX_DIGITS) {
        digits /= 10;
        e += 1;
    }
    let digits = digits.to_string();

    if (0..MAX_DIGITS).contains(&e) {
        let (int, frac) = digits.split_at(e as usize + 1);
        let int = if grouping {
            group_thousands(int)
        } else {
            int.to_string()
        };
        let frac = frac.trim_end_matches('0');
        let point = if frac.is_empty() { "" } else { "." };
        return format!("{sign}{int}{point}{frac}");
    }
    if e < 0 {
        // Zeros after the decimal point before the first significant digit.
        let zeros = -e - 1;
        if zeros <= MAX_ZEROS_AFTER_DECIMAL {
            // 0.0033333333333333: simply 16 digits after the point.
            let frac = scaled_round(&a, q, MAX_DIGITS).to_string();
            let frac = format!("{frac:0>width$}", width = MAX_DIGITS as usize);
            return format!("{sign}0.{}", frac.trim_end_matches('0'));
        }
        // 0.000125: the whole number fits into 16 digits after the point.
        let significant = digits.trim_end_matches('0');
        if zeros + significant.len() as i64 <= MAX_DIGITS {
            return format!("{sign}0.{}{significant}", "0".repeat(zeros as usize));
        }
    }

    let (first, rest) = digits.split_at(1);
    let exp_sign = if e >= 0 { "+" } else { "-" };
    format!(
        "{sign}{first}.{}e{exp_sign}{}",
        rest.trim_end_matches('0'),
        e.abs()
    )
}

fn group_thousands(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn parse_decimal(input: &str) -> Number {
    let (negative, unsigned) = match input.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, input),
    };
    let (int, frac) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let digits = format!("{int}{frac}");
    let numer: BigInt = if digits.is_empty() {
        BigInt::zero()
    } else {
        digits.parse().unwrap()
    };
    let v = Number::new(numer, pow10(frac.len() as i64));
    if negative { -v } else { v }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Presses keys given as a string: digits, `.`, `+-*/`, `=`, `%`, `(`, `)`,
    /// `n` = ±, `s` = x², `r` = √, `i` = 1/x, `c` = C, `e` = CE, `<` = ⌫.
    fn press(calc: &mut Calculator, keys: &str) {
        for key in keys.chars() {
            let command = match key {
                '0'..='9' => Command::Digit(key as u8 - b'0'),
                '.' => Command::Decimal,
                '+' => Command::Operator(Op::Add),
                '-' => Command::Operator(Op::Subtract),
                '*' => Command::Operator(Op::Multiply),
                '/' => Command::Operator(Op::Divide),
                '=' => Command::Equals,
                '%' => Command::Percent,
                '(' => Command::OpenParen,
                ')' => Command::CloseParen,
                'n' => Command::Function(Func::Negate),
                's' => Command::Function(Func::Square),
                'r' => Command::Function(Func::SquareRoot),
                'i' => Command::Function(Func::Reciprocal),
                'c' => Command::Clear,
                'e' => Command::ClearEntry,
                '<' => Command::Backspace,
                ' ' => continue,
                _ => panic!("unknown key {key}"),
            };
            calc.execute(command);
        }
    }

    fn run(keys: &str) -> (String, String) {
        let mut calc = Calculator::default();
        press(&mut calc, keys);
        (calc.display(), calc.expression())
    }

    /// Like `run`, with operator precedence and parentheses (Scientific mode).
    fn run_precedence(keys: &str) -> (String, String) {
        let mut calc = Calculator::with_precedence();
        press(&mut calc, keys);
        (calc.display(), calc.expression())
    }

    fn history(calc: &Calculator) -> Vec<(String, String)> {
        calc.history()
            .iter()
            .map(|e| (e.expression.clone(), format_number(&e.result)))
            .collect()
    }

    #[test]
    fn immediate_execution_without_precedence() {
        assert_eq!(run("2+3*4="), ("20".into(), "5 × 4 =".into()));
    }

    #[test]
    fn chained_operator_shows_intermediate_result() {
        assert_eq!(run("2+3+"), ("5".into(), "5 +".into()));
    }

    #[test]
    fn operator_can_be_replaced() {
        assert_eq!(run("5+*3=").0, "15");
    }

    #[test]
    fn exact_decimal_arithmetic() {
        assert_eq!(run("0.1+0.2=").0, "0.3");
        assert_eq!(run("0.1+0.2-0.3=").0, "0");
        assert_eq!(run("1/3*3=").0, "1");
        assert_eq!(run("1/3*3-1=").0, "0");
    }

    #[test]
    fn subtraction_uses_a_hyphen() {
        assert_eq!(run("9-4="), ("5".into(), "9 - 4 =".into()));
    }

    #[test]
    fn rounding_to_sixteen_digits() {
        assert_eq!(run("1/3=").0, "0.3333333333333333");
        assert_eq!(run("2/3=").0, "0.6666666666666667");
        assert_eq!(run("1/300=").0, "0.0033333333333333");
    }

    #[test]
    fn scientific_notation_for_large_and_small() {
        assert_eq!(run("9999999999999999+1=").0, "1.e+16");
        assert_eq!(run("1234567890123456*1000=").0, "1.234567890123456e+18");
        assert_eq!(run("1/3/1000000000000000/10=").0, "3.333333333333333e-17");
        assert_eq!(run("1/7000000000000000=").0, "1.428571428571429e-16");
        assert_eq!(run("1/3000=").0, "3.333333333333333e-4");
        assert_eq!(run("0.001/8=").0, "0.000125");
        assert_eq!(run("0.0000000000001/8=").0, "0.0000000000000125");
        assert_eq!(run("0.0001/10=").0, "0.00001");
    }

    #[test]
    fn thousands_separators() {
        assert_eq!(run("1234567").0, "1,234,567");
        assert_eq!(run("1234.50").0, "1,234.50");
        assert_eq!(run("1000*1000=").0, "1,000,000");
        assert_eq!(run("1000*1000=").1, "1000 × 1000 =");
        assert_eq!(run("1234567+").1, "1234567 +");
    }

    #[test]
    fn equals_repeats_last_operation() {
        assert_eq!(run("5+3=="), ("11".into(), "8 + 3 =".into()));
        assert_eq!(run("5+3==="), ("14".into(), "11 + 3 =".into()));
        assert_eq!(run("5+="), ("10".into(), "5 + 5 =".into()));
        assert_eq!(run("2+3=7="), ("10".into(), "7 + 3 =".into()));
    }

    #[test]
    fn unary_functions_build_expression() {
        assert_eq!(run("9r"), ("3".into(), "√(9)".into()));
        assert_eq!(run("4ss"), ("256".into(), "sqr(sqr(4))".into()));
        assert_eq!(run("4i"), ("0.25".into(), "1/(4)".into()));
        assert_eq!(run("5+3s"), ("9".into(), "5 + sqr(3)".into()));
        assert_eq!(run("5+3s="), ("14".into(), "5 + sqr(3) =".into()));
        assert_eq!(run("2+3=r").1, "√(5)");
        assert_eq!(run("8=n"), ("-8".into(), "negate(8)".into()));
        // A function right after an operator works on the left operand.
        assert_eq!(run("5+r"), ("2.23606797749979".into(), "5 + √(5)".into()));
    }

    #[test]
    fn square_root_precision() {
        assert_eq!(run("2r").0, "1.414213562373095");
        assert_eq!(run("2rs").0, "2");
        assert_eq!(run("0.25r").0, "0.5");
    }

    #[test]
    fn negate_while_typing() {
        assert_eq!(run("5n"), ("-5".into(), "".into()));
        assert_eq!(run("5nn").0, "5");
        assert_eq!(run("0n").0, "0");
        assert_eq!(run("5n3").0, "-53");
    }

    #[test]
    fn percent_like_windows() {
        assert_eq!(run("50+10%"), ("5".into(), "50 + 5".into()));
        assert_eq!(run("50+10%=").0, "55");
        assert_eq!(run("50*10%=").0, "5");
        assert_eq!(run("50/10%=").0, "500");
        assert_eq!(run("10%"), ("0".into(), "0".into()));
    }

    #[test]
    fn errors() {
        assert_eq!(run("1/0="), ("Cannot divide by zero".into(), "1 ÷".into()));
        assert_eq!(
            run("5+3/0+"),
            ("Cannot divide by zero".into(), "8 ÷ 0 +".into())
        );
        assert_eq!(run("0/0=").0, "Result is undefined");
        assert_eq!(run("0i"), ("Cannot divide by zero".into(), "1/(0)".into()));
        assert_eq!(run("9nr"), ("Invalid input".into(), "√(-9)".into()));
        assert_eq!(run("9999999999999999ssssssssssss").0, "Overflow");
    }

    #[test]
    fn error_blocks_operators_until_digit() {
        let mut calc = Calculator::default();
        press(&mut calc, "1/0=");
        assert!(calc.has_error());
        assert!(!calc.is_enabled(Command::Operator(Op::Add)));
        press(&mut calc, "+");
        assert!(calc.has_error());
        press(&mut calc, "7");
        assert!(!calc.has_error());
        assert_eq!((calc.display(), calc.expression()), ("7".into(), "".into()));
        // ⌫ and CE only clear the error.
        press(&mut calc, "1/0=<");
        assert_eq!((calc.display(), calc.has_error()), ("0".into(), false));
    }

    #[test]
    fn digit_limit() {
        assert_eq!(run("12345678901234567890").0, "1,234,567,890,123,456");
        assert_eq!(run("0.12345678901234567").0, "0.1234567890123456");
    }

    #[test]
    fn decimal_point() {
        assert_eq!(run(".5").0, "0.5");
        assert_eq!(run("5..5").0, "5.5");
        assert_eq!(run("5+.").0, "0.");
    }

    #[test]
    fn backspace_and_clear() {
        assert_eq!(run("123<").0, "12");
        assert_eq!(run("1<<").0, "0");
        assert_eq!(run("5n<").0, "0");
        assert_eq!(run("0.<").0, "0");
        assert_eq!(run("2+3=<"), ("5".into(), "".into()));
        // CE after `=` clears the display and the expression line, but like in Windows
        // (`ClearTemporaryValues`) the next `=` still repeats the last operation.
        assert_eq!(run("2+3=e"), ("0".into(), "".into()));
        assert_eq!(run("2+3=e="), ("3".into(), "0 + 3 =".into()));
        assert_eq!(run("5+3e"), ("0".into(), "5 +".into()));
        assert_eq!(run("5+3e2="), ("7".into(), "5 + 2 =".into()));
        assert_eq!(run("5+3c"), ("0".into(), "".into()));
        // CE replaces a function result: `5 + sqr(3)` goes back to `5 +`.
        assert_eq!(run("5+3se"), ("0".into(), "5 +".into()));
    }

    #[test]
    fn new_number_after_result() {
        assert_eq!(run("2+3=4"), ("4".into(), "".into()));
        assert_eq!(run("2+3=*2="), ("10".into(), "5 × 2 =".into()));
    }

    #[test]
    fn memory() {
        let mut calc = Calculator::default();
        assert!(!calc.is_enabled(Command::MemoryRecall));
        press(&mut calc, "5");
        calc.execute(Command::MemoryStore);
        press(&mut calc, "3");
        assert_eq!(calc.display(), "3");
        calc.execute(Command::MemoryAdd);
        calc.execute(Command::MemoryAdd);
        press(&mut calc, "c");
        calc.execute(Command::MemoryRecall);
        assert_eq!(calc.display(), "11");
        calc.execute(Command::MemoryStore);
        assert_eq!(calc.memory().len(), 2);
        calc.memory_clear_item(0);
        assert_eq!(format_number(&calc.memory()[0]), "11");
        calc.execute(Command::MemoryClear);
        assert!(calc.memory().is_empty());
        // M− on an empty memory stores the negative.
        press(&mut calc, "4");
        calc.execute(Command::MemorySubtract);
        assert_eq!(format_number(&calc.memory()[0]), "-4");
    }

    #[test]
    fn history_logs_every_step() {
        let mut calc = Calculator::default();
        press(&mut calc, "1+2+3=");
        assert_eq!(
            history(&calc),
            [
                ("3 + 3 =".to_string(), "6".to_string()),
                ("1 + 2 =".to_string(), "3".to_string())
            ]
        );
        press(&mut calc, "c");
        calc.recall_history(1);
        assert_eq!(
            (calc.display(), calc.expression()),
            ("3".into(), "1 + 2 =".into())
        );
        press(&mut calc, "+1=");
        assert_eq!(calc.display(), "4");
        calc.delete_history_item(0);
        assert_eq!(history(&calc)[0].0, "3 + 3 =");
    }

    #[test]
    fn history_logs_lone_numbers_and_finished_functions() {
        // Windows logs `5 =` too, and a finished `√(4)` when a new number starts.
        let mut calc = Calculator::default();
        press(&mut calc, "5=");
        assert_eq!(history(&calc), [("5 =".to_string(), "5".to_string())]);
        press(&mut calc, "c4r5");
        assert_eq!(history(&calc)[0], ("√(4)".to_string(), "2".to_string()));
    }

    #[test]
    fn standard_mode_has_no_parentheses() {
        let calc = Calculator::default();
        assert!(!calc.is_enabled(Command::OpenParen));
        // The parentheses are ignored: 2 × 3 + 4 =.
        assert_eq!(run("2*(3+4)="), ("10".into(), "6 + 4 =".into()));
    }

    #[test]
    fn precedence_evaluates_higher_operators_first() {
        assert_eq!(
            run_precedence("2+3*4="),
            ("14".into(), "2 + 3 × 4 =".into())
        );
        assert_eq!(
            run_precedence("1+2*3*4="),
            ("25".into(), "1 + 2 × 3 × 4 =".into())
        );
        assert_eq!(run_precedence("8-6/2="), ("5".into(), "8 - 6 ÷ 2 =".into()));
    }

    #[test]
    fn precedence_shows_partial_results() {
        // `+` after `2 + 3 ×` waits; a lower or equal operator computes what it can.
        assert_eq!(run_precedence("2+3*"), ("3".into(), "2 + 3 ×".into()));
        assert_eq!(run_precedence("2*3+"), ("6".into(), "2 × 3 +".into()));
        assert_eq!(
            run_precedence("2+3*4+"),
            ("14".into(), "2 + 3 × 4 +".into())
        );
    }

    #[test]
    fn precedence_inversion_adds_brackets() {
        // 1 + 2 was computed when `-` was pressed; changing `-` to `×` keeps that order.
        assert_eq!(
            run_precedence("1+2-*3="),
            ("9".into(), "(1 + 2) × 3 =".into())
        );
    }

    #[test]
    fn precedence_history_and_repeat() {
        let mut calc = Calculator::with_precedence();
        press(&mut calc, "2+3*4=");
        assert_eq!(
            history(&calc),
            [("2 + 3 × 4 =".to_string(), "14".to_string())]
        );
        // Repeated `=` repeats the last operation done (`+ 12`), as in Windows.
        press(&mut calc, "=");
        assert_eq!(
            (calc.display(), calc.expression()),
            ("26".into(), "14 + 12 =".into())
        );
    }

    #[test]
    fn parentheses() {
        assert_eq!(
            run_precedence("(1+2)*3="),
            ("9".into(), "(1 + 2) × 3 =".into())
        );
        assert_eq!(
            run_precedence("2*(3+4)="),
            ("14".into(), "2 × (3 + 4) =".into())
        );
        assert_eq!(
            run_precedence("((2+3))*2="),
            ("10".into(), "((2 + 3)) × 2 =".into())
        );
        assert_eq!(
            run_precedence("1+(2*3)="),
            ("7".into(), "1 + (2 × 3) =".into())
        );
        assert_eq!(run_precedence("(1+2)s"), ("9".into(), "sqr(1 + 2)".into()));
    }

    #[test]
    fn parentheses_count_and_auto_close() {
        let mut calc = Calculator::with_precedence();
        press(&mut calc, "2*((3+4");
        assert_eq!(calc.open_parentheses(), 2);
        press(&mut calc, "=");
        assert_eq!(calc.open_parentheses(), 0);
        assert_eq!(
            (calc.display(), calc.expression()),
            ("14".into(), "2 × ((3 + 4)) =".into())
        );
    }

    #[test]
    fn parentheses_edge_cases() {
        // `)` without `(` is ignored.
        assert_eq!(run_precedence("2)"), ("2".into(), "".into()));
        // An empty group is 0.
        assert_eq!(run_precedence("1+()="), ("1".into(), "1 + (0) =".into()));
        // A number next to a parenthesis multiplies.
        assert_eq!(run_precedence("2(3)="), ("6".into(), "2 × (3) =".into()));
        assert_eq!(run_precedence("(2)3="), ("6".into(), "(2) × 3 =".into()));
    }

    #[test]
    fn precedence_mode_keeps_percent_and_functions() {
        assert_eq!(run_precedence("50+10%="), ("55".into(), "50 + 5 =".into()));
        assert_eq!(
            run_precedence("2+3s*2="),
            ("20".into(), "2 + sqr(3) × 2 =".into())
        );
    }

    #[test]
    fn paste_numbers() {
        let mut calc = Calculator::default();
        assert!(calc.paste(" 1,234.5 "));
        assert_eq!(calc.display(), "1,234.5");
        assert!(calc.paste("-007"));
        assert_eq!(calc.display(), "-7");
        assert_eq!(calc.copy_text().as_deref(), Some("-7"));
        assert!(calc.paste("$12"));
        assert_eq!(calc.display(), "12");
        assert!(calc.paste("-0.5"));
        assert_eq!(calc.display(), "-0.5");
    }

    #[test]
    fn paste_expressions() {
        let mut calc = Calculator::default();
        assert!(calc.paste("12 + 5 ="));
        assert_eq!(
            (calc.display(), calc.expression()),
            ("17".into(), "12 + 5 =".into())
        );
        assert!(calc.paste("2*-3="));
        assert_eq!(
            (calc.display(), calc.expression()),
            ("-6".into(), "2 × -3 =".into())
        );
        // Without `=` the expression waits, like typing it.
        assert!(calc.paste("8/2"));
        assert_eq!(
            (calc.display(), calc.expression()),
            ("2".into(), "8 ÷".into())
        );

        let mut scientific = Calculator::with_precedence();
        assert!(scientific.paste("(1+2)*3="));
        assert_eq!(scientific.display(), "9");
        assert!(scientific.paste("-(2+3)="));
        assert_eq!(scientific.display(), "-5");
    }

    #[test]
    fn invalid_paste_shows_invalid_input() {
        for text in ["abc", "12345678901234567", "5+", "(1+2)", "1e5", ""] {
            let mut calc = Calculator::default();
            assert!(!calc.paste(text), "{text:?} should be rejected");
            assert_eq!(calc.display(), "Invalid input", "{text:?}");
        }
        assert!(!paste_is_valid(&"1".repeat(MAX_PASTE_LENGTH + 1), false));
        // Digits are typed again after the error.
        let mut calc = Calculator::default();
        calc.paste("abc");
        press(&mut calc, "4");
        assert_eq!(calc.display(), "4");
    }

    #[test]
    fn paste_operand_rules() {
        assert_eq!(extract_operands("12+-5", false), ["12", "-5"]);
        assert_eq!(extract_operands("-3*4", false), ["-3", "4"]);
        assert!(operand_matches(" -1.5 ", false));
        assert!(operand_matches(".5", false));
        assert!(!operand_matches("1.2.3", false));
        assert!(operand_matches("-(2", true));
        assert!(operand_matches("3))", true));
        assert_eq!(operand_length("0.123"), 3);
        assert_eq!(operand_length("-12.5"), 3);
    }
}
