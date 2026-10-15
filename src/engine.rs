//! Calculator engine: the logic of the Windows calculator's Standard mode,
//! completely independent of the user interface.
//!
//! Like Windows, operations are applied immediately, without precedence:
//! `2 + 3 × 4 =` gives 20. Numbers are stored as exact fractions (`BigRational`),
//! so `0.1 + 0.2 − 0.3` is exactly 0 and `1 ÷ 3 × 3` is exactly 1.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl Op {
    fn symbol(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Subtract => "−",
            Op::Multiply => "×",
            Op::Divide => "÷",
        }
    }
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
    /// How the function looks in the expression line above the display.
    fn wrap(self, operand: &str) -> String {
        match self {
            Func::Reciprocal => format!("1/({operand})"),
            Func::Square => format!("sqr({operand})"),
            Func::SquareRoot => format!("√({operand})"),
            Func::Negate => format!("negate({operand})"),
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

/// Input state; it decides how the next key press is handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// The user is typing a number into `input`.
    Typing,
    /// The display shows a finished number (function result, memory recall, ...);
    /// typing a digit replaces it.
    Operand,
    /// An operator was just pressed; the display still shows the left operand.
    AwaitingOperand,
    /// `=` was just pressed; the display shows the result.
    Result,
}

/// Left operand and operator waiting for the right operand.
#[derive(Clone, Debug)]
struct Pending {
    left: Number,
    expression: String,
    op: Op,
}

#[derive(Clone, Debug)]
pub struct Calculator {
    value: Number,
    input: String,
    /// Expression for the current operand when a function produced it: `sqr(5)`.
    operand_expression: Option<String>,
    pending: Option<Pending>,
    /// Last operation, for repeated `=` (`8 + 3 =`, `11 + 3 =`, ...).
    repeat: Option<(Op, Number)>,
    result_expression: String,
    phase: Phase,
    error: Option<(CalcError, String)>,
    memory: Vec<Number>,
    history: Vec<HistoryEntry>,
}

impl Default for Calculator {
    fn default() -> Self {
        Self {
            value: Number::zero(),
            input: "0".to_string(),
            operand_expression: None,
            pending: None,
            repeat: None,
            result_expression: String::new(),
            phase: Phase::Operand,
            error: None,
            memory: Vec::new(),
            history: Vec::new(),
        }
    }
}

impl Calculator {
    // --------------------------------------------------------------- queries

    /// The big number on the display (or the error message).
    pub fn display(&self) -> String {
        if let Some((error, _)) = &self.error {
            error.message().to_string()
        } else if self.phase == Phase::Typing {
            format_input(&self.input)
        } else {
            format_number(&self.value)
        }
    }

    /// The expression line above the display, e.g. `5 + sqr(3)`.
    pub fn expression(&self) -> String {
        if let Some((_, expression)) = &self.error {
            expression.clone()
        } else if self.phase == Phase::Result {
            self.result_expression.clone()
        } else {
            self.with_pending(self.operand_expression.as_deref().unwrap_or(""))
        }
    }

    pub fn has_error(&self) -> bool {
        self.error.is_some()
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
        match command {
            Command::Digit(d) => self.push_digit(d),
            Command::Decimal => self.push_decimal(),
            Command::Operator(op) => self.operator(op),
            Command::Function(func) => self.function(func),
            Command::Percent => self.percent(),
            Command::Equals => self.equals(),
            Command::ClearEntry => self.clear_entry(),
            Command::Clear => self.clear(),
            Command::Backspace => self.backspace(),
            Command::MemoryClear => self.memory.clear(),
            Command::MemoryRecall => self.recall(self.memory[0].clone()),
            Command::MemoryAdd => self.memory_add(0, false),
            Command::MemorySubtract => self.memory_add(0, true),
            Command::MemoryStore => {
                self.memory.insert(0, self.value.clone());
                self.memory.truncate(MEMORY_LIMIT);
                self.finish_typing();
            }
        }
    }

    fn push_digit(&mut self, digit: u8) {
        self.begin_input();
        if count_digits(&self.input) >= MAX_DIGITS {
            return;
        }
        if self.input == "0" {
            self.input.clear();
        }
        self.input.push(char::from(b'0' + digit));
        self.value = parse_decimal(&self.input);
    }

    fn push_decimal(&mut self) {
        self.begin_input();
        if !self.input.contains('.') {
            self.input.push('.');
        }
    }

    /// Starts typing a new number unless one is already being typed.
    fn begin_input(&mut self) {
        if self.error.is_some() {
            self.clear();
        }
        if self.phase != Phase::Typing {
            self.input = "0".to_string();
            self.value = Number::zero();
            self.operand_expression = None;
            self.phase = Phase::Typing;
        }
    }

    /// After memory operations the next digit starts a new number.
    fn finish_typing(&mut self) {
        if self.phase == Phase::Typing {
            self.phase = Phase::Operand;
        }
    }

    fn operator(&mut self, op: Op) {
        // Pressing another operator just replaces the previous one.
        if self.phase == Phase::AwaitingOperand {
            if let Some(pending) = &mut self.pending {
                pending.op = op;
            }
            return;
        }
        let mut expression = self.operand_text();
        if let Some(pending) = self.pending.take() {
            let full = format!(
                "{} {} {}",
                pending.expression,
                pending.op.symbol(),
                expression
            );
            match apply(&pending.left, pending.op, &self.value) {
                Ok(result) => {
                    self.push_history(format!("{full} ="), &result);
                    expression = expression_number(&result);
                    self.value = result;
                }
                Err(error) => return self.fail(error, format!("{full} {}", op.symbol())),
            }
        }
        self.pending = Some(Pending {
            left: self.value.clone(),
            expression,
            op,
        });
        self.operand_expression = None;
        self.phase = Phase::AwaitingOperand;
    }

    fn equals(&mut self) {
        let operand = self.operand_text();
        let (left, left_text, op, right, right_text) =
            match (self.pending.take(), self.repeat.clone()) {
                // `5 + =` uses the displayed number as the right operand: 5 + 5.
                (Some(pending), _) => (
                    pending.left,
                    pending.expression,
                    pending.op,
                    self.value.clone(),
                    operand,
                ),
                // Repeated `=` repeats the last operation with the new left operand.
                (None, Some((op, right))) => {
                    let right_text = expression_number(&right);
                    (self.value.clone(), operand, op, right, right_text)
                }
                (None, None) => return self.show_result(format!("{operand} =")),
            };
        let expression = format!("{left_text} {} {right_text} =", op.symbol());
        match apply(&left, op, &right) {
            Ok(result) => {
                self.push_history(expression.clone(), &result);
                self.value = result;
                self.repeat = Some((op, right));
                self.show_result(expression);
            }
            // In this case Windows keeps only `1 ÷` in the expression line.
            Err(error) => self.fail(error, format!("{left_text} {}", op.symbol())),
        }
    }

    fn show_result(&mut self, expression: String) {
        self.result_expression = expression;
        self.operand_expression = None;
        self.phase = Phase::Result;
    }

    fn function(&mut self, func: Func) {
        // While typing, ± just flips the sign of the number being typed.
        if func == Func::Negate && self.phase == Phase::Typing {
            if !self.value.is_zero() {
                match self.input.strip_prefix('-') {
                    Some(rest) => self.input = rest.to_string(),
                    None => self.input.insert(0, '-'),
                }
                self.value = -&self.value;
            }
            return;
        }
        let expression = func.wrap(&self.operand_text());
        match func.apply(&self.value) {
            Ok(result) => self.set_operand(result, Some(expression)),
            Err(error) => {
                let shown = self.with_pending(&expression);
                self.fail(error, shown);
            }
        }
    }

    /// `%` as in Windows: for + and − it is a percentage of the left operand,
    /// for × and ÷ it simply divides by 100, and without an operator it gives 0.
    fn percent(&mut self) {
        let hundred = Number::from_integer(100.into());
        let result = match &self.pending {
            Some(p) if matches!(p.op, Op::Add | Op::Subtract) => &p.left * &self.value / hundred,
            Some(_) => &self.value / hundred,
            None => Number::zero(),
        };
        match normalize(result) {
            Ok(result) => {
                let text = expression_number(&result);
                self.set_operand(result, Some(text));
            }
            Err(error) => self.fail(error, self.expression()),
        }
    }

    fn set_operand(&mut self, value: Number, expression: Option<String>) {
        self.value = value;
        self.operand_expression = expression;
        self.phase = Phase::Operand;
    }

    fn clear_entry(&mut self) {
        if self.error.is_some() || self.phase == Phase::Result {
            self.clear();
        } else {
            self.set_operand(Number::zero(), None);
        }
    }

    /// Resets the calculation; memory and history are kept.
    fn clear(&mut self) {
        let memory = std::mem::take(&mut self.memory);
        let history = std::mem::take(&mut self.history);
        *self = Self {
            memory,
            history,
            ..Self::default()
        };
    }

    fn backspace(&mut self) {
        if self.error.is_some() {
            return self.clear();
        }
        match self.phase {
            Phase::Typing => {
                self.input.pop();
                if matches!(self.input.as_str(), "" | "-" | "-0") {
                    self.input = "0".to_string();
                }
                self.value = parse_decimal(&self.input);
            }
            // After `=` only the expression line is erased.
            Phase::Result => self.result_expression.clear(),
            _ => {}
        }
    }

    fn fail(&mut self, error: CalcError, expression: String) {
        let memory = std::mem::take(&mut self.memory);
        let history = std::mem::take(&mut self.history);
        *self = Self {
            memory,
            history,
            error: Some((error, expression)),
            ..Self::default()
        };
    }

    /// Puts a ready number on the display (from memory or history).
    fn recall(&mut self, value: Number) {
        if self.phase == Phase::Result {
            self.result_expression.clear();
        }
        self.set_operand(value, None);
    }

    // ---------------------------------------------------- memory and history

    pub fn memory_recall_item(&mut self, index: usize) {
        if self.error.is_none() && index < self.memory.len() {
            self.recall(self.memory[index].clone());
        }
    }

    /// M+ (or M− when `subtract`) for memory slot `index`.
    pub fn memory_add(&mut self, index: usize, subtract: bool) {
        if self.error.is_some() {
            return;
        }
        let delta = if subtract {
            -&self.value
        } else {
            self.value.clone()
        };
        match self.memory.get_mut(index) {
            Some(cell) => {
                if let Ok(sum) = normalize(&*cell + delta) {
                    *cell = sum;
                }
            }
            None => self.memory.insert(0, delta),
        }
        self.finish_typing();
    }

    pub fn memory_clear_item(&mut self, index: usize) {
        if index < self.memory.len() {
            self.memory.remove(index);
        }
    }

    pub fn recall_history(&mut self, index: usize) {
        let Some(entry) = self.history.get(index).cloned() else {
            return;
        };
        self.clear();
        self.value = entry.result;
        self.show_result(entry.expression);
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    fn push_history(&mut self, expression: String, result: &Number) {
        self.history.insert(
            0,
            HistoryEntry {
                expression,
                result: result.clone(),
            },
        );
        self.history.truncate(HISTORY_LIMIT);
    }

    /// Pastes a number from the clipboard. Returns `false` if the text is not a number.
    pub fn paste(&mut self, text: &str) -> bool {
        let cleaned: String = text
            .trim()
            .chars()
            .filter(|c| !matches!(c, ',' | ' ' | '\u{a0}' | '\'' | '’' | '_'))
            .collect();
        let (negative, unsigned) = match cleaned.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, cleaned.as_str()),
        };
        let (int, frac) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        let valid = !(int.is_empty() && frac.is_empty())
            && int.chars().chain(frac.chars()).all(|c| c.is_ascii_digit());
        if !valid {
            return false;
        }
        let int = match int.trim_start_matches('0') {
            "" => "0",
            int => int,
        };
        let mut input = int.to_string();
        if unsigned.contains('.') {
            input.push('.');
            input.push_str(frac);
        }
        if count_digits(&input) > MAX_DIGITS {
            return false;
        }
        if negative && !parse_decimal(&input).is_zero() {
            input.insert(0, '-');
        }
        self.begin_input();
        self.value = parse_decimal(&input);
        self.input = input;
        true
    }

    // --------------------------------------------------------------- helpers

    /// The current operand as it appears in the expression.
    fn operand_text(&self) -> String {
        self.operand_expression
            .clone()
            .unwrap_or_else(|| expression_number(&self.value))
    }

    fn with_pending(&self, operand: &str) -> String {
        match &self.pending {
            Some(p) => format!("{} {} {operand}", p.expression, p.op.symbol())
                .trim_end()
                .to_string(),
            None => operand.to_string(),
        }
    }
}

