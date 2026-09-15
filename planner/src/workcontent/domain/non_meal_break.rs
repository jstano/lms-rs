/// A paid rest break: every `break_every` hours of a shift, `break_length` hours
/// of paid break time are incurred.
///
/// As with [`crate::workcontent::domain::meal_break::MealBreak`], Java signals
/// "no break" with negative values rather than a null.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonMealBreak {
    pub break_every: f64,
    pub break_length: f64,
}

impl NonMealBreak {
    pub fn new(break_every: f64, break_length: f64) -> Self {
        Self {
            break_every,
            break_length,
        }
    }

    pub fn with_no_break() -> Self {
        Self::new(-1.0, -1.0)
    }

    pub fn is_no_break(&self) -> bool {
        self.break_every < 0.0 || self.break_length < 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configured_break_is_not_a_no_break() {
        assert!(!NonMealBreak::new(4.0, 0.25).is_no_break());
    }

    #[test]
    fn the_no_break_sentinel_is_a_no_break() {
        assert!(NonMealBreak::with_no_break().is_no_break());
    }

    #[test]
    fn either_negative_field_means_no_break() {
        assert!(NonMealBreak::new(-1.0, 0.25).is_no_break());
        assert!(NonMealBreak::new(4.0, -1.0).is_no_break());
    }
}
