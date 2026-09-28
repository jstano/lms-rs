//! Phase 1 — the formula engine (`FormulaReader`, `KBIFormula`, `Function`, `Operation`,
//! `KBICode`). Pure, self-contained, no DB access — everything that needs data from outside the
//! formula text itself goes through [`context::FormulaContext`]. See `../DATA_MODEL.md`.

mod context;
mod function;
mod guard;
mod kbi_code;
mod operation;
mod reader;
mod token;

pub use context::{FormulaContext, MarketSegmentType};
pub use function::FunctionCall;
pub use guard::FormulaGuard;
pub use kbi_code::KbiCodeRef;
pub use operation::Op;
pub use reader::FormulaReader;
pub use token::{evaluate, parse_formula, FormulaToken};