// ------------------------------------------------------------- arithmetic

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

/// The number being typed, with thousands separators: `-1234.50` → `-1,234.50`.
fn format_input(input: &str) -> String {
    let (sign, unsigned) = match input.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", input),
    };
    match unsigned.split_once('.') {
        Some((int, frac)) => format!("{sign}{}.{frac}", group_thousands(int)),
        None => format!("{sign}{}", group_thousands(unsigned)),
    }
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

/// Digits that count towards the input limit (the leading `0` of `0.5` doesn't).
fn count_digits(input: &str) -> i64 {
    let unsigned = input.trim_start_matches('-');
    let significant = unsigned.strip_prefix('0').unwrap_or(unsigned);
    significant.chars().filter(char::is_ascii_digit).count() as i64
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

    /// Presses keys given as a string: digits, `.`, `+-*/`, `=`, `%`,
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
        assert_eq!(run("9nr").0, "Invalid input");
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
        assert_eq!(run("2+3=<"), ("5".into(), "".into()));
        assert_eq!(run("5+3e"), ("0".into(), "5 +".into()));
        assert_eq!(run("5+3e2="), ("7".into(), "5 + 2 =".into()));
        assert_eq!(run("5+3c"), ("0".into(), "".into()));
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
    }

    #[test]
    fn history() {
        let mut calc = Calculator::default();
        press(&mut calc, "1+2+3=");
        let entries: Vec<_> = calc
            .history()
            .iter()
            .map(|e| (e.expression.clone(), format_number(&e.result)))
            .collect();
        assert_eq!(
            entries,
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
    }

    #[test]
    fn paste() {
        let mut calc = Calculator::default();
        assert!(calc.paste(" 1,234.5 "));
        assert_eq!(calc.display(), "1,234.5");
        assert!(calc.paste("-007"));
        assert_eq!(calc.display(), "-7");
        assert!(!calc.paste("abc"));
        assert!(!calc.paste("12345678901234567"));
        assert_eq!(calc.copy_text().as_deref(), Some("-7"));
    }
}
