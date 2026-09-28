//! `KBICode.java` — resolves `#code[day_offset]` syntax to a value. Not a `KBI` subtype; a
//! formula-engine helper, hence living in `formula/` rather than `kbi/` (`DATA_MODEL.md` §3).

use super::context::FormulaContext;
use super::reader::FormulaReader;
use crate::kbi::KbiStatType;
use crate::engine::ForecasterError;
use joda_rs::LocalDate;

#[derive(Debug, Clone, PartialEq)]
pub struct KbiCodeRef {
    pub kbi_code: String,
    pub day_offset: i32,
}

/// `new KBICode(formulaReader, formula)`, called with the reader positioned just after the
/// leading `#` (the caller — `KBIFormula::parse` or a function's argument parser — consumes it).
pub fn parse_kbi_code_ref(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<KbiCodeRef, ForecasterError> {
    let kbi_code = read_kbi_code_text(owner_kbi_code, reader)?;
    let day_offset = read_day_offset(owner_kbi_code, reader)?;
    Ok(KbiCodeRef {
        kbi_code,
        day_offset,
    })
}

fn read_kbi_code_text(
    owner_kbi_code: &str,
    reader: &mut FormulaReader,
) -> Result<String, ForecasterError> {
    let mut temp = String::new();
    let mut found_bracket = false;

    while reader.has_next() {
        let ch = reader.next();
        if ch == '[' {
            found_bracket = true;
            break;
        }
        temp.push(ch);
    }

    if !found_bracket {
        return Err(ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: "Unable to locate the day offset bracket.".to_string(),
        });
    }

    Ok(temp)
}

fn read_day_offset(owner_kbi_code: &str, reader: &mut FormulaReader) -> Result<i32, ForecasterError> {
    let mut temp = String::new();
    let mut found_bracket = false;

    while reader.has_next() {
        let ch = reader.next();
        if ch == ']' {
            found_bracket = true;
            break;
        }
        temp.push(ch);
    }

    if !found_bracket {
        return Err(ForecasterError::KbiCompute {
            kbi_code: owner_kbi_code.to_string(),
            message: "Unable to find the closing bracket for the day offset.".to_string(),
        });
    }

    temp.parse::<i32>().map_err(|_| ForecasterError::KbiCompute {
        kbi_code: owner_kbi_code.to_string(),
        message: format!("'{temp}' is not a valid day offset."),
    })
}

impl KbiCodeRef {
    /// `KBICode.calculate(date, kbiReaderWriter, valueType)`.
    pub fn evaluate(
        &self,
        owner_kbi_code: &str,
        date: LocalDate,
        stat_type: KbiStatType,
        ctx: &dyn FormulaContext,
    ) -> Result<f64, ForecasterError> {
        let kbi_id = ctx.kbi_id_for_code(&self.kbi_code)?.ok_or_else(|| {
            ForecasterError::KbiCompute {
                kbi_code: owner_kbi_code.to_string(),
                message: format!(
                    "Unable to find the KBI {} in the database.",
                    self.kbi_code
                ),
            }
        })?;

        let date_to_read = date.plus_days(self.day_offset as i64);

        ctx.read_kbi_stat_value(kbi_id, date_to_read, stat_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_code_and_offset() {
        let mut reader = FormulaReader::new("ROOMS[-1]rest");
        let kbi_code = parse_kbi_code_ref("OWNER", &mut reader).unwrap();
        assert_eq!(kbi_code.kbi_code, "ROOMS");
        assert_eq!(kbi_code.day_offset, -1);
        assert_eq!(reader.next(), 'r');
    }

    #[test]
    fn missing_bracket_is_an_error() {
        let mut reader = FormulaReader::new("ROOMS");
        assert!(parse_kbi_code_ref("OWNER", &mut reader).is_err());
    }
}
