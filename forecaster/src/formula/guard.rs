//! Resolves `KBIFormula`'s `computing` field — a mutable recursion guard on the (persistent,
//! reused-across-dates) formula object — into an explicit, threadable guard.
//!
//! Java's guard only ever matters at one re-entrancy point: `CalculatedKBI.computeValue()` calling
//! its own cached `KBIFormula.calculate()`, which can transitively (through a `#code` reference
//! resolved by `KBIList.getKBI`) reach back into the *same* KBI's `computeValue()` and its *same*
//! `KBIFormula` instance, forming a cycle. Nothing inside the pure formula AST/evaluator in this
//! module re-enters a KBI's own computation by itself (`KBICode`/`Function` only read already
//! materialized stat values through `FormulaContext`, never call back into `Kbi::compute_value`) —
//! so `FormulaGuard` is defined here, ready to be threaded through `Kbi::compute_value` once that
//! method exists (Phase 2), rather than wired into `evaluate()` prematurely.

use crate::engine::ForecasterError;
use crate::KbiId;

#[derive(Debug, Default)]
pub struct FormulaGuard {
    computing: Vec<KbiId>,
}

impl FormulaGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// `if (computing) throw new KBIComputeException(kbi.getCode(), "Recursive Error");` followed
    /// by `computing = true`, combined into one call. `kbi_code` is the KBI whose formula is being
    /// entered, used both as the cycle key and as the error's KBI-code context.
    pub fn try_enter(&mut self, kbi_id: KbiId, kbi_code: &str) -> Result<(), ForecasterError> {
        if self.computing.contains(&kbi_id) {
            return Err(ForecasterError::KbiCompute {
                kbi_code: kbi_code.to_string(),
                message: "Recursive Error".to_string(),
            });
        }
        self.computing.push(kbi_id);
        Ok(())
    }

    /// `computing = false`.
    pub fn leave(&mut self, kbi_id: KbiId) {
        if let Some(pos) = self.computing.iter().position(|&id| id == kbi_id) {
            self.computing.remove(pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_direct_recursion() {
        let mut guard = FormulaGuard::new();
        let kbi = KbiId(1);
        guard.try_enter(kbi, "ROOMS").unwrap();
        let err = guard.try_enter(kbi, "ROOMS").unwrap_err();
        assert_eq!(
            err,
            ForecasterError::KbiCompute {
                kbi_code: "ROOMS".to_string(),
                message: "Recursive Error".to_string(),
            }
        );
    }

    #[test]
    fn leaving_allows_re_entry() {
        let mut guard = FormulaGuard::new();
        let kbi = KbiId(1);
        guard.try_enter(kbi, "ROOMS").unwrap();
        guard.leave(kbi);
        assert!(guard.try_enter(kbi, "ROOMS").is_ok());
    }

    #[test]
    fn unrelated_kbis_do_not_collide() {
        let mut guard = FormulaGuard::new();
        guard.try_enter(KbiId(1), "ROOMS").unwrap();
        assert!(guard.try_enter(KbiId(2), "ARRIVALS").is_ok());
    }
}
