//! Calculator expression evaluator for calc mode
//!
//! This module provides line-by-line math expression evaluation,
//! similar to the webapp's calcExtension.ts but using evalexpr.

use evalexpr::{eval_with_context_mut, HashMapContext, IterateVariablesContext, Value, ContextWithMutableVariables, Context};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use rust_i18n::t;
use std::f64::consts::{E, PI, TAU};
use std::sync::mpsc;
use std::time::Duration;

use crate::ui::helpers::truncate_to_width;
use crate::ui::regex;

/// Maximum number of lines to evaluate in calc mode
/// Documents longer than this will show a warning
const MAX_CALC_LINES: usize = 500;

/// Maximum consecutive errors before stopping evaluation
/// Prevents runaway processing on non-calc content
const MAX_CONSECUTIVE_ERRORS: usize = 10;

/// Maximum total errors before stopping evaluation
const MAX_TOTAL_ERRORS: usize = 50;

/// Longest line handed to the evaluator, in bytes.
///
/// Calc notes arrive through sync, so their text is hostile until shown otherwise,
/// and they're evaluated while drawing. evalexpr recurses over an expression and an
/// abort from overflowing the stack can't be caught, so the terminal would be left
/// in raw mode. This limit and the next keep every expression far from that.
const MAX_CALC_LINE_BYTES: usize = 1024;

/// Deepest bracket nesting handed to the evaluator.
const MAX_CALC_NESTING: usize = 64;

/// Stack for the evaluation thread. Generous, as the limits above already bound depth.
const CALC_STACK_BYTES: usize = 8 * 1024 * 1024;

/// How long drawing waits for a document before showing it without results.
const CALC_DEADLINE: Duration = Duration::from_secs(2);

/// Result of evaluating a single line
#[derive(Debug, Clone)]
pub struct LineResult {
    /// The original line text
    pub line: String,
    /// The computed result (if any)
    pub result: Option<String>,
    /// Whether this line had an error
    pub is_error: bool,
    /// Whether this line is a comment
    pub is_comment: bool,
}

/// Calculator evaluator with persistent variable scope
pub struct CalcEvaluator {
    context: HashMapContext,
}

impl CalcEvaluator {
    /// Create a new evaluator with built-in constants
    pub fn new() -> Self {
        let mut context = HashMapContext::new();

        // Add built-in constants
        let _ = context.set_value("pi".into(), Value::from_float(PI));
        let _ = context.set_value("e".into(), Value::from_float(E));
        let _ = context.set_value("tau".into(), Value::from_float(TAU));
        let _ = context.set_value("phi".into(), Value::from_float(1.618033988749895)); // Golden ratio

        // Note: evalexpr has built-in: min, max, len, str, floor, round, ceil, abs,
        // sqrt, exp, ln, log, sin, cos, tan, asin, acos, atan

        Self { context }
    }

    /// Reset the evaluator context (clear all variables)
    pub fn reset(&mut self) {
        self.context = HashMapContext::new();

        // Re-add built-in constants
        let _ = self.context.set_value("pi".into(), Value::from_float(PI));
        let _ = self.context.set_value("e".into(), Value::from_float(E));
        let _ = self.context.set_value("tau".into(), Value::from_float(TAU));
        let _ = self.context.set_value("phi".into(), Value::from_float(1.618033988749895));
    }

    /// Check if a line is a comment
    fn is_comment(line: &str) -> bool {
        line.trim_start().starts_with('#')
    }

