//! Calculator expression evaluator for calc mode
//!
//! This module provides line-by-line math expression evaluation,
//! similar to the webapp's calcExtension.ts but using evalexpr.

use evalexpr::{eval_with_context_mut, HashMapContext, Value, ContextWithMutableVariables, Context};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use regex::Regex;
use std::f64::consts::{E, PI, TAU};

/// Maximum number of lines to evaluate in calc mode
/// Documents longer than this will show a warning
const MAX_CALC_LINES: usize = 500;

/// Maximum consecutive errors before stopping evaluation
/// Prevents runaway processing on non-calc content
const MAX_CONSECUTIVE_ERRORS: usize = 10;

/// Maximum total errors before stopping evaluation
const MAX_TOTAL_ERRORS: usize = 50;

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
        let re = Regex::new(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*[^=]").ok()?;
        re.captures(line).map(|c| c[1].to_string())
    }

    /// Preprocess expression to handle syntax differences from mathjs
    fn preprocess(expr: &str) -> String {
        let mut result = expr.to_string();

        // Replace ** with ^ for power (Python-style to evalexpr-style)
        result = result.replace("**", "^");

        // Replace // with / for integer division (we'll just use regular division)
        // evalexpr doesn't have integer division, so this is approximate
        result = result.replace("//", "/");

        // Fix power associativity: mathjs uses right-to-left, evalexpr uses left-to-right
        // Convert a^b^c to a^(b^c)
        result = Self::fix_power_associativity(&result);

        // Convert integers involved in division to floats to avoid integer division
        // Match integer followed by / or / followed by integer
        result = Self::ensure_float_division(&result);

        // Map common math functions to evalexpr's math:: namespace
        // These need the math:: prefix in evalexpr
        let math_functions = [
            "sqrt", "cbrt", "exp", "exp2", "ln", "log", "log2", "log10",
            "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
            "sinh", "cosh", "tanh", "asinh", "acosh", "atanh",
            "abs", "hypot",
        ];

        for func in math_functions {
            // Match function call: word boundary + function name + opening paren
            // But skip if already prefixed with math::
            let pattern = format!(r"\b{}\s*\(", func);
            if let Ok(re) = Regex::new(&pattern) {
                // Only replace if not already prefixed
                let replacement = format!("math::{}(", func);
                let math_prefixed = format!("math::{}", func);

                // Simple approach: replace all, then fix double-prefixed ones
                result = re.replace_all(&result, &replacement).to_string();
                // Fix any double-prefixed functions
                result = result.replace(&format!("math::math::{}", func), &math_prefixed);
            }
        }

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
        if let Ok(re) = Regex::new(r"(\^)([^+\-*/^()]+)(\^)") {
            // Keep replacing until no more matches
            loop {
                let new_result = re.replace(&result, "$1($2$3").to_string();
                if new_result == result {
                    break;
                }
                result = new_result;
            }
            // Close any opened parentheses at the end
            let opens = result.matches("(").count();
            let closes = result.matches(")").count();
            for _ in 0..(opens - closes) {
                result.push(')');
            }
        }

        result
    }

    /// Ensure division uses floats to avoid integer division
    fn ensure_float_division(expr: &str) -> String {
        let mut result = expr.to_string();

        // Convert integer before / to float: "4/" -> "4.0/"
        if let Ok(re) = Regex::new(r"(\d+)(\s*/)") {
            result = re.replace_all(&result, |caps: &regex::Captures| {
                let num = &caps[1];
                let slash = &caps[2];
                // Only add .0 if not already a float
                if !num.contains('.') {
                    format!("{}.0{}", num, slash)
                } else {
                    format!("{}{}", num, slash)
                }
            }).to_string();
        }

        // Convert integer after / to float: "/4" -> "/4.0"
        if let Ok(re) = Regex::new(r"(/\s*)(\d+)") {
            result = re.replace_all(&result, |caps: &regex::Captures| {
                let slash = &caps[1];
                let num = &caps[2];
                // Only add .0 if not already a float (check next char isn't .)
                format!("{}{}.0", slash, num)
            }).to_string();
        }

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
        let re = Regex::new(r"^(\d+)!$").ok()?;
        let caps = re.captures(expr.trim())?;
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

        // Check if this is an assignment
        let assignment_var = Self::parse_assignment(trimmed);

        // Try to evaluate the expression
        match eval_with_context_mut(&processed, &mut self.context) {
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
                // Format error message
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
        // Truncate long errors (use character-aware truncation)
        if msg.chars().count() > 50 {
            let truncated: String = msg.chars().take(47).collect();
            format!("{}...", truncated)
        } else {
            msg
        }
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

/// Render calc mode text with syntax highlighting and inline results
pub fn render_calc(text: &str) -> Text<'static> {
    let mut evaluator = CalcEvaluator::new();
    let (results, stopped_early, warning) = evaluator.evaluate_document(text);

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
            // Parse and highlight the expression
            let highlighted = highlight_expression(&result.line);
            spans.extend(highlighted);

            // Add result if present
            if let Some(ref res) = result.result {
                let result_style = if result.is_error {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::DarkGray)
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
        // Whitespace
        else if c.is_whitespace() {
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
        // Other characters
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
        let (results, stopped, _) = eval.evaluate_document("undefined_var");
        assert!(!stopped);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_error);
        assert!(results[0].result.as_ref().unwrap().starts_with("Error:"));
    }

    #[test]
    fn test_consecutive_errors_stop() {
        let mut eval = CalcEvaluator::new();
        // Create content that will produce many consecutive errors
        let error_lines = "undefined\n".repeat(MAX_CONSECUTIVE_ERRORS + 5);
        let (results, stopped, warning) = eval.evaluate_document(&error_lines);
        assert!(stopped);
        assert!(warning.is_some());
        assert!(warning.unwrap().contains("consecutive errors"));
        // Should have stopped at MAX_CONSECUTIVE_ERRORS
        assert_eq!(results.len(), MAX_CONSECUTIVE_ERRORS);
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
