/// How a standard's work is placed across the periods of a shift.
///
/// `NonFlowed` defers to the standard's
/// [`NonFlowedDistributionMethod`](super::non_flowed_distribution_method::NonFlowedDistributionMethod)
/// for the shape of the curve; the rest each have their own distributor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionMethod {
    /// Follow an intraday percentage curve from a distribution pattern.
    Flowed,
    /// Backfill periods left under-covered by other work.
    FillGaps,
    /// Place the work in whole blocks rather than spreading it.
    NonFlowed,
    /// Work that must happen at the start of the operating window.
    Opening,
    /// Work that must happen at the end of the operating window.
    Closing,
    /// Work covered jointly with another schedule, and so deducted here.
    ShareWith,
}
