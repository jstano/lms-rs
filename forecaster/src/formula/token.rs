//! `KBIFormula.java`'s `parse()`/`calculateFast()` — the `FormulaToken` AST (replacing Java's raw
//! `List<Object>` + `instanceof` chain) and the two-stack shunting-yard evaluator over it.
//!
//! Per `DATA_MODEL.md`, the AST is built once (here, at parse time) rather than lazily cached on
//! first use behind a mutable flag, so there is no `OnceCell`/interior-mutability needed to match
//! Java's `parseList == null` check.

use super::context::FormulaContext;
use super::function::{self, FunctionCall};
use super::kbi_code::{self, KbiCodeRef};
use super::operation::Op;
use super::reader::FormulaReader;
use crate::kbi::KbiStatType;
use crate::engine::ForecasterError;
use joda_rs::LocalDate;

#[derive(Debug, Clone, PartialEq)]
pub enum FormulaToken {
    KbiRef(KbiCodeRef),
    Function(FunctionCall),
    Constant(f64),
    /// `(subformula)` — Java constructs a nested `KBIFormula`; here it's just a nested token list.
    SubFormula(Vec<FormulaToken>),
    Op(Op),
}

/// `KBIFormula.parse()`. `owner_kbi_code` is the KBI that owns this formula text, threaded through
/// for `KBIComputeException`-equivalent error messages exactly as `formula.kbi.getCode()` is in
/// Java.
pub fn parse_formula(
    owner_kbi_code: &str,
    formula_text: &str,
) -> Result<Vec<FormulaToken>, ForecasterError> {
    let mut tokens = Vec::new();

    if formula_text.trim().is_empty() {
        return Ok(tokens);
    }

    let mut reader = FormulaReader::new(formula_text);

    while reader.has_next() {
        reader.mark();
        let ch = reader.next();

        if ch == '#' {
            tokens.push(FormulaToken::KbiRef(kbi_code::parse_kbi_code_ref(
                owner_kbi_code,
                &mut reader,
            )?));
        } else if ch == '@' {
            tokens.push(FormulaToken::Function(function::parse_function_call(
                owner_kbi_code,
                &mut reader,
            )?));
        } else if ch == '(' {
            if let Some(sub_tokens) = parse_sub_formula(owner_kbi_code, &mut reader)? {
                tokens.push(FormulaToken::SubFormula(sub_tokens));
            }
            // An empty `()` yields `None`: Java adds a null placeholder to `parseList` in this
            // case, which every `instanceof` check in `calculateFast()` then silently skips over —
            // equivalent to not adding anything.
        } else if ch.is_ascii_digit() || ch == '.' {
            reader.reset();
            tokens.push(FormulaToken::Constant(parse_constant(
                owner_kbi_code,
                &mut reader,
            )?));
        } else if Op::is_operation_char(ch) {
            tokens.push(FormulaToken::Op(Op::from_char(ch)));
        }
        // Any other character (whitespace, stray punctuation) is silently skipped, matching
        // Java's if/else-if chain with no trailing `else`.
    }

    Ok(tokens)
}

/// `KBIFormula.getSubFormula`. Returns `None` for an empty `()`, matching Java's `result` staying
/// `null` when `subFormulaText.length() == 0`.
fn parse_sub_formula(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<Option<Vec<FormulaToken>>, ForecasterError> {
    let mut number_parens = 1;
    let mut found_closing = false;
    let mut temp = String::new();

    while reader.has_next() {
        let ch = reader.next();
        if ch == '(' {
            number_parens += 1;
        } else if ch == ')' {
            number_parens -= 1;
            if number_parens == 0 {
                found_closing = true;
                break;
            }
        }
        temp.push(ch);
    }

    if !found_closing {
        return Err(ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: "Unable to locate the closing parenthesis.".to_string(),
        });
    }

    if temp.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_formula(owner_kbi_code, &temp)?))
    }
}

/// `KBIFormula.getConstantValue`.
fn parse_constant(owner_kbi_code: &str, reader: &mut FormulaReader) -> Result<f64, ForecasterError> {
    let mut temp = String::new();

    while reader.has_next() {
        let ch = reader.next();
        if !ch.is_ascii_digit() && ch != '.' {
            reader.reset();
            break;
        }
        reader.mark();
        temp.push(ch);
    }

    temp.parse::<f64>().map_err(|_| ForecasterError::KbiCompute {
        kbi_code: owner_kbi_code.to_string(),
        message: format!("'{temp}' is not a valid numeric constant."),
    })
}