    /// Check if a line is an assignment and extract the variable name
    fn parse_assignment(line: &str) -> Option<String> {
        // Match pattern: identifier = expression (but not == comparison)
        regex!(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*[^=]")
            .captures(line)
            .map(|c| c[1].to_string())
    }

    /// Why a line mustn't reach the evaluator, if there's a reason.
    ///
    /// Calc needs numbers. Strings, tuples and statement chains are how a short
    /// line doubles a value until memory runs out, as in `a=a+a` from a long string
    /// or `(a=(a,a)),(a=(a,a)),...` in one line. Without quotes, `;` or a second
    /// assignment, a line's value can't outgrow the line. Whatever a builtin such
    /// as `str()` still produces is dropped after the line by `drop_non_scalars`.
    fn refusal(line: &str) -> Option<String> {
        if line.len() > MAX_CALC_LINE_BYTES {
            return Some(t!("calc.line_too_long", max = MAX_CALC_LINE_BYTES).to_string());
        }
        if line.contains('"') || line.contains(';') || Self::count_assignments(line) > 1 {
            return Some(t!("calc.numbers_only").to_string());
        }
        None
    }

    /// Bracket depth of an expression.
    fn nesting(expression: &str) -> usize {
        let (mut depth, mut deepest) = (0usize, 0usize);
        for c in expression.chars() {
            match c {
                '(' => {
                    depth += 1;
                    deepest = deepest.max(depth);
                }
                ')' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        deepest
    }

    /// How many assignments a line makes, counting `+=` and friends but not `==`.
    fn count_assignments(line: &str) -> usize {
        let bytes = line.as_bytes();
        bytes.iter().enumerate()
            .filter(|&(i, &byte)| {
                let comparison = bytes.get(i + 1) == Some(&b'=')
                    || (i > 0 && matches!(bytes[i - 1], b'=' | b'!' | b'<' | b'>'));
                byte == b'=' && !comparison
            })
            .count()
    }

    /// Remove any variable that isn't a number or boolean, returning whether there
    /// was one. A variable that can't hold a string or tuple can't grow between lines.
    fn drop_non_scalars(&mut self) -> bool {
        let offenders: Vec<String> = self.context.iter_variables()
            .filter(|(_, value)| !matches!(value, Value::Float(_) | Value::Int(_) | Value::Boolean(_) | Value::Empty))
            .map(|(name, _)| name)
            .collect();

        for name in &offenders {
            let _ = self.context.remove_value(name);
        }
        !offenders.is_empty()
    }

    /// Preprocess expression to handle syntax differences from mathjs
    fn preprocess(expr: &str) -> String {
        let mut result = expr.to_string();

        // Replace ** with ^ for power (Python-style to evalexpr-style)
        result = result.replace("**", "^");

        // Replace // with / for integer division (we'll just use regular division)
        // evalexpr doesn't have integer division, so this is approximate
        result = result.replace("//", "/");

        // Fix log() function: mathjs's log() is natural log (like ln())
        // evalexpr's math::log(x, base) requires 2 args, so map log() -> ln()
        result = regex!(r"\blog\s*\(").replace_all(&result, "ln(").to_string();

        // Fix power associativity: mathjs uses right-to-left, evalexpr uses left-to-right
        // Convert a^b^c to a^(b^c)
        result = Self::fix_power_associativity(&result);

        // Convert integers involved in division to floats to avoid integer division
        // Match integer followed by / or / followed by integer
        result = Self::ensure_float_division(&result);

        // Map common math functions to evalexpr's math:: namespace. These need the
        // math:: prefix, and one already written with it mustn't gain a second.
        // Note: "log" is NOT in this list - we handle it above by converting to "ln"
        result = regex!(
            r"\b(sqrt|cbrt|exp|exp2|ln|log2|log10|sin|cos|tan|asin|acos|atan|atan2|sinh|cosh|tanh|asinh|acosh|atanh|abs|hypot)\s*\("
        )
        .replace_all(&result, "math::$1(")
        .replace("math::math::", "math::");

        result
    }

    /// Fix power operator associativity from left-to-right to right-to-left
    /// Converts a^b^c to a^(b^c)
    fn fix_power_associativity(expr: &str) -> String {
        // Find sequences of a^b^c and parenthesize from right
        // Simple approach: repeatedly find rightmost ^ pair and parenthesize
        let mut result = expr.to_string();

        // Pattern to find x^y^z (simplified - handles basic cases)
        // We need to handle: 2^3^2 -> 2^(3^2)
        let re = regex!(r"(\^)([^+\-*/^()]+)(\^)");
        let mut opened = 0;

        // Keep replacing until no more matches
        loop {
            let new_result = re.replace(&result, "$1($2$3").to_string();
            if new_result == result {
                break;
            }
            result = new_result;
            opened += 1;
        }

        // Close only what was opened here. The text's own brackets aren't ours to
        // balance: a line such as "1) milk" has more ')' than '('.
        for _ in 0..opened {
            result.push(')');
        }

        result
    }

    /// Ensure division uses floats to avoid integer division
    fn ensure_float_division(expr: &str) -> String {
        let mut result = expr.to_string();

        // Convert integer before / to float: "4/" -> "4.0/"
        result = regex!(r"(\d+)(\s*/)").replace_all(&result, |caps: &regex::Captures| {
            let num = &caps[1];
            let slash = &caps[2];
            // Only add .0 if not already a float
            if !num.contains('.') {
                format!("{}.0{}", num, slash)
            } else {
                format!("{}{}", num, slash)
            }
        }).to_string();

        // Convert integer after / to float: "/4" -> "/4.0"
        result = regex!(r"(/\s*)(\d+)").replace_all(&result, |caps: &regex::Captures| {
            let slash = &caps[1];
            let num = &caps[2];
            // Only add .0 if not already a float (check next char isn't .)
            format!("{}{}.0", slash, num)
        }).to_string();

        // Fix double .0.0 that might occur
        result = result.replace(".0.0", ".0");

        result
    }

    /// Calculate factorial for small numbers
    fn factorial(n: i64) -> Option<i64> {
        if n < 0 {
            return None;
        }
        if n > 20 {
            return None; // Would overflow i64
        }
        let mut result: i64 = 1;
        for i in 2..=n {
            result = result.checked_mul(i)?;
        }
        Some(result)
    }

    /// Handle factorial expressions (e.g., "5!")
    fn handle_factorial(expr: &str) -> Option<String> {
        let caps = regex!(r"^(\d+)!$").captures(expr.trim())?;
        let n: i64 = caps[1].parse().ok()?;
        Self::factorial(n).map(|r| r.to_string())
    }

    /// Evaluate a single line
    fn evaluate_line(&mut self, line: &str) -> LineResult {
        let trimmed = line.trim();

        // Empty line
        if trimmed.is_empty() {
            return LineResult {
                line: line.to_string(),
                result: None,
                is_error: false,
                is_comment: false,
            };
        }

        // Comment
        if Self::is_comment(trimmed) {
            return LineResult {
                line: line.to_string(),
                result: None,
                is_error: false,
                is_comment: true,
            };
        }

        // Anything that could exhaust the stack or memory stops here
        if let Some(reason) = Self::refusal(trimmed) {
            return Self::refused(line, reason);
        }

        // Handle factorial expressions specially (e.g., "5!")
        if let Some(factorial_result) = Self::handle_factorial(trimmed) {
            return LineResult {
                line: line.to_string(),
                result: Some(factorial_result),
                is_error: false,
                is_comment: false,
            };
        }

        // Preprocess the expression to handle syntax differences
        let processed = Self::preprocess(trimmed);

        // Measured after preprocessing, which adds brackets of its own
        if Self::nesting(&processed) > MAX_CALC_NESTING {
            return Self::refused(line, t!("calc.too_deep", max = MAX_CALC_NESTING).to_string());
        }

        // Check if this is an assignment
        let assignment_var = Self::parse_assignment(trimmed);

        // Try to evaluate the expression
        let outcome = eval_with_context_mut(&processed, &mut self.context);

        if self.drop_non_scalars() {
            return Self::refused(line, t!("calc.numbers_only").to_string());
        }

        match outcome {
            Ok(value) => {
                // For assignments, evalexpr returns Empty - get the actual value from context
                let display_value = if let Some(var_name) = assignment_var {
                    if value == Value::Empty {
                        // Look up the assigned value from context
                        self.context
                            .get_value(&var_name)
                            .cloned()
                            .unwrap_or(Value::Empty)
                    } else {
                        value
                    }
                } else {
                    value
                };

                let result = Self::format_value(&display_value);
                LineResult {
                    line: line.to_string(),
                    result: Some(result),
                    is_error: false,
                    is_comment: false,
                }
            }
            Err(e) => {
                // Check if this is an undefined variable error - suppress these
                let error_str = e.to_string();
                if error_str.contains("Variable identifier is not bound") ||
                   error_str.contains("identifier") && error_str.contains("not bound") {
                    // Suppress undefined variable errors - just don't show a result
                    LineResult {
                        line: line.to_string(),
                        result: None,
                        is_error: false,
                        is_comment: false,
                    }
                } else {
                    // Format other error messages
                    let error_msg = format!("Error: {}", Self::format_error(&e));
                    LineResult {
                        line: line.to_string(),
                        result: Some(error_msg),
                        is_error: true,
                        is_comment: false,
                    }
                }
            }
        }
    }

    /// The result for a line that wasn't evaluated. It counts as an error, so a
    /// document full of them stops early like any other that isn't calc.
    fn refused(line: &str, reason: String) -> LineResult {
        LineResult {
            line: line.to_string(),
            result: Some(reason),
            is_error: true,
            is_comment: false,
        }
    }

    /// Format a value for display
    fn format_value(value: &Value) -> String {
        match value {
            Value::Float(f) => {
                // Handle special float values
                if f.is_nan() {
                    return "NaN".to_string();
                }
                if f.is_infinite() {
                    return if *f > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() };
                }
                // Format with reasonable precision
                if f.fract() == 0.0 && f.abs() < 1e15 {
                    format!("{}", *f as i64)
                } else {
                    // Remove trailing zeros
                    let formatted = format!("{:.10}", f);
                    formatted.trim_end_matches('0').trim_end_matches('.').to_string()
                }
            }
            Value::Int(i) => i.to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::String(s) => format!("\"{}\"", s),
            Value::Tuple(t) => {
                let items: Vec<String> = t.iter().map(Self::format_value).collect();
                format!("({})", items.join(", "))
            }
            Value::Empty => "()".to_string(),
        }
    }

    /// Format an error message
    fn format_error(error: &evalexpr::EvalexprError) -> String {
        let msg = error.to_string();
        // Truncate long errors (Unicode-aware)
        truncate_to_width(&msg, 50)
    }

    /// Evaluate all lines in a document, with error limits
    ///
    /// Returns (results, stopped_early, reason) where stopped_early indicates
    /// if evaluation was halted due to error limits
    pub fn evaluate_document(&mut self, text: &str) -> (Vec<LineResult>, bool, Option<String>) {
        self.reset();

        let lines: Vec<&str> = text.lines().collect();
        let total_lines = lines.len();

        // Check if document is too long
        if total_lines > MAX_CALC_LINES {
            return (
                Vec::new(),
                true,
                Some(format!(
                    "Document too long for calc mode ({} lines, max {}). Try a shorter document or switch to a different syntax mode.",
                    total_lines, MAX_CALC_LINES
                )),
            );
        }

        let mut results = Vec::with_capacity(lines.len());
        let mut consecutive_errors = 0;
        let mut total_errors = 0;

        for line in lines {
            let result = self.evaluate_line(line);

            if result.is_error {
                consecutive_errors += 1;
                total_errors += 1;

                // Check consecutive error limit
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    results.push(result);
                    return (
                        results,
                        true,
                        Some(format!(
                            "Stopped after {} consecutive errors. This doesn't look like calc content - try switching to a different syntax mode.",
                            MAX_CONSECUTIVE_ERRORS
                        )),
                    );
                }

                // Check total error limit
                if total_errors >= MAX_TOTAL_ERRORS {
                    results.push(result);
                    return (
                        results,
                        true,
                        Some(format!(
                            "Stopped after {} total errors. Consider switching to a different syntax mode for this content.",
                            MAX_TOTAL_ERRORS
                        )),
                    );
                }
            } else if !result.is_comment && result.result.is_some() {
                // Reset consecutive errors on successful evaluation
                consecutive_errors = 0;
            }

            results.push(result);
        }

        (results, false, None)
    }
}

impl Default for CalcEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// A document's results, whether evaluation stopped early, and why.
type Evaluation = (Vec<LineResult>, bool, Option<String>);

/// The text of a document with no results, for when evaluation couldn't finish.
fn unevaluated(text: &str, warning: String) -> Evaluation {
    let lines = text.lines()
        .take(MAX_CALC_LINES)
        .map(|line| LineResult {
            line: line.to_string(),
            result: None,
            is_error: false,
            is_comment: false,
        })
        .collect();
    (lines, true, Some(warning))
}

/// Evaluate a document away from the UI thread.
///
/// Drawing calls this, so the note stays readable whatever the evaluator does: a
/// panic ends this thread and not the app, and a document that takes too long is
/// shown without results.
fn evaluate_isolated(text: &str) -> Evaluation {
    let (sender, receiver) = mpsc::channel();
    let document = text.to_string();

    let started = std::thread::Builder::new()
        .name("calc".to_string())
        .stack_size(CALC_STACK_BYTES)
        .spawn(move || {
            let _ = sender.send(CalcEvaluator::new().evaluate_document(&document));
        });

    match started.map(|_| receiver.recv_timeout(CALC_DEADLINE)) {
        Ok(Ok(evaluation)) => evaluation,
        _ => unevaluated(text, t!("calc.gave_up").to_string()),
    }
}

/// Render calc mode text with syntax highlighting and inline results
pub fn render_calc(text: &str) -> Text<'static> {
    let (results, stopped_early, warning) = evaluate_isolated(text);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // If there's a warning (limits hit), show it prominently at the top
    if let Some(warning_msg) = warning {
        lines.push(Line::from(vec![
            Span::styled(
                "⚠ ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                warning_msg,
                Style::default().fg(Color::Yellow),
            ),
        ]));
        lines.push(Line::raw(""));
    }

