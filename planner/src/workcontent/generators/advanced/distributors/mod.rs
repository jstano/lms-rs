//! How a standard's work is placed across the day.
//!
//! A spreader knows one shape; a distributor knows which shape a particular
//! standard should take and hands the work to it. Most standards are
//! non-flowed and simply pick a spreader, but flowed work follows a configured
//! curve, opening and closing work is pinned to the edges of the operating
//! window, and shared work is deducted rather than added.

pub mod capacity;
pub mod closing;
pub mod flow_plan;
pub mod non_flowed;
pub mod opening;
pub mod retention;
pub mod share_with;

use crate::workcontent::domain::distribution_method::DistributionMethod;
use crate::workcontent::domain::non_flowed_distribution_method::NonFlowedDistributionMethod;
use crate::workcontent::domain::shift_standard::ShiftStandard;
use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;
use crate::workcontent::generators::advanced::providers::Providers;
use crate::workcontent::generators::error::GenerationError;
use crate::workcontent::generators::advanced::spreaders::WorkSpreader;
use crate::workcontent::generators::advanced::spreaders::beginning::BeginningWorkSpreader;
use crate::workcontent::generators::advanced::spreaders::ending::EndingWorkSpreader;
use crate::workcontent::generators::advanced::spreaders::even::EvenWorkSpreader;
use crate::workcontent::generators::advanced::spreaders::middle::MiddleWorkSpreader;
use crate::workcontent::generators::advanced::spreaders::varying::VaryingWorkSpreader;

pub trait Distributor {
    /// Place `work_minutes` across the day.
    ///
    /// `standard` is the standard the work came from, where the distributor
    /// needs it — the flowed one reads its curve and retention setting from it.
    /// Distributors that do not care take `None`, which is how Java's
    /// single-argument overload calls them.
    fn distribute(
        &self,
        work_minutes: f64,
        standard: Option<&ShiftStandard>,
        params: &GeneratorParameters,
        providers: &Providers,
    ) -> Vec<DistributionItem>;
}

/// The spreader behind a non-flowed distribution method.
pub fn non_flowed_work_spreader(method: NonFlowedDistributionMethod) -> Box<dyn WorkSpreader> {
    match method {
        NonFlowedDistributionMethod::BEGINNING => Box::new(BeginningWorkSpreader::new()),
        NonFlowedDistributionMethod::MIDDLE => Box::new(MiddleWorkSpreader::new()),
        NonFlowedDistributionMethod::END => Box::new(EndingWorkSpreader::new()),
        NonFlowedDistributionMethod::EVEN => Box::new(EvenWorkSpreader::new()),
        NonFlowedDistributionMethod::VARYING => Box::new(VaryingWorkSpreader::new()),
    }
}

/// A distributor that lays work out in the given non-flowed shape.
pub fn non_flowed_distributor(method: NonFlowedDistributionMethod) -> Box<dyn Distributor> {
    Box::new(non_flowed::NonFlowedWorkDistributor::new(
        non_flowed_work_spreader(method),
    ))
}

