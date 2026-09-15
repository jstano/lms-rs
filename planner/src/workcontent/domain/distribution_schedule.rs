use crate::id_type;
use crate::workcontent::domain::distribution_pattern::DistributionPatternId;
use crate::workcontent::domain::location::LocationId;
use joda_rs::{DayOfWeek, LocalDate};
use std::collections::HashMap;

id_type!(DistributionScheduleId, uuid_v4);

/// Which intraday curve applies on which day of the week.
///
/// Java holds seven discrete `sunFlowPattern..satFlowPattern` fields; a map
/// keyed by day of week says the same thing without the positional bookkeeping.
/// A day with no entry has no curve, and the flowed distributor plans nothing
/// for it rather than falling back to another day's shape.
pub struct DistributionSchedule {
    id: DistributionScheduleId,
    property_id: LocationId,
    name: String,
    code: String,
    pattern_by_day: HashMap<DayOfWeek, DistributionPatternId>,
}

impl DistributionSchedule {
    pub fn new(
        property_id: LocationId,
        name: String,
        code: String,
        pattern_by_day: HashMap<DayOfWeek, DistributionPatternId>,
    ) -> Self {
        Self {
            id: DistributionScheduleId::new(),
            property_id,
            name,
            code,
            pattern_by_day,
        }
    }

    pub fn id(&self) -> DistributionScheduleId {
        self.id
    }

    pub fn property_id(&self) -> LocationId {
        self.property_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    /// The curve to use on `date`, if this schedule covers that day of week.
    pub fn pattern_for_date(&self, date: LocalDate) -> Option<DistributionPatternId> {
        self.pattern_for_day(date.day_of_week())
    }

    pub fn pattern_for_day(&self, day_of_week: DayOfWeek) -> Option<DistributionPatternId> {
        self.pattern_by_day.get(&day_of_week).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule_with(entries: Vec<(DayOfWeek, DistributionPatternId)>) -> DistributionSchedule {
        DistributionSchedule::new(
            LocationId::new(),
            "Standard".to_string(),
            "STD".to_string(),
            entries.into_iter().collect(),
        )
    }

    #[test]
    fn a_date_resolves_to_the_pattern_for_its_day_of_week() {
        let weekday_pattern = DistributionPatternId::new();
        let weekend_pattern = DistributionPatternId::new();
        let schedule = schedule_with(vec![
            (DayOfWeek::Thursday, weekday_pattern),
            (DayOfWeek::Saturday, weekend_pattern),
        ]);

        // 2013-07-25 is a Thursday, 2013-07-27 a Saturday.
        assert_eq!(
            schedule.pattern_for_date(LocalDate::new(2013, 7, 25)),
            Some(weekday_pattern)
        );
        assert_eq!(
            schedule.pattern_for_date(LocalDate::new(2013, 7, 27)),
            Some(weekend_pattern)
        );
    }

    #[test]
    fn a_day_the_schedule_does_not_cover_has_no_pattern() {
        let schedule = schedule_with(vec![(DayOfWeek::Thursday, DistributionPatternId::new())]);

        // 2013-07-26 is a Friday.
        assert_eq!(schedule.pattern_for_date(LocalDate::new(2013, 7, 26)), None);
    }
}