    for result in results {
        let mut spans: Vec<Span<'static>> = Vec::new();

        if result.is_comment {
            // Comment style: green
            spans.push(Span::styled(
                result.line.clone(),
                Style::default().fg(Color::Green),
            ));
        } else if result.line.trim().is_empty() {
            // Empty line
            spans.push(Span::raw(result.line.clone()));
        } else {
            // Parse and highlight the expression. A line refused for its length
            // stays one span: highlighting makes a span per character.
            if result.line.len() > MAX_CALC_LINE_BYTES {
                spans.push(Span::raw(result.line.clone()));
            } else {
                spans.extend(highlight_expression(&result.line));
            }

            // Add result if present
            if let Some(ref res) = result.result {
                let result_style = if result.is_error {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Gray)
                };
                spans.push(Span::styled(format!("  {}", res), result_style));
            }
        }

        lines.push(Line::from(spans));
    }

    // If we stopped early, add an indicator at the end
    if stopped_early && !lines.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled(
                "... (evaluation stopped)",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    Text::from(lines)
}

/// Highlight a single expression with syntax colouring
fn highlight_expression(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_word = String::new();
    let mut current_number = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    // Built-in functions from evalexpr (both with and without math:: prefix)
    let builtin_functions = [
        // Core functions
        "min", "max", "len", "str", "floor", "round", "ceil", "abs", "if", "typeof", "contains",
        // Math namespace functions (evalexpr uses math:: prefix)
        "math", "sqrt", "cbrt", "exp", "exp2", "ln", "log", "log2", "log10", "pow", "hypot",
        "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
        "sinh", "cosh", "tanh", "asinh", "acosh", "atanh",
        "is_nan", "is_finite", "is_infinite", "is_normal",
    ];

    // Constants
    let constants = ["pi", "e", "tau", "phi", "true", "false", "nan", "inf"];

    while i < chars.len() {
        let c = chars[i];

        // Numbers (including decimals and scientific notation)
        if c.is_ascii_digit() || (c == '.' && !current_number.is_empty()) {
            if !current_word.is_empty() {
                spans.push(create_word_span(&current_word, &builtin_functions, &constants));
                current_word.clear();
            }
            current_number.push(c);
            // Check for scientific notation
            if i + 1 < chars.len()
                && (chars[i + 1] == 'e' || chars[i + 1] == 'E')
                && !current_number.is_empty()
            {
                current_number.push(chars[i + 1]);
                i += 1;
                if i + 1 < chars.len() && (chars[i + 1] == '+' || chars[i + 1] == '-') {
                    current_number.push(chars[i + 1]);
                    i += 1;
                }
            }
        }
        // Word characters
        else if c.is_ascii_alphabetic() || c == '_' {
            if !current_number.is_empty() {
                spans.push(Span::styled(
                    current_number.clone(),
                    Style::default().fg(Color::LightGreen),
                ));
                current_number.clear();
            }
            current_word.push(c);
        }
        // Operators and punctuation (including :: for namespace access)
        else if "+-*/%^=()[]{},.;:<>!&|:".contains(c) {
            if !current_word.is_empty() {
                spans.push(create_word_span(&current_word, &builtin_functions, &constants));
                current_word.clear();
            }
            if !current_number.is_empty() {
                spans.push(Span::styled(
                    current_number.clone(),
                    Style::default().fg(Color::LightGreen),
                ));
                current_number.clear();
            }
            spans.push(Span::styled(
                c.to_string(),
                Style::default().fg(Color::White),
            ));
        }
        // Whitespace and other characters
        else {
            if !current_word.is_empty() {
                spans.push(create_word_span(&current_word, &builtin_functions, &constants));
                current_word.clear();
            }
            if !current_number.is_empty() {
                spans.push(Span::styled(
                    current_number.clone(),
                    Style::default().fg(Color::LightGreen),
                ));
                current_number.clear();
            }
            spans.push(Span::raw(c.to_string()));
        }

        i += 1;
    }

    // Flush remaining
    if !current_word.is_empty() {
        spans.push(create_word_span(&current_word, &builtin_functions, &constants));
    }
    if !current_number.is_empty() {
        spans.push(Span::styled(
            current_number,
            Style::default().fg(Color::LightGreen),
        ));
    }

    spans
}

