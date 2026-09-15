//! Sums several distribution arrays into one.
//!
//! Each standard is distributed independently, producing its own array, and the
//! shift's total work is their sum. Adding by index is safe because every
//! producer builds against the same canonical two-day grid, so index `n` means
//! the same half hour in all of them.

use crate::workcontent::generators::advanced::distribution_item::DistributionItem;
use crate::workcontent::generators::advanced::distribution_item_list_creator::{
    DistributionItemListCreator, DistributionItemListCreatorImpl,
};
use crate::workcontent::generators::advanced::generator_parameters::GeneratorParameters;

pub trait DistributionAggregator {
    fn aggregate(
        &self,
        params: &GeneratorParameters,
        arrays: &[&[DistributionItem]],
    ) -> Vec<DistributionItem>;
}

pub struct DistributionAggregatorImpl {
    list_creator: Box<dyn DistributionItemListCreator>,
}

impl DistributionAggregatorImpl {
    pub fn new() -> Self {
        Self {
            list_creator: Box::new(DistributionItemListCreatorImpl::new()),
        }
    }
}

impl DistributionAggregator for DistributionAggregatorImpl {
    fn aggregate(
        &self,
        params: &GeneratorParameters,
        arrays: &[&[DistributionItem]],
    ) -> Vec<DistributionItem> {
        let mut totals = self.list_creator.create_array_for(params);

        for array in arrays {
            for (index, item) in array.iter().enumerate() {
                // An input longer than the canonical grid has nothing to add to.
                let Some(total) = totals.get_mut(index) else {
                    break;
                };

                total.add_to_period_value(item.value_per_period());
            }
        }

        totals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workcontent::generators::advanced::generator_parameters::fixtures::Context;
    use joda_rs::{LocalDate, LocalTime};

    fn context() -> Context {
        Context::new(
            LocalDate::new(2013, 8, 26),
            LocalTime::new(8, 0, 0),
            LocalTime::new(16, 0, 0),
            30,
        )
    }

    /// An array with `values` laid in from `start_index`.
    fn array_from(context: &Context, start_index: usize, values: &[f64]) -> Vec<DistributionItem> {
        let params = context.params();
        let mut array = DistributionItemListCreatorImpl::new().create_array_for(&params);

        for (offset, value) in values.iter().enumerate() {
            array[start_index + offset].set_value_per_period(*value);
        }

        array
    }

    fn total_of(items: &[DistributionItem]) -> f64 {
        items.iter().map(|item| item.value_per_period()).sum()
    }

    #[test]
    fn aggregating_nothing_gives_an_empty_array() {
        let context = context();
        let params = context.params();

        let totals = DistributionAggregatorImpl::new().aggregate(&params, &[]);

        assert_eq!(totals.len(), 96);
        assert_eq!(total_of(&totals), 0.0);
    }

    #[test]
    fn a_single_array_comes_back_unchanged() {
        let context = context();
        let params = context.params();
        let array = array_from(&context, 16, &[10.0, 20.0, 30.0]);

        let totals = DistributionAggregatorImpl::new().aggregate(&params, &[&array]);

        assert_eq!(totals[16].value_per_period(), 10.0);
        assert_eq!(totals[17].value_per_period(), 20.0);
        assert_eq!(totals[18].value_per_period(), 30.0);
        assert_eq!(total_of(&totals), 60.0);
    }

    #[test]
    fn overlapping_arrays_are_summed_period_by_period() {
        let context = context();
        let params = context.params();
        let first = array_from(&context, 16, &[10.0, 20.0, 30.0]);
        let second = array_from(&context, 16, &[1.0, 2.0, 3.0]);

        let totals = DistributionAggregatorImpl::new().aggregate(&params, &[&first, &second]);

        assert_eq!(totals[16].value_per_period(), 11.0);
        assert_eq!(totals[17].value_per_period(), 22.0);
        assert_eq!(totals[18].value_per_period(), 33.0);
        assert_eq!(total_of(&totals), 66.0);
    }

    #[test]
    fn arrays_covering_different_times_keep_their_own_periods() {
        let context = context();
        let params = context.params();
        // A morning block and an evening one, with a gap between them.
        let morning = array_from(&context, 16, &[10.0, 10.0]);
        let evening = array_from(&context, 40, &[5.0, 5.0]);

        let totals = DistributionAggregatorImpl::new().aggregate(&params, &[&morning, &evening]);

        assert_eq!(totals[16].value_per_period(), 10.0);
        assert_eq!(totals[17].value_per_period(), 10.0);
        // The gap stays empty.
        assert_eq!(totals[18].value_per_period(), 0.0);
        assert_eq!(totals[39].value_per_period(), 0.0);
        assert_eq!(totals[40].value_per_period(), 5.0);
        assert_eq!(totals[41].value_per_period(), 5.0);
        assert_eq!(total_of(&totals), 30.0);
    }

    #[test]
    fn four_arrays_are_summed_together() {
        let context = context();
        let params = context.params();
        let arrays: Vec<_> = (1..=4)
            .map(|multiple| array_from(&context, 16, &[multiple as f64, multiple as f64]))
            .collect();
        let borrowed: Vec<&[DistributionItem]> =
            arrays.iter().map(|array| array.as_slice()).collect();

        let totals = DistributionAggregatorImpl::new().aggregate(&params, &borrowed);

        assert_eq!(totals[16].value_per_period(), 10.0);
        assert_eq!(totals[17].value_per_period(), 10.0);
        assert_eq!(total_of(&totals), 20.0);
    }

    #[test]
    fn fractional_values_accumulate_without_drifting() {
        let context = context();
        let params = context.params();
        // A third of an hour, added three times, must come back to one.
        let third = array_from(&context, 16, &[1.0 / 3.0]);

        let totals = DistributionAggregatorImpl::new().aggregate(&params, &[&third, &third, &third]);

        assert_eq!(numbers_round(total_of(&totals)), 1.0);
        // Each add is rounded, so the stored value is the rounded sum rather
        // than the exact one.
        assert_eq!(totals[16].value_per_period(), 0.9999);
    }

    fn numbers_round(value: f64) -> f64 {
        crate::workcontent::common::numbers::round(value) as f64
    }
}