/// The distributor a standard's work should go through.
///
/// `FillGaps` has no implementation in Java either — it throws — so it is
/// reported rather than silently planning nothing.
pub fn get_distributor(
    standard: &ShiftStandard,
    params: &GeneratorParameters,
) -> Result<Box<dyn Distributor>, GenerationError> {
    Ok(match standard.distribution_method() {
        DistributionMethod::NonFlowed => {
            // A standard with no shape of its own takes the plan's default.
            let method = standard
                .non_flowed_distribution_method()
                .unwrap_or(params.planner_settings().non_flowed_distribution_method);

            non_flowed_distributor(method)
        }
        DistributionMethod::Flowed => Box::new(flow_plan::FlowPlanDistributor::new()),
        DistributionMethod::Opening => Box::new(opening::OpeningWorkDistributor::new()),
        DistributionMethod::Closing => Box::new(closing::ClosingWorkDistributor::new()),
        DistributionMethod::ShareWith => Box::new(share_with::ShareWithDistributor::new()),
        DistributionMethod::FillGaps => return Err(GenerationError::FillGapsNotImplemented),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::distribution_item_list_creator::{
        DistributionItemListCreator, DistributionItemListCreatorImpl,
    };
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};
    use rstest::rstest;

    /// `DistributorFactoryTest."testing the spreader"` — each non-flowed method
    /// maps to its own spreader.
    ///
    /// Rust has no runtime class to compare, so each spreader is identified by
    /// the shape it produces from the same input instead, which is the thing
    /// the mapping actually has to get right.
    #[rstest]
    // Five quarter-hours from the front of an 08:00-16:00 shift.
    #[case(NonFlowedDistributionMethod::BEGINNING, 32, 36)]
    // The same from the back.
    #[case(NonFlowedDistributionMethod::END, 59, 63)]
    // And centred.
    #[case(NonFlowedDistributionMethod::MIDDLE, 45, 49)]
    fn each_method_selects_a_spreader_with_its_own_shape(
        #[case] method: NonFlowedDistributionMethod,
        #[case] first: usize,
        #[case] last: usize,
    ) {
        let context = Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            15,
        );
        let params = context.params();
        let range = params.shift_date_range().expect("the shift has times");
        let mut array = DistributionItemListCreatorImpl::new().create_array(&range);

        non_flowed_work_spreader(method).populate_array_with_work_minutes(
            75.0,
            &params,
            &range,
            &mut array,
        );

        for (index, item) in array.iter().enumerate() {
            let expected = if (first..=last).contains(&index) {
                15.0
            } else {
                0.0
            };
            assert_eq!(item.value_per_period(), expected, "period {index}");
        }
    }

    /// `DistributorFactoryTest."test factory returns supported distribution
    /// methods"` — each method reaches a distributor that behaves distinctly.
    ///
    /// Rust has no class to compare against, so each is identified by what it
    /// does with the same input: non-flowed and flowed both place work (the
    /// flowed one needs a curve, and has none here, so it plans nothing),
    /// opening and closing pin to the shift edges, share-with subtracts.
    #[rstest]
    // Non-flowed places the work inside the shift, at its front.
    #[case(DistributionMethod::NonFlowed, 60.0, Some(16))]
    // Flowed with no curve configured plans nothing.
    #[case(DistributionMethod::Flowed, 0.0, None)]
    // Opening places it before the shift starts...
    #[case(DistributionMethod::Opening, 60.0, Some(14))]
    // ...and closing after it ends.
    #[case(DistributionMethod::Closing, 60.0, Some(32))]
    // Share-with has no schedule, so it deducts nothing.
    #[case(DistributionMethod::ShareWith, 0.0, None)]
    fn each_distribution_method_reaches_its_own_distributor(
        #[case] method: DistributionMethod,
        #[case] expected_total: f64,
        #[case] period_with_work: Option<usize>,
    ) {
        use crate::workcontent::domain::business_driver::BusinessDriverId;
        use crate::workcontent::domain::job::JobId;
        use crate::workcontent::domain::job_shift::JobShiftId;
        use crate::workcontent::domain::shift_standard::ShiftStandardRange;
        use crate::workcontent::domain::standard_set::StandardSetId;
        use crate::workcontent::domain::units::Units;
        use crate::workcontent::domain::work_type::WorkType;

        let context = Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            30,
        )
        .with_max_duration_minutes_for_dynamic_work(120);
        let standard = ShiftStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            BusinessDriverId::new(),
            WorkType::Variable,
            Units::Minutes,
            0,
            vec![ShiftStandardRange::new(0, 1000, 1.0)],
        )
        .distributed_by(method)
        .with_non_flowed_distribution_method(NonFlowedDistributionMethod::BEGINNING);

        let params = context.params();
        let distributor = get_distributor(&standard, &params).expect("a supported method");
        let items = distributor.distribute(60.0, Some(&standard), &params, &Providers::none());

        let total: f64 = items.iter().map(|item| item.value_per_period()).sum();
        assert_eq!(total, expected_total);

        if let Some(index) = period_with_work {
            assert!(
                items[index].value_per_period() > 0.0,
                "expected work in period {index}"
            );
        }
    }

    #[test]
    fn filling_gaps_is_reported_rather_than_silently_skipped() {
        use crate::workcontent::domain::business_driver::BusinessDriverId;
        use crate::workcontent::domain::job::JobId;
        use crate::workcontent::domain::job_shift::JobShiftId;
        use crate::workcontent::domain::shift_standard::ShiftStandardRange;
        use crate::workcontent::domain::standard_set::StandardSetId;
        use crate::workcontent::domain::units::Units;
        use crate::workcontent::domain::work_type::WorkType;

        let context = Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            30,
        );
        let standard = ShiftStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            BusinessDriverId::new(),
            WorkType::Variable,
            Units::Minutes,
            0,
            vec![ShiftStandardRange::new(0, 1000, 1.0)],
        )
        .distributed_by(DistributionMethod::FillGaps);

        assert!(matches!(
            get_distributor(&standard, &context.params()),
            Err(GenerationError::FillGapsNotImplemented)
        ));
    }

    #[test]
    fn a_standard_with_no_shape_of_its_own_takes_the_plans_default() {
        use crate::workcontent::domain::business_driver::BusinessDriverId;
        use crate::workcontent::domain::job::JobId;
        use crate::workcontent::domain::job_shift::JobShiftId;
        use crate::workcontent::domain::shift_standard::ShiftStandardRange;
        use crate::workcontent::domain::standard_set::StandardSetId;
        use crate::workcontent::domain::units::Units;
        use crate::workcontent::domain::work_type::WorkType;

        let mut context = Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            30,
        );
        context.planner_settings.non_flowed_distribution_method =
            NonFlowedDistributionMethod::END;

        let standard = ShiftStandard::new(
            JobId::new(),
            StandardSetId::new(),
            JobShiftId::new(),
            BusinessDriverId::new(),
            WorkType::Variable,
            Units::Minutes,
            0,
            vec![ShiftStandardRange::new(0, 1000, 1.0)],
        );

        let params = context.params();
        let items = get_distributor(&standard, &params)
            .expect("non-flowed is supported")
            .distribute(60.0, Some(&standard), &params, &Providers::none());

        // The plan's default is END, so the work sits at the back of the shift.
        assert_eq!(items[31].value_per_period(), 30.0);
        assert_eq!(items[16].value_per_period(), 0.0);
    }

    #[test]
    fn the_even_method_spreads_flat_rather_than_bunching() {
        let context = Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            15,
        );
        let params = context.params();
        let range = params.shift_date_range().expect("the shift has times");
        let mut array = DistributionItemListCreatorImpl::new().create_array(&range);

        non_flowed_work_spreader(NonFlowedDistributionMethod::EVEN)
            .populate_array_with_work_minutes(64.0, &params, &range, &mut array);

        // Thirty-two periods share the work equally.
        assert_eq!(array[32].value_per_period(), 2.0);
        assert_eq!(array[63].value_per_period(), 2.0);
    }

    #[test]
    fn the_varying_method_bounces_off_the_end() {
        let context = Context::new(
            LocalDate::new(2013, 7, 24),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            15,
        );
        let params = context.params();
        let range = params.shift_date_range().expect("the shift has times");
        let mut array = DistributionItemListCreatorImpl::new().create_array(&range);

        non_flowed_work_spreader(NonFlowedDistributionMethod::VARYING)
            .populate_array_with_work_minutes(510.0, &params, &range, &mut array);

        // The turn at the end doubles up the last two periods.
        assert_eq!(array[62].value_per_period(), 30.0);
        assert_eq!(array[63].value_per_period(), 30.0);
    }
}
