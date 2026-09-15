use crate::workcontent::domain::business_driver::BusinessDriverId;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// The per-date volumes recorded against one business driver.
pub struct BusinessDriverValues {
    business_driver_id: BusinessDriverId,
    values: HashMap<LocalDate, i32>,
}

impl BusinessDriverValues {
    pub fn new(business_driver_id: BusinessDriverId, values: HashMap<LocalDate, i32>) -> Self {
        Self {
            business_driver_id,
            values,
        }
    }

    pub fn business_driver_id(&self) -> BusinessDriverId {
        self.business_driver_id
    }

    /// The volume on `date`, or zero when nothing was recorded for it.
    pub fn value_for_date(&self, date: LocalDate) -> i32 {
        self.values.get(&date).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recorded_date_reads_back_its_volume() {
        let date = LocalDate::new(2025, 10, 6);
        let values = BusinessDriverValues::new(
            BusinessDriverId::new(),
            HashMap::from([(date, 250)]),
        );

        assert_eq!(values.value_for_date(date), 250);
    }

    #[test]
    fn an_unrecorded_date_reads_back_zero() {
        let values = BusinessDriverValues::new(BusinessDriverId::new(), HashMap::new());

        assert_eq!(values.value_for_date(LocalDate::new(2025, 10, 6)), 0);
    }
}
