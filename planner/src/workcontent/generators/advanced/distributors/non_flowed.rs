//! Places work in a fixed shape across the shift.
//!
//! The simplest distributor: build the day's array and hand it to a spreader.
//! Which spreader is decided when the distributor is built, from the standard's
//! non-flowed distribution method.

use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::distributors::Distributor;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::advanced::spreaders::WorkSpreader;

pub struct NonFlowedWorkDistributor {
    work_spreader: Box<dyn WorkSpreader>,
    list_creator: Box<dyn DistributionItemListCreator>,
}

impl NonFlowedWorkDistributor {
    pub fn new(work_spreader: Box<dyn WorkSpreader>) -> Self {
        Self {
            work_spreader,
            list_creator: Box::new(DistributionItemListCreatorImpl::new()),
        }
    }
}

impl Distributor for NonFlowedWorkDistributor {
    fn distribute(
        &self,
        work_minutes: f64,
        _standard: Option<&ShiftStandard>,
        params: &GeneratorParameters,
        _providers: &Providers,
    ) -> Vec<DistributionItem> {
        let mut array = self.list_creator.create_array_for(params);

        let Some(range) = params.shift_date_range() else {
            return array;
        };

        self.work_spreader
            .populate_array_with_work_minutes(work_minutes, params, &range, &mut array);

        array
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
    use crate::workcontent::generators::advanced::distributors::non_flowed_work_spreader;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};

    fn context(period_length: u32) -> Context {
        Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            period_length,
        )
    }

    /// `NonFlowedWorkDistributorTest` — Java mocks both collaborators and only
    /// checks they were called. Here both are real, so the test can assert the
    /// work actually landed where the chosen spreader would put it.
    #[test]
    fn the_work_is_laid_out_by_the_spreader_it_was_built_with() {
        let context = context(15);
        let distributor =
            NonFlowedWorkDistributor::new(non_flowed_work_spreader(
                NonFlowedDistributionMethod::BEGINNING,
            ));

        let array = distributor.distribute(
            100.0,
            None,
            &context.params(),
            &Providers::none(),
        );

        // A full two-day array, with the work at the front of the shift:
        // six full quarter-hours and then the ten minutes left over.
        assert_eq!(array.len(), 192);
        assert_eq!(array[32].value_per_period(), 15.0);
        assert_eq!(array[37].value_per_period(), 15.0);
        assert_eq!(array[38].value_per_period(), 10.0);
        assert_eq!(array[39].value_per_period(), 0.0);
    }

    #[test]
    fn a_shift_without_times_distributes_nothing() {
        let mut context = context(15);
        context.shift_detail =
            crate::workcontent::domain::job_shift::JobShiftDefinition::without_times(
                joda_rs::DayOfWeek::Monday,
            );
        let distributor =
            NonFlowedWorkDistributor::new(non_flowed_work_spreader(
                NonFlowedDistributionMethod::BEGINNING,
            ));

        let array = distributor.distribute(
            100.0,
            None,
            &context.params(),
            &Providers::none(),
        );

        assert!(array.iter().all(|item| item.value_per_period() == 0.0));
    }
}
