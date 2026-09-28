//! `Function.java` and its ~15 concrete subclasses — the `@NAME(...)` dispatch table.
//!
//! `AVG`/`SUM`/`MAX`/`MIN` are pure combinators over sub-formula text (`ExpressionListFunction`,
//! `MaxFunction`/`MinFunction`) and are parsed into nested `FormulaToken` ASTs up front, per
//! `DATA_MODEL.md`'s "parse once, no lazy caching" decision — Java instead reparses each
//! sub-expression's text from scratch on every `calculate()` call by constructing a fresh
//! `KBIFormula`; the Rust port only pays that parsing cost once, at formula-parse time.
//!
//! Every other function (`ARRIVALSTTL`/`DEPARTSTTL`/`GUESTSTTL`/`ROOMSTTL`/`REVTTL`/`CFGROOMS`/
//! `READKBI`/`READLABORKBI`/`READBUDGETKBI`/`SDLM`/`PAVG`) is I/O-bound in Java (market-segment and
//! revenue-center SQL, `Property.getNumberRooms()`, `PastAverageAnalyzer`) or needs a KBI id
//! resolved by code — both are modeled behind [`FormulaContext`] (`context.rs`) rather than
//! performed here. One further deviation from Java: those functions resolve their referenced KBI
//! id eagerly, during *parsing* (`getKBIID` runs a raw SQL query from inside the constructor) —
//! the Rust port defers that resolution to evaluation time via `FormulaContext::kbi_id_for_code`,
//! keeping parsing itself pure/DB-free, the same tradeoff already made for `KBICode`
//! (`kbi_code.rs`).
//!
//! `DAYSINPERIOD` is a literal Java no-op: `DaysInPeriodFunction.calculate()` always returns
//! `0.0` regardless of its parsed offset. Preserved as-is, not "fixed" — see `FunctionCall::DaysInPeriod`.
//!
//! `@STAT`/`StatisticalFunction` is parsed nowhere in `Function.createFunction`'s dispatch table —
//! dead code in the Java source — and is not ported.
//!
//! **Parity note on `READKBI`/`READLABORKBI`/`READBUDGETKBI`'s "optional" trailing arguments**:
//! `ReadLaborKBIFunction`'s javadoc says the data-slot argument "is optional and will default to
//! ADJ", and its constructor does set that default when the KBI-code segment's scan hits the
//! call's closing paren early — but it then *unconditionally* calls `parseDayOffset`/
//! `parseDataSlot` anyway, which resume scanning from wherever the reader cursor now sits (past
//! the closing paren, into whatever formula text follows) and unconditionally overwrite the
//! default. The 1-/2-argument shortcut is therefore unreachable in practice; every real formula
//! must supply the full `@READKBI(#code,offset,slot)` / `@READBUDGETKBI(#code,slot)` form. This
//! port requires the full form and does not attempt to special-case the documented-but-dead
//! shortcut — replicating the *actual* runtime behavior, not the javadoc.

use super::context::{FormulaContext, MarketSegmentType};
use super::reader::FormulaReader;
use super::token::{self, FormulaToken};
use crate::kbi::KbiStatType;
use crate::engine::ForecasterError;
use joda_rs::LocalDate;

/// `MaxFunction`/`MinFunction`'s two argument expressions, each already parsed to an AST.
pub type ExprPair = Box<(Vec<FormulaToken>, Vec<FormulaToken>)>;

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionCall {
    Average(Vec<Vec<FormulaToken>>),
    Sum(Vec<Vec<FormulaToken>>),
    Max(ExprPair),
    Min(ExprPair),
    ArrivalsTotal {
        segment: MarketSegmentType,
        day_offset: i32,
    },
    DeparturesTotal {
        segment: MarketSegmentType,
        day_offset: i32,
    },
    GuestsTotal {
        segment: MarketSegmentType,
        day_offset: i32,
    },
    RoomsTotal {
        segment: MarketSegmentType,
        day_offset: i32,
    },
    RevenueTotal {
        revenue_center_name: String,
        units: String,
        day_offset: i32,
    },
    ConfigRooms,
    ReadLaborKbi {
        kbi_code: String,
        day_offset: i32,
        data_slot: KbiStatType,
    },
    ReadBudgetKbi {
        kbi_code: String,
    },
    /// Always evaluates to `0.0` — see the module doc comment.
    DaysInPeriod,
    SameDayLastMonth {
        kbi_code: String,
        day_offset: i32,
    },
    PastAverage {
        kbi_code: String,
        number_data_points: i32,
    },
}