/// `KBIFormula.calculateFast` — the two-stack shunting-yard evaluator, honoring `Operation
/// ::getPriority()` (`operation.rs`).
pub fn evaluate(
    tokens: &[FormulaToken],
    owner_kbi_code: &str,
    date: LocalDate,
    stat_type: KbiStatType,
    ctx: &dyn FormulaContext,
) -> Result<f64, ForecasterError> {
    if tokens.is_empty() {
        return Ok(0.0);
    }

    let mut operands: Vec<f64> = Vec::new();
    let mut operations: Vec<Op> = Vec::new();

    for token in tokens {
        match token {
            FormulaToken::KbiRef(kbi_ref) => {
                operands.push(kbi_ref.evaluate(owner_kbi_code, date, stat_type, ctx)?);
            }
            FormulaToken::Function(call) => {
                operands.push(function::evaluate_function_call(
                    call,
                    owner_kbi_code,
                    date,
                    stat_type,
                    ctx,
                )?);
            }
            FormulaToken::Constant(value) => {
                operands.push(*value);
            }
            FormulaToken::SubFormula(sub_tokens) => {
                operands.push(evaluate(sub_tokens, owner_kbi_code, date, stat_type, ctx)?);
            }
            FormulaToken::Op(new_op) => {
                while let Some(top) = operations.last().copied() {
                    if new_op.priority() <= top.priority() {
                        operations.pop();
                        // Falls back to 0.0 on an empty stack rather than panicking (Java's
                        // `Stack.pop()` throws `EmptyStackException` here for a malformed
                        // formula) — a deliberate "stay total" deviation, not a modeled behavior.
                        let value2 = operands.pop().unwrap_or(0.0);
                        let value1 = operands.pop().unwrap_or(0.0);
                        operands.push(top.calculate(value1, value2));
                    } else {
                        break;
                    }
                }
                operations.push(*new_op);
            }
        }
    }

    // Perform any remaining computations.
    while let Some(op) = operations.pop() {
        let value2 = operands.pop();
        let value1 = operands.pop();
        match (value1, value2) {
            (Some(v1), Some(v2)) => operands.push(op.calculate(v1, v2)),
            (None, Some(v2)) if op == Op::Subtract => operands.push(op.calculate(0.0, v2)),
            _ => {}
        }
    }

    Ok(operands.pop().unwrap_or(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::context::MarketSegmentType;
    use crate::KbiId;
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct StubContext {
        kbi_ids_by_code: HashMap<String, KbiId>,
        values: RefCell<HashMap<(KbiId, LocalDate, KbiStatType), f64>>,
    }

    impl StubContext {
        fn new() -> Self {
            Self {
                kbi_ids_by_code: HashMap::new(),
                values: RefCell::new(HashMap::new()),
            }
        }

        fn with_kbi(mut self, code: &str, id: i32) -> Self {
            self.kbi_ids_by_code.insert(code.to_string(), KbiId(id));
            self
        }

        fn with_value(self, code: &str, date: LocalDate, stat_type: KbiStatType, value: f64) -> Self {
            let id = self.kbi_ids_by_code[code];
            self.values.borrow_mut().insert((id, date, stat_type), value);
            self
        }
    }

    impl FormulaContext for StubContext {
        fn kbi_id_for_code(&self, code: &str) -> Result<Option<KbiId>, ForecasterError> {
            Ok(self.kbi_ids_by_code.get(code).copied())
        }

        fn read_kbi_stat_value(
            &self,
            kbi_id: KbiId,
            date: LocalDate,
            stat_type: KbiStatType,
        ) -> Result<f64, ForecasterError> {
            Ok(self
                .values
                .borrow()
                .get(&(kbi_id, date, stat_type))
                .copied()
                .unwrap_or(0.0))
        }

        fn arrivals_total(
            &self,
            _segment: MarketSegmentType,
            _date: LocalDate,
            _day_offset: i32,
            _stat_type: KbiStatType,
        ) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }

        fn departures_total(
            &self,
            _segment: MarketSegmentType,
            _date: LocalDate,
            _day_offset: i32,
            _stat_type: KbiStatType,
        ) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }

        fn guests_total(
            &self,
            _segment: MarketSegmentType,
            _date: LocalDate,
            _day_offset: i32,
            _stat_type: KbiStatType,
        ) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }

        fn rooms_total(
            &self,
            _segment: MarketSegmentType,
            _date: LocalDate,
            _day_offset: i32,
            _stat_type: KbiStatType,
        ) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }

        fn revenue_total(
            &self,
            _revenue_center_name: &str,
            _units: &str,
            _date: LocalDate,
            _day_offset: i32,
            _stat_type: KbiStatType,
        ) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }

        fn config_rooms(&self) -> Result<f64, ForecasterError> {
            Ok(250.0)
        }

        fn past_average(
            &self,
            _kbi_id: KbiId,
            _number_data_points: i32,
            _date: LocalDate,
        ) -> Result<f64, ForecasterError> {
            Ok(0.0)
        }
    }

    fn date(y: i32, m: i32, d: i32) -> LocalDate {
        LocalDate::of(y, m, d)
    }

    #[test]
    fn evaluates_a_constant() {
        let tokens = parse_formula("ROOMS", "42").unwrap();
        let ctx = StubContext::new();
        let result = evaluate(&tokens, "ROOMS", date(2026, 1, 1), KbiStatType::Actual, &ctx).unwrap();
        assert_eq!(result, 42.0);
    }

    #[test]
    fn respects_operation_priority_not_left_to_right() {
        // Priorities: + = 1, - = 2, *|/|% = 3/4. So "2+3*4" should still evaluate as 2+(3*4)=14
        // since * (priority 4) outranks + (priority 1) in the shunting-yard loop.
        let tokens = parse_formula("KBI", "2+3*4").unwrap();
        let ctx = StubContext::new();
        let result = evaluate(&tokens, "KBI", date(2026, 1, 1), KbiStatType::Actual, &ctx).unwrap();
        assert_eq!(result, 14.0);
    }

    #[test]
    fn subtract_outranks_add_per_the_java_priorities() {
        // Priorities: + = 1, - = 2. So "2-3+4" folds the "-" first because it is higher priority
        // than the pending "+" is compared against — this mirrors Java's (surprising) ordering.
        let tokens = parse_formula("KBI", "10-2+1").unwrap();
        let ctx = StubContext::new();
        let result = evaluate(&tokens, "KBI", date(2026, 1, 1), KbiStatType::Actual, &ctx).unwrap();
        assert_eq!(result, 9.0);
    }

    #[test]
    fn resolves_a_kbi_reference_with_day_offset() {
        let ctx = StubContext::new()
            .with_kbi("ROOMS", 1)
            .with_value("ROOMS", date(2026, 1, 2), KbiStatType::Actual, 100.0);

        let tokens = parse_formula("OTHER", "#ROOMS[1]").unwrap();
        let result = evaluate(&tokens, "OTHER", date(2026, 1, 1), KbiStatType::Actual, &ctx).unwrap();
        assert_eq!(result, 100.0);
    }

    #[test]
    fn unknown_kbi_code_is_an_error() {
        let ctx = StubContext::new();
        let tokens = parse_formula("OTHER", "#MISSING[0]").unwrap();
        let err = evaluate(&tokens, "OTHER", date(2026, 1, 1), KbiStatType::Actual, &ctx).unwrap_err();
        assert_eq!(
            err,
            ForecasterError::KbiCompute {
                kbi_code: "OTHER".to_string(),
                message: "Unable to find the KBI MISSING in the database.".to_string(),
            }
        );
    }

    #[test]
    fn avg_skips_zero_values() {
        let d = date(2026, 1, 1);
        let ctx = StubContext::new()
            .with_kbi("A", 1)
            .with_value("A", d, KbiStatType::Actual, 10.0)
            .with_kbi("B", 2)
            .with_value("B", d, KbiStatType::Actual, 0.0)
            .with_kbi("C", 3)
            .with_value("C", d, KbiStatType::Actual, 20.0);

        let tokens = parse_formula("OTHER", "@AVG(#A[0],#B[0],#C[0])").unwrap();
        let result = evaluate(&tokens, "OTHER", d, KbiStatType::Actual, &ctx).unwrap();
        // (10 + 20) / 2, not / 3 — B's zero value is excluded from both the sum and the count.
        assert_eq!(result, 15.0);
    }

    #[test]
    fn sum_includes_all_values() {
        let d = date(2026, 1, 1);
        let ctx = StubContext::new()
            .with_kbi("A", 1)
            .with_value("A", d, KbiStatType::Actual, 10.0)
            .with_kbi("B", 2)
            .with_value("B", d, KbiStatType::Actual, 5.0);

        let tokens = parse_formula("OTHER", "@SUM(#A[0],#B[0])").unwrap();
        let result = evaluate(&tokens, "OTHER", d, KbiStatType::Actual, &ctx).unwrap();
        assert_eq!(result, 15.0);
    }

    #[test]
    fn max_and_min_pick_the_right_side() {
        let d = date(2026, 1, 1);
        let ctx = StubContext::new()
            .with_kbi("A", 1)
            .with_value("A", d, KbiStatType::Actual, 10.0)
            .with_kbi("B", 2)
            .with_value("B", d, KbiStatType::Actual, 20.0);

        let max_tokens = parse_formula("OTHER", "@MAX(#A[0],#B[0])").unwrap();
        assert_eq!(
            evaluate(&max_tokens, "OTHER", d, KbiStatType::Actual, &ctx).unwrap(),
            20.0
        );

        let min_tokens = parse_formula("OTHER", "@MIN(#A[0],#B[0])").unwrap();
        assert_eq!(
            evaluate(&min_tokens, "OTHER", d, KbiStatType::Actual, &ctx).unwrap(),
            10.0
        );
    }

    #[test]
    fn nested_subformula_and_function_in_an_expression() {
        let d = date(2026, 1, 1);
        let ctx = StubContext::new()
            .with_kbi("A", 1)
            .with_value("A", d, KbiStatType::Actual, 4.0);

        // (2 + #A[0]) * @CFGROOMS() -> (2 + 4) * 250 = 1500
        let tokens = parse_formula("OTHER", "(2+#A[0])*@CFGROOMS()").unwrap();
        let result = evaluate(&tokens, "OTHER", d, KbiStatType::Actual, &ctx).unwrap();
        assert_eq!(result, 1500.0);
    }

    #[test]
    fn config_rooms_function_parses_and_evaluates() {
        let ctx = StubContext::new();
        let tokens = parse_formula("OTHER", "@CFGROOMS()").unwrap();
        let result =
            evaluate(&tokens, "OTHER", date(2026, 1, 1), KbiStatType::Actual, &ctx).unwrap();
        assert_eq!(result, 250.0);
    }

    #[test]
    fn empty_formula_evaluates_to_zero() {
        let tokens = parse_formula("OTHER", "").unwrap();
        assert!(tokens.is_empty());
        let ctx = StubContext::new();
        let result =
            evaluate(&tokens, "OTHER", date(2026, 1, 1), KbiStatType::Actual, &ctx).unwrap();
        assert_eq!(result, 0.0);
    }

    #[test]
    fn missing_day_offset_bracket_is_an_error() {
        let err = parse_formula("OTHER", "#ROOMS").unwrap_err();
        assert_eq!(
            err,
            ForecasterError::KbiCompute {
                kbi_code: "OTHER".to_string(),
                message: "Unable to locate the day offset bracket.".to_string(),
            }
        );
    }

    #[test]
    fn unclosed_subformula_paren_is_an_error() {
        let err = parse_formula("OTHER", "(2+3").unwrap_err();
        assert_eq!(
            err,
            ForecasterError::KbiCompute {
                kbi_code: "OTHER".to_string(),
                message: "Unable to locate the closing parenthesis.".to_string(),
            }
        );
    }

    #[test]
    fn unknown_function_name_is_an_error() {
        let err = parse_formula("OTHER", "@NOPE(1)").unwrap_err();
        assert_eq!(
            err,
            ForecasterError::KbiCompute {
                kbi_code: "OTHER".to_string(),
                message: "Invalid function name".to_string(),
            }
        );
    }

    #[test]
    fn read_labor_kbi_requires_the_full_three_argument_form() {
        let d = date(2026, 1, 5);
        let ctx = StubContext::new()
            .with_kbi("ROOMS", 1)
            .with_value("ROOMS", date(2026, 1, 3), KbiStatType::Adjusted, 77.0);

        let tokens = parse_formula("OTHER", "@READLABORKBI(#ROOMS,2,ADJ)").unwrap();
        let result = evaluate(&tokens, "OTHER", d, KbiStatType::Actual, &ctx).unwrap();
        assert_eq!(result, 77.0);
    }

    #[test]
    fn days_in_period_always_evaluates_to_zero() {
        let ctx = StubContext::new();
        let tokens = parse_formula("OTHER", "@DAYSINPERIOD()").unwrap();
        let result =
            evaluate(&tokens, "OTHER", date(2026, 1, 1), KbiStatType::Actual, &ctx).unwrap();
        assert_eq!(result, 0.0);
    }
}
