//! Port of `com.unifocus.watson.server.scheduler.engine.io.ScheduleModelCreator`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/scheduler/engine/io/
//! ScheduleModelCreator.java`. `progress` isn't threaded through — see `ScheduleModel`'s own doc
//! for why the type doesn't carry it yet.

use crate::engine::generate_schedules_parameters::GenerateSchedulesParameters;
use crate::engine::io::ports::PropertyPort;
use crate::engine::model::schedule_model::ScheduleModel;

/// `ScheduleModelCreator`.
pub struct ScheduleModelCreator<'a> {
    properties: &'a dyn PropertyPort,
}

impl<'a> ScheduleModelCreator<'a> {
    pub fn new(properties: &'a dyn PropertyPort) -> Self {
        Self { properties }
    }

    /// `createScheduleModel(GenerateSchedulesParameters, Progress)` — `None` where Java's
    /// `propertyDAO.findByID` would return (or Hibernate would throw for) a missing property;
    /// nothing upstream is known to pass an id that doesn't resolve, but `findByID` is not
    /// guaranteed non-null by its own signature.
    pub fn create_schedule_model(
        &self,
        params: &GenerateSchedulesParameters,
    ) -> Option<ScheduleModel> {
        let property = self.properties.find_by_id(params.property_id())?;

        Some(ScheduleModel::new(property, *params.date_range()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::property::Property;
    use date_range_rs::DateRange;
    use joda_rs::LocalDate;

    struct FakePropertyPort {
        property: Option<Property>,
    }

    impl PropertyPort for FakePropertyPort {
        fn find_by_id(&self, _id: i32) -> Option<Property> {
            self.property.clone()
        }
    }

    fn params() -> GenerateSchedulesParameters {
        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        GenerateSchedulesParameters::new(range, 1)
    }

    #[test]
    fn builds_a_schedule_model_from_the_resolved_property_and_the_parameters_date_range() {
        let range = DateRange::new(LocalDate::of(2024, 1, 1), LocalDate::of(2024, 1, 7));
        let property = Property::new(1, range);
        let port = FakePropertyPort {
            property: Some(property.clone()),
        };
        let creator = ScheduleModelCreator::new(&port);

        let schedule_model = creator.create_schedule_model(&params()).unwrap();

        assert_eq!(*schedule_model.property(), property);
        assert_eq!(*schedule_model.date_range(), range);
    }

    #[test]
    fn returns_none_when_the_property_does_not_resolve() {
        let port = FakePropertyPort { property: None };
        let creator = ScheduleModelCreator::new(&port);

        assert!(creator.create_schedule_model(&params()).is_none());
    }
}
