/// A paid meal break: after `break_after` hours of a shift, `break_length` hours
/// of paid break time are incurred.
///
/// Java models "no break" with a sentinel of negative values rather than a null,
/// so `is_no_break` is preserved here even though the settings hold an `Option`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MealBreak {
    pub break_after: f64,
    pub break_length: f64,
}

impl MealBreak {
    pub fn new(break_after: f64, break_length: f64) -> Self {
        Self {
            break_after,
            break_length,
        }
    }

    pub fn with_no_break() -> Self {
        Self::new(-1.0, -1.0)
    }

    pub fn is_no_break(&self) -> bool {
        self.break_after < 0.0 || self.break_length < 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configured_break_is_not_a_no_break() {
        assert!(!MealBreak::new(5.0, 0.5).is_no_break());
    }

    #[test]
    fn the_no_break_sentinel_is_a_no_break() {
        assert!(MealBreak::with_no_break().is_no_break());
    }

    #[test]
    fn either_negative_field_means_no_break() {
        assert!(MealBreak::new(-1.0, 0.5).is_no_break());
        assert!(MealBreak::new(5.0, -1.0).is_no_break());
    }
}