/// `Function.createFunction(formula, formulaReader)`, called with the reader positioned just
/// after the leading `@`.
pub fn parse_function_call(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<FunctionCall, ForecasterError> {
    let name = parse_function_name(owner_kbi_code, reader)?;

    match name.to_ascii_uppercase().as_str() {
        "ARRIVALSTTL" => {
            let (segment, day_offset) = parse_market_segment_total(owner_kbi_code, reader)?;
            Ok(FunctionCall::ArrivalsTotal { segment, day_offset })
        }
        "AVG" => Ok(FunctionCall::Average(parse_expression_list(
            owner_kbi_code,
            reader,
        )?)),
        "CFGROOMS" => {
            parse_no_args(owner_kbi_code, reader)?;
            Ok(FunctionCall::ConfigRooms)
        }
        "DEPARTSTTL" => {
            let (segment, day_offset) = parse_market_segment_total(owner_kbi_code, reader)?;
            Ok(FunctionCall::DeparturesTotal { segment, day_offset })
        }
        "GUESTSTTL" => {
            let (segment, day_offset) = parse_market_segment_total(owner_kbi_code, reader)?;
            Ok(FunctionCall::GuestsTotal { segment, day_offset })
        }
        "MAX" => Ok(FunctionCall::Max(parse_two_expr(owner_kbi_code, reader)?)),
        "MIN" => Ok(FunctionCall::Min(parse_two_expr(owner_kbi_code, reader)?)),
        "PAVG" => parse_past_average(owner_kbi_code, reader),
        "REVTTL" => parse_revenue_total(owner_kbi_code, reader),
        "ROOMSTTL" => {
            let (segment, day_offset) = parse_market_segment_total(owner_kbi_code, reader)?;
            Ok(FunctionCall::RoomsTotal { segment, day_offset })
        }
        "SUM" => Ok(FunctionCall::Sum(parse_expression_list(
            owner_kbi_code,
            reader,
        )?)),
        "READKBI" | "READLABORKBI" => parse_read_labor_kbi(owner_kbi_code, reader),
        "READBUDGETKBI" => parse_read_budget_kbi(owner_kbi_code, reader),
        "DAYSINPERIOD" => {
            parse_days_in_period(owner_kbi_code, reader)?;
            Ok(FunctionCall::DaysInPeriod)
        }
        "SDLM" => parse_same_day_last_month(owner_kbi_code, reader),
        _ => Err(ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: "Invalid function name".to_string(),
        }),
    }
}

pub fn evaluate_function_call(
    call: &FunctionCall,
    owner_kbi_code: &str,
    date: LocalDate,
    stat_type: KbiStatType,
    ctx: &dyn FormulaContext,
) -> Result<f64, ForecasterError> {
    match call {
        FunctionCall::Average(exprs) => {
            let mut total = 0.0;
            let mut count = 0;
            for expr in exprs {
                let value = token::evaluate(expr, owner_kbi_code, date, stat_type, ctx)?;
                if value != 0.0 {
                    total += value;
                    count += 1;
                }
            }
            Ok(if count > 0 { total / count as f64 } else { 0.0 })
        }
        FunctionCall::Sum(exprs) => {
            let mut total = 0.0;
            for expr in exprs {
                total += token::evaluate(expr, owner_kbi_code, date, stat_type, ctx)?;
            }
            Ok(total)
        }
        FunctionCall::Max(pair) => {
            let v1 = token::evaluate(&pair.0, owner_kbi_code, date, stat_type, ctx)?;
            let v2 = token::evaluate(&pair.1, owner_kbi_code, date, stat_type, ctx)?;
            Ok(if v1 >= v2 { v1 } else { v2 })
        }
        FunctionCall::Min(pair) => {
            let v1 = token::evaluate(&pair.0, owner_kbi_code, date, stat_type, ctx)?;
            let v2 = token::evaluate(&pair.1, owner_kbi_code, date, stat_type, ctx)?;
            Ok(if v1 <= v2 { v1 } else { v2 })
        }
        FunctionCall::ArrivalsTotal { segment, day_offset } => {
            ctx.arrivals_total(*segment, date, *day_offset, stat_type)
        }
        FunctionCall::DeparturesTotal { segment, day_offset } => {
            ctx.departures_total(*segment, date, *day_offset, stat_type)
        }
        FunctionCall::GuestsTotal { segment, day_offset } => {
            ctx.guests_total(*segment, date, *day_offset, stat_type)
        }
        FunctionCall::RoomsTotal { segment, day_offset } => {
            ctx.rooms_total(*segment, date, *day_offset, stat_type)
        }
        FunctionCall::RevenueTotal {
            revenue_center_name,
            units,
            day_offset,
        } => ctx.revenue_total(revenue_center_name, units, date, *day_offset, stat_type),
        FunctionCall::ConfigRooms => ctx.config_rooms(),
        FunctionCall::ReadLaborKbi {
            kbi_code,
            day_offset,
            data_slot,
        } => {
            let kbi_id = resolve_kbi_id(owner_kbi_code, kbi_code, ctx)?;
            let stat_date = date.plus_days(-(*day_offset as i64));
            ctx.read_kbi_stat_value(kbi_id, stat_date, *data_slot)
        }
        FunctionCall::ReadBudgetKbi { kbi_code } => {
            let kbi_id = resolve_kbi_id(owner_kbi_code, kbi_code, ctx)?;
            ctx.read_kbi_stat_value(kbi_id, date, stat_type)
        }
        FunctionCall::DaysInPeriod => Ok(0.0),
        FunctionCall::SameDayLastMonth {
            kbi_code,
            day_offset,
        } => {
            let kbi_id = resolve_kbi_id(owner_kbi_code, kbi_code, ctx)?;
            let stat_date = date.plus_days(-(*day_offset as i64)).minus_months(1);
            ctx.read_kbi_stat_value(kbi_id, stat_date, stat_type)
        }
        FunctionCall::PastAverage {
            kbi_code,
            number_data_points,
        } => {
            let kbi_id = resolve_kbi_id(owner_kbi_code, kbi_code, ctx)?;
            ctx.past_average(kbi_id, *number_data_points, date)
        }
    }
}

