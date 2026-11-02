//! `AnniversaryDateTypes`, the dates `AnniversaryEarningRuleImpl` can pay on.
//!
//! **Source not located.** Java imports
//! `com.unifocus.watson.common.ta.anniversarydates.AnniversaryDateTypes`, which
//! is not in the `taps` checkout (nor, as far as a search found, the other
//! monorepo modules). The three constants and their display names are
//! reconstructed from the rule and its spec:
//!
//! - the rule's `getDateForAnniversaryType` handles exactly `HIRE_DATE`,
//!   `SENIORITY_DATE` and `BIRTHDAY` (any other constant would yield no date);
//! - the spec's notes read `(Hire Date)`, `(Seniority Date)`, `(Birthday)`,
//!   which is what `toString()` must return;
//! - the spec configures `[BIRTHDAY,SENIORITY_DATE,HIRE_DATE]` yet the earnings
//!   come out hire, seniority, birthday: an `EnumMap` iterates in declaration
//!   order, so that is the declaration order.
//!
//! If the real enum has further constants they are unreachable in the rule, so
//! only their *config parsing* (an unknown name makes `valueOf` throw) could
//! differ — see divergence 90.

/// A kind of anniversary date on an employee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnniversaryDateTypes {
    /// `HIRE_DATE`.
    HireDate,
    /// `SENIORITY_DATE`.
    SeniorityDate,
    /// `BIRTHDAY`.
    Birthday,
}

impl AnniversaryDateTypes {
    /// The constant's `name()`, as stored in a rule's JSON array.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "HIRE_DATE" => Some(Self::HireDate),
            "SENIORITY_DATE" => Some(Self::SeniorityDate),
            "BIRTHDAY" => Some(Self::Birthday),
            _ => None,
        }
    }

    /// The constant's `toString()`, used in an earning's note.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::HireDate => "Hire Date",
            Self::SeniorityDate => "Seniority Date",
            Self::Birthday => "Birthday",
        }
    }
}

/// Parse a rule parameter such as `[BIRTHDAY, "HIRE_DATE"]`.
///
/// Java uses `org.json.JSONArray`, which accepts unquoted strings, and the
/// spec stores them unquoted. Names are trimmed and may be single- or
/// double-quoted; unknown names are dropped (divergence 90). A value that is
/// not a bracketed list yields nothing.
pub fn parse_anniversary_types(value: &str) -> Vec<AnniversaryDateTypes> {
    let trimmed = value.trim();
    let Some(inner) = trimmed.strip_prefix('[').and_then(|v| v.strip_suffix(']')) else {
        return Vec::new();
    };
    inner
        .split(',')
        .map(|name| name.trim().trim_matches(|c| c == '"' || c == '\''))
        .filter_map(AnniversaryDateTypes::from_name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_order_is_hire_seniority_birthday() {
        assert!(AnniversaryDateTypes::HireDate < AnniversaryDateTypes::SeniorityDate);
        assert!(AnniversaryDateTypes::SeniorityDate < AnniversaryDateTypes::Birthday);
    }

    #[test]
    fn unquoted_and_quoted_names_parse() {
        assert_eq!(
            parse_anniversary_types("[BIRTHDAY, \"HIRE_DATE\"]"),
            vec![
                AnniversaryDateTypes::Birthday,
                AnniversaryDateTypes::HireDate
            ]
        );
    }

    #[test]
    fn empty_malformed_and_unknown_yield_nothing() {
        assert!(parse_anniversary_types("[]").is_empty());
        assert!(parse_anniversary_types("BIRTHDAY").is_empty());
        assert!(parse_anniversary_types("[NOPE]").is_empty());
    }
}
