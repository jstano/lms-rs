//! Port of `com.unifocus.watson.common.enums.MealPunchPermission`.

use crate::coded_enum;

coded_enum! {
    /// Whether an employee may record a meal punch. `MealPunchPermission`.
    MealPunchPermission {
        Allow => "A",
        Disallow => "DA",
        /// Allowed, and the time is taken off the clock.
        AllowWithOffClock => "AWOC",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in MealPunchPermission::VALUES {
            assert_eq!(MealPunchPermission::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn disallow_is_two_letters_so_it_does_not_collide_with_allow() {
        assert_eq!(MealPunchPermission::Allow.code(), "A");
        assert_eq!(MealPunchPermission::Disallow.code(), "DA");
    }
}