fn resolve_kbi_id(
    owner_kbi_code: &str,
    referenced_kbi_code: &str,
    ctx: &dyn FormulaContext,
) -> Result<crate::KbiId, ForecasterError> {
    ctx.kbi_id_for_code(referenced_kbi_code)?
        .ok_or_else(|| ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: format!("Unable to find the KBI {referenced_kbi_code} in the database."),
        })
}

// --- parsing helpers -------------------------------------------------------------------------

fn err_param_count(owner_kbi_code: &str) -> ForecasterError {
    ForecasterError::KbiCompute {
        kbi_code: owner_kbi_code.to_string(),
        message: "The function has an incorrect number of parameters.".to_string(),
    }
}

fn err_missing_close_paren(owner_kbi_code: &str) -> ForecasterError {
    ForecasterError::KbiCompute {
        kbi_code: owner_kbi_code.to_string(),
        message: "Unable to find the closing parenthesis for the function parameters.".to_string(),
    }
}

fn read_until(reader: &mut FormulaReader, stop: char) -> (String, bool) {
    let mut buf = String::new();
    while reader.has_next() {
        let ch = reader.next();
        if ch == stop {
            return (buf, true);
        }
        buf.push(ch);
    }
    (buf, false)
}

fn read_until_any(reader: &mut FormulaReader, stops: &[char]) -> (String, Option<char>) {
    let mut buf = String::new();
    while reader.has_next() {
        let ch = reader.next();
        if stops.contains(&ch) {
            return (buf, Some(ch));
        }
        buf.push(ch);
    }
    (buf, None)
}

/// `Function.getFunctionName`.
fn parse_function_name(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<String, ForecasterError> {
    let mut buf = String::new();
    while reader.has_next() {
        let ch = reader.next();
        if ch == '(' {
            return Ok(buf);
        }
        buf.push(ch);
    }
    Err(ForecasterError::KbiCompute {
        kbi_code: owner_kbi_code.to_string(),
        message: "Unable to find the opening paranthesis for the function parameters.".to_string(),
    })
}

fn parse_no_args(owner_kbi_code: &str, reader: &mut FormulaReader) -> Result<(), ForecasterError> {
    let (_, found) = read_until(reader, ')');
    if !found {
        return Err(ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: "Unable to find the closing paranthesis for the function parameters."
                .to_string(),
        });
    }
    Ok(())
}

fn parse_day_offset_until_close_paren(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<i32, ForecasterError> {
    let (text, found) = read_until(reader, ')');
    if !found {
        return Err(err_missing_close_paren(owner_kbi_code));
    }
    text.trim()
        .parse::<i32>()
        .map_err(|_| ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: "The day offset parameter must be numeric.".to_string(),
        })
}