/// Create a span for a word with appropriate styling
fn create_word_span(word: &str, builtin_functions: &[&str], constants: &[&str]) -> Span<'static> {
    let word_lower = word.to_lowercase();

    if builtin_functions.contains(&word_lower.as_str()) {
        // Built-in function: cyan
        Span::styled(word.to_string(), Style::default().fg(Color::Cyan))
    } else if constants.contains(&word_lower.as_str()) {
        // Constant: blue
        Span::styled(word.to_string(), Style::default().fg(Color::Blue))
    } else {
        // Variable: light blue
        Span::styled(word.to_string(), Style::default().fg(Color::LightBlue))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        let mut eval = CalcEvaluator::new();
        let (results, stopped, _) = eval.evaluate_document("2 + 2");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Some("4".to_string()));
        assert!(!results[0].is_error);
    }

    #[test]
    fn test_variable_assignment() {
        let mut eval = CalcEvaluator::new();
        // Assignments should now show the assigned value
        let (results, stopped, _) = eval.evaluate_document("x = 10\nx * 2");
        assert!(!stopped);
        assert_eq!(results.len(), 2);
        // Assignment should show the assigned value
        assert!(!results[0].is_error);
        assert_eq!(results[0].result, Some("10".to_string()));
        // Variable can be used in subsequent expressions
        assert_eq!(results[1].result, Some("20".to_string()));
    }

    #[test]
    fn test_comments() {
        let mut eval = CalcEvaluator::new();
        let (results, stopped, _) = eval.evaluate_document("# This is a comment\n2 + 2");
        assert!(!stopped);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_comment);
        assert_eq!(results[0].result, None);
        assert_eq!(results[1].result, Some("4".to_string()));
    }

    #[test]
    fn test_constants() {
        let mut eval = CalcEvaluator::new();
        let (results, stopped, _) = eval.evaluate_document("pi");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_some());
        let result = results[0].result.as_ref().unwrap();
        assert!(result.starts_with("3.14159"));
    }

    #[test]
    fn test_functions() {
        let mut eval = CalcEvaluator::new();
        // sqrt should work without math:: prefix (preprocessor adds it)
        let (results, stopped, _) = eval.evaluate_document("sqrt(16)");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Some("4".to_string()));
    }

    #[test]
    fn test_floor() {
        let mut eval = CalcEvaluator::new();
        let (results, stopped, _) = eval.evaluate_document("floor(3.7)");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Some("3".to_string()));
    }

    #[test]
    fn test_abs() {
        let mut eval = CalcEvaluator::new();
        // abs should work without math:: prefix
        let (results, stopped, _) = eval.evaluate_document("abs(-7)");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Some("7".to_string()));
    }

    #[test]
    fn test_trig_functions() {
        let mut eval = CalcEvaluator::new();
        // sin(pi/2) should be approximately 1
        let (results, stopped, _) = eval.evaluate_document("sin(pi/2)");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_error);
        let result = results[0].result.as_ref().unwrap();
        let value: f64 = result.parse().unwrap();
        assert!((value - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_log_function() {
        let mut eval = CalcEvaluator::new();
        // log(10) should be natural log (ln) = approximately 2.302585
        let (results, stopped, _) = eval.evaluate_document("log(10)");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_error, "log(10) should not error: {:?}", results[0].result);
        let result = results[0].result.as_ref().unwrap();
        let value: f64 = result.parse().unwrap();
        assert!((value - 2.302585).abs() < 0.0001, "log(10) should be ~2.302585, got {}", value);
    }

    #[test]
    fn test_log10_function() {
        let mut eval = CalcEvaluator::new();
        // log10(100) should be 2
        let (results, stopped, _) = eval.evaluate_document("log10(100)");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_error);
        assert_eq!(results[0].result, Some("2".to_string()));
    }

    #[test]
    fn test_factorial() {
        let mut eval = CalcEvaluator::new();
        let (results, stopped, _) = eval.evaluate_document("5!");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Some("120".to_string()));
    }

    #[test]
    fn test_power_operator() {
        let mut eval = CalcEvaluator::new();
        // Test ** syntax (Python-style)
        let (results, stopped, _) = eval.evaluate_document("2**10");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Some("1024".to_string()));
    }

    #[test]
    fn test_power_associativity() {
        let mut eval = CalcEvaluator::new();
        // 2^3^2 should be 2^(3^2) = 2^9 = 512 (right-to-left associativity)
        let (results, stopped, _) = eval.evaluate_document("2^3^2");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Some("512".to_string()));
    }

    #[test]
    fn test_float_division() {
        let mut eval = CalcEvaluator::new();
        // 4/5 should be 0.8, not 0 (integer division)
        let (results, stopped, _) = eval.evaluate_document("4/5");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Some("0.8".to_string()));
    }

    #[test]
    fn test_complex_expression() {
        let mut eval = CalcEvaluator::new();
        // 1 + 2 * 3 - 4 / 5 = 1 + 6 - 0.8 = 6.2
        let (results, stopped, _) = eval.evaluate_document("1 + 2 * 3 - 4 / 5");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Some("6.2".to_string()));
    }

    #[test]
    fn test_error_handling() {
        let mut eval = CalcEvaluator::new();
        // Undefined variables are now silently suppressed (no result shown)
        let (results, stopped, _) = eval.evaluate_document("undefined_var");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_error);
        assert_eq!(results[0].result, None); // No error shown for undefined vars

        // Test that other errors still show
        let (results, stopped, _) = eval.evaluate_document("1 / 0");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        // Division by zero should still work in floating point (returns Infinity)
        assert!(!results[0].is_error);
    }

    #[test]
    fn test_consecutive_errors_stop() {
        let mut eval = CalcEvaluator::new();
        // Create content that will produce many consecutive errors
        // Use syntax errors instead of undefined vars (which are now suppressed)
        let error_lines = "1 ++ 2\n".repeat(MAX_CONSECUTIVE_ERRORS + 5);
        let (results, stopped, warning) = eval.evaluate_document(&error_lines);
        assert!(stopped);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("consecutive errors"));
        // Should have stopped at MAX_CONSECUTIVE_ERRORS
        assert_eq!(results.len(), MAX_CONSECUTIVE_ERRORS);
    }

    /// The reported crash: 200 KB of brackets overflowed the stack. An abort can't
    /// be caught, so reaching the end of this test is the assertion.
    #[test]
    fn deep_nesting_is_refused_without_reaching_the_evaluator() {
        let mut eval = CalcEvaluator::new();
        let line = format!("{}1{}", "(".repeat(100_000), ")".repeat(100_000));
        let (results, _, _) = eval.evaluate_document(&line);
        assert!(results[0].is_error);
    }

    #[test]
    fn nesting_inside_the_length_limit_is_refused_too() {
        let mut eval = CalcEvaluator::new();
        let line = format!("{}1{}", "(".repeat(100), ")".repeat(100));
        assert!(line.len() <= MAX_CALC_LINE_BYTES);
        let (results, _, _) = eval.evaluate_document(&line);
        assert!(results[0].is_error);
        assert!(results[0].result.as_ref().unwrap().contains(&MAX_CALC_NESTING.to_string()));
    }

    #[test]
    fn reasonable_nesting_still_works() {
        let mut eval = CalcEvaluator::new();
        let line = format!("{}1{}", "(".repeat(20), ")".repeat(20));
        let (results, _, _) = eval.evaluate_document(&line);
        assert_eq!(results[0].result, Some("1".to_string()));
    }

    #[test]
    fn a_string_cannot_be_doubled_line_by_line() {
        let mut eval = CalcEvaluator::new();
        let mut note = String::from("a=\"xxxxxxxxxxxxxxxx\"\n");
        note.push_str(&"a=a+a\n".repeat(60));
        eval.evaluate_document(&note);
        assert!(eval.context.get_value("a").is_none());
    }

    #[test]
    fn a_tuple_cannot_be_doubled_line_by_line() {
        let mut eval = CalcEvaluator::new();
        let mut note = String::from("a=(1,2)\n");
        note.push_str(&"a=(a,a)\n".repeat(60));
        eval.evaluate_document(&note);
        assert!(eval.context.get_value("a").is_none());
    }

    #[test]
    fn an_assignment_hidden_in_an_expression_cannot_keep_a_tuple() {
        let mut eval = CalcEvaluator::new();
        let (results, _, _) = eval.evaluate_document("1+(b=(1,2))\nb");
        assert!(results[0].is_error);
        assert!(eval.context.get_value("b").is_none());
    }

    #[test]
    fn a_tuple_cannot_be_doubled_within_one_line() {
        let mut eval = CalcEvaluator::new();
        let line = format!("(a=(1,1)){}", ",(a=(a,a))".repeat(80));
        assert!(line.len() <= MAX_CALC_LINE_BYTES);
        let (results, _, _) = eval.evaluate_document(&line);
        assert!(results[0].is_error);
    }

    #[test]
    fn a_chain_of_statements_is_refused() {
        let mut eval = CalcEvaluator::new();
        let (results, _, _) = eval.evaluate_document("a=1;a=2");
        assert!(results[0].is_error);
    }

    #[test]
    fn a_string_from_a_builtin_is_not_kept() {
        let mut eval = CalcEvaluator::new();
        eval.evaluate_document("a=str(1)");
        assert!(eval.context.get_value("a").is_none());
    }

    #[test]
    fn comparisons_are_not_assignments() {
        assert_eq!(CalcEvaluator::count_assignments("a == 1"), 0);
        assert_eq!(CalcEvaluator::count_assignments("a != 1 && b <= 2 || c >= 3"), 0);
        assert_eq!(CalcEvaluator::count_assignments("a = 1 == 1"), 1);
        assert_eq!(CalcEvaluator::count_assignments("a += 1"), 1);
    }

    #[test]
    fn numbers_keep_working_across_lines() {
        let mut eval = CalcEvaluator::new();
        let (results, stopped, _) = eval.evaluate_document("a = 2\na = a + a\nb = a * 2\nmin(a, b)");
        assert!(!stopped);
        assert_eq!(results[1].result, Some("4".to_string()));
        assert_eq!(results[2].result, Some("8".to_string()));
        assert_eq!(results[3].result, Some("4".to_string()));
    }

    #[test]
    fn a_stray_closing_bracket_does_not_hang() {
        let mut eval = CalcEvaluator::new();
        let (results, _, _) = eval.evaluate_document("1) milk");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn unbalanced_brackets_are_left_alone() {
        assert_eq!(CalcEvaluator::fix_power_associativity("1) milk"), "1) milk");
        assert_eq!(CalcEvaluator::fix_power_associativity("))) ((("), "))) (((");
        assert_eq!(CalcEvaluator::fix_power_associativity("2^3^2)"), "2^(3^2))");
    }

    #[test]
    fn prose_in_a_calc_note_renders() {
        let text = render_calc("1) milk\n2) eggs (large\n3) bread)))");
        assert!(text.lines.len() >= 3);
    }

    #[test]
    fn rendering_keeps_the_text_of_a_refused_line() {
        let text = render_calc(&"9".repeat(MAX_CALC_LINE_BYTES + 1));
        let first: String = text.lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(first.starts_with("999"));
    }

    #[test]
    fn a_document_with_no_result_still_shows_its_text() {
        let (results, stopped, warning) = unevaluated("2 + 2\nnotes", "too slow".to_string());
        assert_eq!(results.len(), 2);
        assert_eq!(results[1].line, "notes");
        assert!(stopped);
        assert_eq!(warning.as_deref(), Some("too slow"));
    }

    #[test]
    fn test_document_too_long() {
        let mut eval = CalcEvaluator::new();
        // Create a document that exceeds the line limit
        let long_doc = "1 + 1\n".repeat(MAX_CALC_LINES + 10);
        let (results, stopped, warning) = eval.evaluate_document(&long_doc);
        assert!(stopped);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("too long"));
        // No lines should be evaluated
        assert_eq!(results.len(), 0);
    }
}
