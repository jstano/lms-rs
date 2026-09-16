//! Port of `com.unifocus.watson.server.hibernate.entity.Holiday`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/Holiday.java`.
//!
//! One dated holiday at one property, of one configured type. A property
//! defines its own calendar — a `HolidayType` is the category ("Federal",
//! "Floating") that rules select on, and a `Holiday` is that category landing
//! on a date in a year.
//!
//! `HolidayType` itself is not ported: the rules only ever read its id, which
//! they compare against the ids in their own parameters, so it comes across as
//! [`holiday_type_id`](Holiday::holiday_type_id) the way every other lazy
//! `@ManyToOne` in the entity model does.

use joda_rs::LocalDate;

/// A dated holiday at one property. `Holiday`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holiday {
    id: i32,
    property_id: i32,
    holiday_date: LocalDate,
    name: String,
    holiday_type_id: i32,
}

impl Holiday {
    /// Build a holiday.
    pub fn new(
        id: i32,
        property_id: i32,
        holiday_date: LocalDate,
        name: impl Into<String>,
        holiday_type_id: i32,
    ) -> Self {
        Self {
            id,
            property_id,
            holiday_date,
            name: name.into(),
            holiday_type_id,
        }
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getProperty().getID()`.
    pub fn property_id(&self) -> i32 {
        self.property_id
    }

    /// `getHolidayDate()`.
    pub fn holiday_date(&self) -> LocalDate {
        self.holiday_date
    }

    /// `getName()`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The category this holiday belongs to, which is what a rule's parameters
    /// select on. `getHolidayType().getID()`.
    pub fn holiday_type_id(&self) -> i32 {
        self.holiday_type_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_holiday_carries_its_type_and_date() {
        let holiday = Holiday::new(10, 1, LocalDate::of(2010, 12, 25), "Christmas", 3);

        assert_eq!(holiday.id(), 10);
        assert_eq!(holiday.property_id(), 1);
        assert_eq!(holiday.holiday_date(), LocalDate::of(2010, 12, 25));
        assert_eq!(holiday.name(), "Christmas");
        assert_eq!(holiday.holiday_type_id(), 3);
    }
}