/// `MarketSegmentTotalFunction`'s constructor — shared by `ARRIVALSTTL`/`DEPARTSTTL`/
/// `GUESTSTTL`/`ROOMSTTL`.
fn parse_market_segment_total(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<(MarketSegmentType, i32), ForecasterError> {
    let (market_text, found) = read_until(reader, ',');
    if !found {
        return Err(err_param_count(owner_kbi_code));
    }
    let segment = MarketSegmentType::from_str_ci(&market_text).ok_or_else(|| {
        ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: "The segment type parameter is not correct. It must be one of ALL,TRANSIENT,GROUP,CONTRACT, or CASINO."
                .to_string(),
        }
    })?;
    let day_offset = parse_day_offset_until_close_paren(owner_kbi_code, reader)?;
    Ok((segment, day_offset))
}

/// `ExpressionListFunction`'s constructor — shared by `AVG`/`SUM`. Tracks paren depth so that
/// commas inside a nested `@FUNC(...)` call or `(subformula)` don't split the list early.
fn parse_expression_list(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<Vec<Vec<FormulaToken>>, ForecasterError> {
    let mut expression_texts = Vec::new();
    let mut buf = String::new();
    let mut number_parens = 1;
    let mut parsed_ok = false;

    while reader.has_next() {
        let mut ch = reader.next();
        if ch == '(' {
            number_parens += 1;
        } else if ch == ')' {
            number_parens -= 1;
            if number_parens == 0 {
                expression_texts.push(buf.clone());
                parsed_ok = true;
                break;
            }
        } else if ch == ',' && number_parens == 1 {
            expression_texts.push(buf.clone());
            buf.clear();
            ch = '\0';
        }

        if ch != '\0' {
            buf.push(ch);
        }
    }

    if !parsed_ok {
        return Err(err_param_count(owner_kbi_code));
    }

    expression_texts
        .into_iter()
        .map(|expr| token::parse_formula(owner_kbi_code, &expr))
        .collect()
}

/// `Function.parseSubFunction` — used only while scanning a `MAX`/`MIN` argument so a nested
/// `@FUNC(...)` call's own comma/paren doesn't get mistaken for the outer argument separator.
/// Stops at the first `)`, so (like Java) it does not itself handle a nested function whose
/// arguments contain literal parentheses.
fn parse_sub_function_text(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<String, ForecasterError> {
    let mut buf = String::from("@");
    while reader.has_next() {
        let ch = reader.next();
        buf.push(ch);
        if ch == ')' {
            return Ok(buf);
        }
    }
    Err(err_missing_close_paren(owner_kbi_code))
}

fn parse_expr_with_subfunctions(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
    stop: char,
) -> Result<(String, bool), ForecasterError> {
    let mut buf = String::new();
    while reader.has_next() {
        let ch = reader.next();
        if ch == '@' {
            buf.push_str(&parse_sub_function_text(owner_kbi_code, reader)?);
        } else if ch == stop {
            return Ok((buf, true));
        } else {
            buf.push(ch);
        }
    }
    Ok((buf, false))
}

/// `MaxFunction`/`MinFunction`'s constructor (`parseExpr1`/`parseExpr2`).
fn parse_two_expr(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<ExprPair, ForecasterError> {
    let (expr1_text, found1) = parse_expr_with_subfunctions(owner_kbi_code, reader, ',')?;
    if !found1 {
        return Err(err_param_count(owner_kbi_code));
    }
    let (expr2_text, found2) = parse_expr_with_subfunctions(owner_kbi_code, reader, ')')?;
    if !found2 {
        return Err(err_missing_close_paren(owner_kbi_code));
    }
    let expr1 = token::parse_formula(owner_kbi_code, &expr1_text)?;
    let expr2 = token::parse_formula(owner_kbi_code, &expr2_text)?;
    Ok(Box::new((expr1, expr2)))
}

/// `RevenueTotalFunction`'s constructor.
fn parse_revenue_total(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<FunctionCall, ForecasterError> {
    let (revenue_center_name, found1) = read_until(reader, ',');
    if !found1 {
        return Err(err_param_count(owner_kbi_code));
    }
    let (units, found2) = read_until(reader, ',');
    if !found2 {
        return Err(err_param_count(owner_kbi_code));
    }
    let day_offset = parse_day_offset_until_close_paren(owner_kbi_code, reader)?;
    Ok(FunctionCall::RevenueTotal {
        revenue_center_name,
        units,
        day_offset,
    })
}

/// `ReadLaborKBIFunction`'s constructor. See the module doc comment's parity note: this always
/// requires the full 3-argument form.
fn parse_read_labor_kbi(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<FunctionCall, ForecasterError> {
    if reader.has_next() {
        reader.next(); // skip the leading '#'
    }
    let (kbi_code, found1) = read_until_any(reader, &[',', ')']);
    if found1.is_none() {
        return Err(err_param_count(owner_kbi_code));
    }

    let (day_offset_text, found2) = read_until_any(reader, &[',', ')']);
    if found2.is_none() {
        return Err(err_param_count(owner_kbi_code));
    }
    let day_offset = day_offset_text
        .parse::<i32>()
        .map_err(|_| ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: "The day offset parameter must be numeric.".to_string(),
        })?;

    let (data_slot_text, found3) = read_until(reader, ')');
    if !found3 {
        return Err(err_param_count(owner_kbi_code));
    }
    let data_slot =
        KbiStatType::from_code(&data_slot_text).ok_or_else(|| ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: format!("'{data_slot_text}' is not a valid data slot."),
        })?;

    Ok(FunctionCall::ReadLaborKbi {
        kbi_code,
        day_offset,
        data_slot,
    })
}

/// `ReadBudgetKBIFunction`'s constructor. The data-slot argument is parsed (required, per the
/// module doc comment's parity note) but never consulted by `calculate()` in Java — kept
/// parsed-but-unused here too, for fidelity.
fn parse_read_budget_kbi(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<FunctionCall, ForecasterError> {
    if reader.has_next() {
        reader.next(); // skip the leading '#'
    }
    let (kbi_code, found1) = read_until_any(reader, &[',', ')']);
    if found1.is_none() {
        return Err(err_param_count(owner_kbi_code));
    }
    let (_data_slot, found2) = read_until(reader, ')');
    if !found2 {
        return Err(err_param_count(owner_kbi_code));
    }
    Ok(FunctionCall::ReadBudgetKbi { kbi_code })
}

/// `DaysInPeriodFunction`'s constructor. Java also validates the parsed offset against
/// `numPeriods`, derived at parse time from the property's financial-year-type — an I/O/config
/// lookup Phase 1 doesn't have access to; deferred, and moot regardless since `calculate()`
/// ignores the offset entirely (see `FunctionCall::DaysInPeriod`).
fn parse_days_in_period(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<(), ForecasterError> {
    let (text, found) = read_until(reader, ')');
    if !found {
        return Err(ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: "The number of parameters is incorrect".to_string(),
        });
    }
    if !text.is_empty() {
        text.parse::<i32>()
            .map_err(|_| ForecasterError::KbiCompute {
                kbi_code: owner_kbi_code.to_string(),
                message: "The day offset parameter must be numeric.".to_string(),
            })?;
    }
    Ok(())
}

/// `SameDayLastMonthFunction`'s constructor.
fn parse_same_day_last_month(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<FunctionCall, ForecasterError> {
    if reader.has_next() {
        reader.next(); // skip the leading '#'
    }
    let (kbi_code, found1) = read_until(reader, ',');
    if !found1 {
        return Err(err_param_count(owner_kbi_code));
    }
    let day_offset = parse_day_offset_until_close_paren(owner_kbi_code, reader)?;
    Ok(FunctionCall::SameDayLastMonth {
        kbi_code,
        day_offset,
    })
}

/// `PastAverageFunction`'s constructor. Anything from `[` up to the closing `)` (i.e. the day
/// offset the javadoc says "is not used") is scanned past and discarded, not just left unparsed.
fn parse_past_average(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<FunctionCall, ForecasterError> {
    let (num_text, found1) = read_until(reader, ',');
    if !found1 {
        return Err(err_param_count(owner_kbi_code));
    }
    let number_data_points = num_text
        .parse::<i32>()
        .map_err(|_| ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: "The number of data points parameter must be numeric.".to_string(),
        })?;

    if reader.has_next() {
        reader.next(); // skip the leading '#'
    }
    let mut kbi_code = String::new();
    let mut found_bracket = false;
    let mut found_close = false;
    while reader.has_next() {
        let ch = reader.next();
        if ch == ')' {
            found_close = true;
            break;
        } else if ch == '[' {
            found_bracket = true;
        }
        if !found_bracket {
            kbi_code.push(ch);
        }
    }
    if !found_close {
        return Err(err_missing_close_paren(owner_kbi_code));
    }

    Ok(FunctionCall::PastAverage {
        kbi_code,
        number_data_points,
    })
}
