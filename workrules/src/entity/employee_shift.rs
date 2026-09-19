//! Port of `com.unifocus.watson.server.hibernate.entity.EmployeeShift`, plus
//! the parts of `com.unifocus.watson.server.labor.ShiftUtil` and
//! `PunchTimeComparator` that its own methods depend on.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/hibernate/entity/EmployeeShift.java`
//! and `taps/src/java/com/unifocus/watson/server/labor/ShiftUtil.java`.
//!
//! The Java entity is 1,202 lines; rules read about twenty fields of it. Only
//! those come across.
//!
//! # The shift is the aggregate
//!
//! In Java a punch holds a back-reference to its shift, and
//! `EmployeeShiftPunch.setRoundedTime` calls
//! `shift.resetStartAndEndTimesFromPunch(this)` whenever the value changes,
//! which resets the shift's start or end and reruns `calcWorkedHours()`.
//!
//! That cycle cannot be expressed with `&mut` — but it is load-bearing, and not
//! only for the obvious reason. `WorkedHoursRoundingRuleImpl` depends on it
//! directly:
//!
//! ```java
//! final EmployeeShift shift = punch.getEmployeeShift();
//! for (EmployeeShiftPunch otherPunch : shift.getPunches()) {
//!    otherPunch.setRoundedTime(otherPunch.getAdjTime());   // each fires the callback
//! }
//! double workedHours = shift.getWorkedHours();             // correct only because of them
//! ```
//!
//! It mutates every sibling punch and then reads shift state that those
//! mutations recomputed. So the shift owns the punches, owns the callback, and
//! hands rules a [`PunchCursor`] — a `&mut EmployeeShift` plus an index — whose
//! `set_rounded_time` performs both halves. Rule bodies then read almost
//! exactly as the Java does, which is what keeps them reviewable against it.
//!
//! Five of the six punch-rounding rules reach the shift this way, so this is
//! the normal case, not an exception.

use crate::common::enums::punch_type::PunchType;
use crate::common::enums::shift_error_type::ShiftErrorType;
use crate::common::enums::shift_type::ShiftType;
use crate::common::numbers::{round_hours, round_raw_hours};
use crate::entity::employee_shift_punch::EmployeeShiftPunch;
use crate::entity::hours_distribution::HoursDistribution;
use date_range_rs::DateRange;
use date_range_rs::datetimerange::date_time_range::DateTimeRange;
use joda_rs::LocalDate;
use joda_rs::LocalDateTime;
use std::cmp::Ordering;
use std::collections::HashSet;

const MINUTES_PER_HOUR: f64 = 60.0;
const SECONDS_PER_MINUTE: i32 = 60;

/// A worked, scheduled or generated shift. `EmployeeShift`.
#[derive(Debug, Clone, PartialEq)]
pub struct EmployeeShift {
    id: i32,
    employee_id: i32,
    job_id: i32,
    property_id: i32,
    shift_date: LocalDate,
    shift_type: ShiftType,
    start_date_time: Option<LocalDateTime>,
    end_date_time: Option<LocalDateTime>,
    punches: Vec<EmployeeShiftPunch>,
    hours_distributions: Vec<HoursDistribution>,
    worked_hours: f64,
    net_hours: f64,
    adj_hours: f64,
    reg_rate: f64,
    shift_category_id: Option<i32>,
    /// Stands in for `workedAdjustmentsEmpty()` until the adjustments entity
    /// is ported. `true` matches a shift with no worked adjustments, which is
    /// every shift the punch-rounding tests build.
    worked_adjustments_empty: bool,
    /// The *persisted* errors, as last saved. Distinct from
    /// [`derived_errors`](EmployeeShift::derived_errors).
    errors: Vec<ShiftErrorType>,
}

impl EmployeeShift {
    /// Build a shift from its punches.
    pub fn new(
        id: i32,
        employee_id: i32,
        job_id: i32,
        shift_date: LocalDate,
        shift_type: ShiftType,
        punches: Vec<EmployeeShiftPunch>,
    ) -> Self {
        Self {
            id,
            employee_id,
            job_id,
            property_id: 0,
            shift_date,
            shift_type,
            start_date_time: None,
            end_date_time: None,
            punches,
            hours_distributions: Vec::new(),
            worked_hours: 0.0,
            net_hours: 0.0,
            adj_hours: 0.0,
            reg_rate: 0.0,
            shift_category_id: None,
            worked_adjustments_empty: true,
            errors: Vec::new(),
        }
    }

    /// Set the persisted errors, as loading from the database would.
    #[must_use]
    pub fn with_errors(mut self, errors: Vec<ShiftErrorType>) -> Self {
        self.errors = errors;
        self
    }

    /// Set the start and end directly, as loading from the database would.
    #[must_use]
    pub fn with_times(mut self, start: Option<LocalDateTime>, end: Option<LocalDateTime>) -> Self {
        self.start_date_time = start;
        self.end_date_time = end;
        self
    }

    /// Mark the shift as carrying worked adjustments rather than punches.
    ///
    /// The adjustments entity is not ported; `workedAdjustmentsEmpty()` is a
    /// `bool` field here, so this is how a test builds the adjustment-only
    /// shape [`is_adjustment_only_shift`](Self::is_adjustment_only_shift) tests
    /// for.
    #[must_use]
    pub fn with_worked_adjustments(mut self) -> Self {
        self.worked_adjustments_empty = false;
        self
    }

    /// Hours recorded as an adjustment with no punches behind them.
    /// `isAdjustmentOnlyShift()`.
    ///
    /// Java is `punches.isEmpty() && !workedAdjustmentsEmpty()`. Note this is
    /// **not** `TwentyFourHourOTRuleImpl`'s `isShiftAdjustmentOnly`, which asks
    /// `!hasErrors() && !hasBothTimes()` — two rules, two definitions of the
    /// same phrase, and a shift with no punches but no adjustment either
    /// answers differently to each.
    pub fn is_adjustment_only_shift(&self) -> bool {
        self.punches.is_empty() && !self.worked_adjustments_empty
    }

    /// Set the job. `setJob()`.
    #[must_use]
    pub fn with_job_id(mut self, job_id: i32) -> Self {
        self.job_id = job_id;
        self
    }

    /// Set whether this is an actual or a scheduled shift. `setShiftType()`.
    #[must_use]
    pub fn with_shift_type(mut self, shift_type: ShiftType) -> Self {
        self.shift_type = shift_type;
        self
    }

    /// Add a punch, as `shift.punches << punch` does in the Groovy tables.
    #[must_use]
    pub fn with_punch(mut self, punch: EmployeeShiftPunch) -> Self {
        self.punches.push(punch);
        self
    }

    /// Set the adjustment hours that `calcWorkedHours` folds into net hours.
    #[must_use]
    pub fn with_adj_hours(mut self, adj_hours: f64) -> Self {
        self.adj_hours = adj_hours;
        self
    }

    /// Set the persisted regular rate, as loading from the database would.
    #[must_use]
    pub fn with_reg_rate(mut self, reg_rate: f64) -> Self {
        self.reg_rate = reg_rate;
        self
    }

    /// Tag the shift with a shift category. `setShiftCategory()`.
    #[must_use]
    pub fn with_shift_category_id(mut self, shift_category_id: Option<i32>) -> Self {
        self.shift_category_id = shift_category_id;
        self
    }

    /// `getID()`.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// `getEmployee().getID()`.
    pub fn employee_id(&self) -> i32 {
        self.employee_id
    }

    /// `getJob().getID()`.
    pub fn job_id(&self) -> i32 {
        self.job_id
    }

    /// `getProperty().getID()`.
    ///
    /// Java reaches the property two ways — `shift.getProperty()` directly, and
    /// `shift.getJob().getProperty()`, which is the chain
    /// `PropertyDataRoundingRuleImpl` walks. Both land on the same site, so the
    /// shift carries the id and neither navigation needs the job entity.
    pub fn property_id(&self) -> i32 {
        self.property_id
    }

    /// Set the property. Defaults to 0, which no real site uses.
    #[must_use]
    pub fn with_property_id(mut self, property_id: i32) -> Self {
        self.property_id = property_id;
        self
    }

    /// `getShiftDate()`.
    pub fn shift_date(&self) -> LocalDate {
        self.shift_date
    }

    /// `setShiftDate()`.
    ///
    /// The punch-rounding runner moves this when rounding pushes the in punch
    /// across midnight.
    pub fn set_shift_date(&mut self, shift_date: LocalDate) {
        self.shift_date = shift_date;
    }

    /// `getShiftType()`.
    pub fn shift_type(&self) -> ShiftType {
        self.shift_type
    }

    /// `getStartDateTime()`.
    pub fn start_date_time(&self) -> Option<LocalDateTime> {
        self.start_date_time
    }

    /// `getEndDateTime()`.
    pub fn end_date_time(&self) -> Option<LocalDateTime> {
        self.end_date_time
    }

    /// Whether the shift has both a start and an end. `hasBothTimes()`.
    pub fn has_both_times(&self) -> bool {
        self.start_date_time.is_some() && self.end_date_time.is_some()
    }

    /// Set the persisted net hours, as loading from the database would.
    ///
    /// `calcWorkedHours` recomputes this from the punches; Java also has a
    /// plain `setNetHours` for the column, which is what the rule specs use to
    /// build a shift of a given length without punches.
    #[must_use]
    pub fn with_net_hours(mut self, net_hours: f64) -> Self {
        self.net_hours = net_hours;
        self
    }

    /// Set the persisted worked hours directly, the same way
    /// [`with_net_hours`](Self::with_net_hours) sets net hours — for a spec
    /// that fixes `workedHours` on the fixture rather than deriving it from
    /// punches.
    #[must_use]
    pub fn with_worked_hours(mut self, worked_hours: f64) -> Self {
        self.worked_hours = worked_hours;
        self
    }

    /// The instant `ShiftStartTimeComparator` orders shifts by: the start time
    /// if the shift has one, and otherwise **midnight on its shift date**.
    ///
    /// Ground truth: `taps/src/java/com/unifocus/watson/server/labor/ShiftStartTimeComparator.java`.
    /// A one-method comparator in Java; here it is the key it sorts on, so a
    /// caller can `sort_by_key` and get the same order. Two shifts on the same
    /// date that both lack a start time compare **equal**, so whatever order
    /// they arrive in survives a stable sort — which several rules depend on
    /// without saying so.
    pub fn start_time_for_ordering(&self) -> LocalDateTime {
        self.start_date_time
            .unwrap_or_else(|| self.shift_date.at_start_of_day())
    }

    /// `getWorkedHours()` — rounded to 4 places by `calcWorkedHours`.
    pub fn worked_hours(&self) -> f64 {
        self.worked_hours
    }

    /// `getNetHours()` — worked hours plus adjustments, rounded to 2 places.
    pub fn net_hours(&self) -> f64 {
        self.net_hours
    }

    /// `getAdjHours()`.
    pub fn adj_hours(&self) -> f64 {
        self.adj_hours
    }

    /// The regular rate `RegularRate` family rules write here so
    /// `FLSAOTRateRuleImpl`/`WeightedOTRateRuleImpl` can read it back.
    /// `getRegRate()`.
    pub fn reg_rate(&self) -> f64 {
        self.reg_rate
    }

    /// `setRegRate()`.
    pub fn set_reg_rate(&mut self, reg_rate: f64) {
        self.reg_rate = reg_rate;
    }

    /// The configured tag this shift carries, if any. `getShiftCategory().getID()`.
    pub fn shift_category_id(&self) -> Option<i32> {
        self.shift_category_id
    }

    /// How this shift's hours break down. `getHoursDistributions()`.
    ///
    /// Empty until the distribution families have run — the punch families
    /// never populate it.
    pub fn hours_distributions(&self) -> &[HoursDistribution] {
        &self.hours_distributions
    }

    /// The distributions, for a rule that is rewriting them.
    pub fn hours_distributions_mut(&mut self) -> &mut Vec<HoursDistribution> {
        &mut self.hours_distributions
    }

    /// Append one distribution. `addHoursDistribution()`.
    pub fn add_hours_distribution(&mut self, distribution: HoursDistribution) {
        self.hours_distributions.push(distribution);
    }

    /// Append several distributions. `addHoursDistributions()`.
    pub fn add_hours_distributions(&mut self, distributions: Vec<HoursDistribution>) {
        self.hours_distributions.extend(distributions);
    }

    /// Drop every distribution. `clearHoursDistributions()`.
    pub fn clear_hours_distributions(&mut self) {
        self.hours_distributions.clear();
    }

    /// The distinct dates this shift has distributions on, in date order.
    /// `getDatesWithHoursDistributions()`.
    ///
    /// Java returns a `HashSet`; this returns a sorted `Vec` so a caller that
    /// iterates gets a defined order rather than the hash one.
    pub fn dates_with_hours_distributions(&self) -> Vec<LocalDate> {
        let mut dates: Vec<LocalDate> = self
            .hours_distributions
            .iter()
            .map(HoursDistribution::date)
            .collect();
        dates.sort_unstable();
        dates.dedup();
        dates
    }

    /// Whether any distribution on this shift falls inside `period`.
    ///
    /// `ActualsTimeCardShiftMethods.shiftHasDistributionWhichFallsWithinPeriod`,
    /// which is the filter behind both
    /// [`shifts_with_distributions_for_period`](crate::entity::time_card::TimeCard::shifts_with_distributions_for_period)
    /// and its schedules twin.
    pub fn has_distribution_within_period(&self, period: &DateRange) -> bool {
        self.hours_distributions
            .iter()
            .any(|distribution| distribution.falls_within_period(period))
    }

    /// Attach hours distributions, as loading from the database would.
    #[must_use]
    pub fn with_hours_distributions(mut self, distributions: Vec<HoursDistribution>) -> Self {
        self.hours_distributions = distributions;
        self
    }

    /// The punches, in the order they are stored. `getPunches()`.
    pub fn punches(&self) -> &[EmployeeShiftPunch] {
        &self.punches
    }

    /// One punch by position.
    pub fn punch(&self, index: usize) -> &EmployeeShiftPunch {
        &self.punches[index]
    }

    /// How many punches this shift has.
    pub fn punch_count(&self) -> usize {
        self.punches.len()
    }

    /// A cursor onto one punch, through which writes fire the shift callback.
    ///
    /// # Panics
    ///
    /// If `index` is out of range.
    pub fn punch_cursor(&mut self, index: usize) -> PunchCursor<'_> {
        assert!(
            index < self.punches.len(),
            "punch index {index} out of range for a shift with {} punches",
            self.punches.len()
        );
        PunchCursor { shift: self, index }
    }

    /// The punch indices in the order `PunchTimeComparator` puts them.
    ///
    /// Java sorts a copy of the punch list; sorting indices says the same thing
    /// while keeping the identity comparisons in
    /// [`errors`](Self::errors) — which are reference comparisons in Java —
    /// expressible as index comparisons.
    pub(crate) fn punch_order_for_breaks(&self) -> Vec<usize> {
        self.punch_order()
    }

    fn punch_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.punches.len()).collect();
        order.sort_by(|a, b| compare_punch_times(&self.punches[*a], &self.punches[*b]));
        order
    }

    /// Net hours worked inside a window, with the shift's adjustment applied.
    /// `ShiftUtil.getNetHoursInRange`.
    ///
    /// The shift's punches are paired into worked ranges, the adjustment hours
    /// are added to or taken off the **end** of that list, and what overlaps
    /// `range` is summed. `TwentyFourHourOTRuleImpl` is the caller: it measures
    /// a shift against a rolling 24-hour work day, which no field on the shift
    /// can answer.
    ///
    /// Zero when the shift does not overlap the window, and zero when it has no
    /// start or end time — Java dereferences both unguarded in
    /// `shiftOverlapsDateTimeRange`, and an adjustment-only shift has neither.
    pub fn net_hours_in_range(&self, range: &DateTimeRange) -> f64 {
        if !self.overlaps_date_time_range(range) {
            return 0.0;
        }

        self.altered_worked_date_time_ranges()
            .iter()
            .map(|worked| range.overlap_duration(worked).fractional_hours())
            .sum()
    }

    /// `shiftOverlapsDateTimeRange`.
    fn overlaps_date_time_range(&self, range: &DateTimeRange) -> bool {
        match (self.start_date_time, self.end_date_time) {
            (Some(start), Some(end)) => start <= range.end() && end >= range.start(),
            _ => false,
        }
    }

    /// `getAlteredWorkedDateTimeRangesForShiftWithAdjustments`.
    ///
    /// A positive adjustment lengthens the last worked range; a negative one
    /// eats back from the end, dropping whole ranges until it is spent. Exactly
    /// zero takes the negative branch, whose loop does not run.
    fn altered_worked_date_time_ranges(&self) -> Vec<DateTimeRange> {
        let mut ranges = self.worked_date_time_ranges();

        if self.adj_hours > 0.0 {
            alter_last_punch_time(&mut ranges, self.adj_hours);
        } else {
            alter_punches_from_end(&mut ranges, self.adj_hours);
        }

        ranges
    }

    /// `getWorkedDateTimeRangesFromShift` — punches paired **positionally** in
    /// `PunchTimeComparator` order, 0 with 1, 2 with 3, and so on.
    ///
    /// Not by punch type, exactly as `ShiftUtil.getBreaks` does it. Java sorts
    /// the shift's live punch list in place here; sorting indices says the same
    /// thing without the side effect.
    ///
    /// **Divergence:** an odd number of punches throws
    /// `IndexOutOfBoundsException` in Java, because the loop steps by two and
    /// reads `get(i + 1)` unguarded. The unpaired trailing punch is dropped
    /// here — a shift that is still clocked in contributes the time it has
    /// closed rather than failing the calculation, which is the reading
    /// divergences 20, 32 and 37 took.
    fn worked_date_time_ranges(&self) -> Vec<DateTimeRange> {
        let order = self.punch_order();

        order
            .chunks_exact(2)
            .filter_map(|pair| {
                let start = self.punches[pair[0]].rounded_time()?;
                let end = self.punches[pair[1]].rounded_time()?;
                Some(DateTimeRange::of(start, end))
            })
            .collect()
    }

    /// The errors as last saved. `getErrors()`.
    ///
    /// **Not** the same as [`derived_errors`](Self::derived_errors), and the
    /// difference is load-bearing: `calcWorkedHours` deliberately consults the
    /// derived set ("we may have not updated the saved errors yet"), while
    /// `hasErrors()` and `ShiftUtil.getBreaks` consult this one. Conflating them
    /// changes which shifts get their hours zeroed.
    pub fn errors(&self) -> &[ShiftErrorType] {
        &self.errors
    }

    /// Does the shift have saved errors? `hasErrors()`.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Does the shift have this saved error? `hasError(ShiftErrorType)`.
    pub fn has_error(&self, error_type: ShiftErrorType) -> bool {
        self.errors.contains(&error_type)
    }

    /// Recompute the shift's errors from its punches. `getErrorsOld()`.
    pub fn derived_errors(&self) -> HashSet<ShiftErrorType> {
        let mut errors = HashSet::new();

        if self.punches.is_empty()
            && self.worked_adjustments_empty
            && self.shift_type == ShiftType::Actual
        {
            errors.insert(ShiftErrorType::EmptyShift);
            return errors;
        }

        if self.punches.is_empty() {
            return errors;
        }

        let mut in_punch: Option<usize> = None;
        let mut out_punch: Option<usize> = None;

        for (index, punch) in self.punches.iter().enumerate() {
            match punch.punch_type() {
                PunchType::In => {
                    in_punch = Some(index);
                    if punch.rounded_time().is_none() {
                        errors.insert(ShiftErrorType::MissingIn);
                    }
                }
                PunchType::Out => {
                    out_punch = Some(index);
                    if punch.rounded_time().is_none() {
                        errors.insert(ShiftErrorType::MissingOut);
                    }
                }
                PunchType::Break if punch.rounded_time().is_none() => {
                    errors.insert(ShiftErrorType::MissingBreakOut);
                }
                PunchType::Back if punch.rounded_time().is_none() => {
                    errors.insert(ShiftErrorType::MissingBreakIn);
                }
                _ => {}
            }
        }

        let order = self.punch_order();

        // Java compares object references here; comparing indices is the same
        // question, and stays correct when two punches are equal by value.
        match in_punch {
            None => {
                errors.insert(ShiftErrorType::MissingIn);
            }
            Some(index) if index != order[0] => {
                errors.insert(ShiftErrorType::InvalidTimes);
            }
            Some(_) => {}
        }

        if out_punch.is_none() {
            errors.insert(ShiftErrorType::MissingOut);
        }
        if out_punch != order.last().copied() {
            errors.insert(ShiftErrorType::InvalidTimes);
        }

        if let (Some(i), Some(o)) = (in_punch, out_punch)
            && let (Some(in_time), Some(out_time)) = (
                self.punches[i].rounded_time(),
                self.punches[o].rounded_time(),
            )
            && in_time == out_time
        {
            errors.insert(ShiftErrorType::InOutSame);
        }

        for (position, index) in order.iter().enumerate() {
            let punch_type = self.punches[*index].punch_type();

            let unbalanced = match punch_type {
                PunchType::Break => order
                    .get(position + 1)
                    .is_none_or(|next| self.punches[*next].punch_type() != PunchType::Back),
                PunchType::Back => position
                    .checked_sub(1)
                    .and_then(|previous| order.get(previous))
                    .is_none_or(|previous| {
                        self.punches[*previous].punch_type() != PunchType::Break
                    }),
                _ => false,
            };

            if unbalanced {
                errors.insert(ShiftErrorType::UnbalancedBreak);
                break;
            }
        }

        errors
    }

    /// The shift's span in whole minutes, from its first punch to its last.
    /// `ShiftUtil.shiftDurationInMinutes`.
    fn shift_duration_in_minutes(&self) -> i32 {
        let order = self.punch_order();
        let (Some(first), Some(last)) = (order.first(), order.last()) else {
            return 0;
        };

        let (Some(in_time), Some(out_time)) = (
            self.punches[*first].rounded_time(),
            self.punches[*last].rounded_time(),
        ) else {
            return 0;
        };

        duration_in_seconds(in_time, out_time) / SECONDS_PER_MINUTE
    }

    /// This shift's breaks, as rounded-time ranges. `ShiftUtil.getBreaks(shift,
    /// false)` — the `useActual` argument every caller in this tree passes.
    ///
    /// Punches pair positionally — indices 1&2, 3&4, … in `PunchTimeComparator`
    /// order, not by punch type — and the whole list is empty when the shift
    /// has (persisted) errors or three punches or fewer.
    pub fn breaks(&self) -> Vec<DateTimeRange> {
        // ShiftUtil.getBreaks tests `shift.getErrors()`, the persisted set —
        // not the derived one calcWorkedHours uses two lines later.
        if self.has_errors() || self.punches.len() <= 2 {
            return Vec::new();
        }

        let order = self.punch_order();
        let mut breaks = Vec::new();

        let mut index = 1;
        while index < order.len().saturating_sub(2) {
            let (Some(start), Some(end)) = (
                self.punches[order[index]].rounded_time(),
                self.punches[order[index + 1]].rounded_time(),
            ) else {
                index += 2;
                continue;
            };
            breaks.push(DateTimeRange::of(start, end));
            index += 2;
        }

        breaks
    }

    /// Total break time in whole minutes. `ShiftUtil.totalBreakTimeInMinutes`.
    fn total_break_time_in_minutes(&self) -> i32 {
        self.breaks()
            .iter()
            .map(|range| duration_in_seconds(range.start(), range.end()))
            .sum::<i32>()
            / SECONDS_PER_MINUTE
    }

    /// Recompute worked and net hours from the punches. `calcWorkedHours()`.
    ///
    /// The two roundings are different on purpose: worked hours keep four
    /// decimal places (`TDouble.roundRawHours`) and net hours are reduced to
    /// two (`TDouble.roundHours`). Reusing one rounding for both is exactly the
    /// class of drift the parity audit warns about.
    pub fn calc_worked_hours(&mut self) {
        if self.derived_errors().is_empty() || self.shift_type != ShiftType::Actual {
            let worked_minutes = if self.punches.is_empty() {
                0.0
            } else {
                f64::from(self.shift_duration_in_minutes() - self.total_break_time_in_minutes())
            };

            let worked_hours = worked_minutes / MINUTES_PER_HOUR;

            self.worked_hours = round_raw_hours(worked_hours);
            self.net_hours = round_hours(worked_hours + self.adj_hours);
        } else {
            self.worked_hours = 0.0;
            self.net_hours = 0.0;
        }
    }

    /// Pull the shift's start or end from a punch whose rounded time changed,
    /// then recompute hours. `resetStartAndEndTimesFromPunch`.
    ///
    /// Fired by [`PunchCursor::set_rounded_time`], never called directly by a
    /// rule — which is the whole reason the cursor exists.
    fn reset_start_and_end_times_from_punch(&mut self, index: usize) {
        match self.punches[index].punch_type() {
            PunchType::In => self.start_date_time = self.punches[index].rounded_time(),
            PunchType::Out => self.end_date_time = self.punches[index].rounded_time(),
            _ => {}
        }

        self.calc_worked_hours();
    }
}

/// Whole seconds from `start` to `end`.
///
/// `DateTimePeriod.getDurationInSeconds`, which is what `ShiftUtil` measures
/// spans with.
fn duration_in_seconds(start: LocalDateTime, end: LocalDateTime) -> i32 {
    (end.epoch_seconds() - start.epoch_seconds()) as i32
}

/// `alterLastPunchTime` — move the end of the last worked range by `adj_hours`.
///
/// Java removes the range by value, so with two worked ranges equal to each
/// other it removes the **first** of them and appends the altered one at the
/// end, reordering the list. Reproduced; nothing downstream reads the order,
/// since the caller only sums overlaps.
fn alter_last_punch_time(ranges: &mut Vec<DateTimeRange>, adj_hours: f64) {
    let Some(last) = ranges.last().cloned() else {
        return;
    };

    // `(int)(adjHours * SECONDS_PER_HOUR)` truncates toward zero.
    let seconds = (adj_hours * 3600.0) as i64;
    let altered = DateTimeRange::of(last.start(), last.end().plus_seconds(seconds));

    if let Some(first_equal) = ranges.iter().position(|range| *range == last) {
        ranges.remove(first_equal);
    }
    ranges.push(altered);
}

/// `alterPunchesFromEnd` — spend a negative adjustment backwards through the
/// worked ranges, dropping whole ranges until what is left fits inside one.
///
/// Java's `get(size - 1)` throws once the list empties, which happens when the
/// adjustment is more negative than every worked range put together; the loop
/// stops instead.
fn alter_punches_from_end(ranges: &mut Vec<DateTimeRange>, mut adj_hours: f64) {
    while adj_hours < 0.0 {
        let Some(last) = ranges.last().cloned() else {
            return;
        };
        let last_duration = last.duration().fractional_hours();

        if last_duration > adj_hours.abs() {
            alter_last_punch_time(ranges, adj_hours);
            adj_hours = 0.0;
        } else {
            if let Some(first_equal) = ranges.iter().position(|range| *range == last) {
                ranges.remove(first_equal);
            }
            adj_hours += last_duration;
        }
    }
}

/// Order two punches the way `PunchTimeComparator` does.
///
/// Rounded time first; ties broken by punch type, where an `In` sorts before
/// anything and an `Out` after. A punch with no rounded time is ordered by type
/// alone — which is how a shift missing its out punch still sorts sensibly.
fn compare_punch_times(a: &EmployeeShiftPunch, b: &EmployeeShiftPunch) -> Ordering {
    let (Some(a_time), Some(b_time)) = (a.rounded_time(), b.rounded_time()) else {
        return compare_punch_types(a.punch_type(), b.punch_type());
    };

    a_time
        .epoch_seconds()
        .cmp(&b_time.epoch_seconds())
        .then_with(|| compare_punch_types(a.punch_type(), b.punch_type()))
        // Java's third tiebreak, on the punch type's declaration position. It
        // only decides between two punches at the same instant that
        // comparePunchTypes called equal — two breaks, say — but dropping it
        // would leave their order down to the sort's stability instead.
        .then_with(|| a.punch_type().ordinal().cmp(&b.punch_type().ordinal()))
}

/// `PunchTimeComparator.comparePunchTypes`.
///
/// Note the asymmetry is Java's: the test is `pt1 == IN || pt2 == OUT` rather
/// than a symmetric rank, so a `Break` compared against an `Out` sorts first
/// while two `Break`s compare equal.
fn compare_punch_types(a: PunchType, b: PunchType) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    if a == PunchType::In || b == PunchType::Out {
        return Ordering::Less;
    }
    if a == PunchType::Out || b == PunchType::In {
        return Ordering::Greater;
    }
    Ordering::Equal
}

/// A handle onto one punch of a shift, through which writes stay coupled.
///
/// Stands in for Java's `EmployeeShiftPunch` back-reference. Reads delegate to
/// the punch; [`set_rounded_time`](Self::set_rounded_time) writes it *and* runs
/// the shift's `resetStartAndEndTimesFromPunch`, so a rule cannot update a
/// rounded time without the shift noticing.
pub struct PunchCursor<'a> {
    shift: &'a mut EmployeeShift,
    index: usize,
}

impl PunchCursor<'_> {
    /// The punch this cursor points at.
    pub fn punch(&self) -> &EmployeeShiftPunch {
        &self.shift.punches[self.index]
    }

    /// Its position in the shift's punch list.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The shift that owns it — `punch.getEmployeeShift()`.
    pub fn shift(&self) -> &EmployeeShift {
        self.shift
    }

    /// `getPunchType()`.
    pub fn punch_type(&self) -> PunchType {
        self.punch().punch_type()
    }

    /// `getAdjTime()`.
    pub fn adj_time(&self) -> Option<LocalDateTime> {
        self.punch().adj_time()
    }

    /// `getRoundedTime()`.
    pub fn rounded_time(&self) -> Option<LocalDateTime> {
        self.punch().rounded_time()
    }

    /// `getSource()`.
    pub fn source(&self) -> crate::common::enums::punch_source::PunchSource {
        self.punch().source()
    }

    /// Write the rounded time, firing the shift callback if it changed.
    ///
    /// `EmployeeShiftPunch.setRoundedTime` — both halves of it.
    pub fn set_rounded_time(&mut self, rounded_time: Option<LocalDateTime>) {
        if self.shift.punches[self.index].set_rounded_time(rounded_time) {
            self.shift.reset_start_and_end_times_from_punch(self.index);
        }
    }

    /// Run `action` against every punch on the shift, this one included.
    ///
    /// `WorkedHoursRoundingRuleImpl` iterates `shift.getPunches()` and calls
    /// `setRoundedTime` on each, relying on every one of those firing the
    /// callback so that the `shift.getWorkedHours()` it reads next is correct.
    /// This reproduces that, including the order and the per-punch callback.
    pub fn for_each_punch(&mut self, mut action: impl FnMut(&mut PunchCursor<'_>)) {
        for index in 0..self.shift.punches.len() {
            let mut cursor = PunchCursor {
                shift: self.shift,
                index,
            };
            action(&mut cursor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_source::PunchSource;
    use rstest::rstest;

    fn at(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2010, 1, 2, hour, minute, 0)
    }

    fn punch(id: i32, punch_type: PunchType, hour: i32, minute: i32) -> EmployeeShiftPunch {
        EmployeeShiftPunch::new(id, punch_type, PunchSource::Clock, at(hour, minute))
    }

    fn shift(punches: Vec<EmployeeShiftPunch>) -> EmployeeShift {
        EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Actual,
            punches,
        )
    }

    /// A clean eight-hour shift with no break.
    fn worked_shift() -> EmployeeShift {
        shift(vec![
            punch(1, PunchType::In, 8, 0),
            punch(2, PunchType::Out, 16, 0),
        ])
    }

    #[test]
    fn a_shift_carries_its_identity_and_punches() {
        let shift = worked_shift();

        assert_eq!(shift.id(), 1);
        assert_eq!(shift.employee_id(), 100);
        assert_eq!(shift.job_id(), 200);
        assert_eq!(shift.shift_date(), LocalDate::of(2010, 1, 2));
        assert_eq!(shift.shift_type(), ShiftType::Actual);
        assert_eq!(shift.punch_count(), 2);
    }

    #[test]
    fn rounding_an_in_punch_moves_the_shift_start() {
        // The callback the whole design exists for.
        let mut shift = worked_shift();

        shift.punch_cursor(0).set_rounded_time(Some(at(8, 15)));

        assert_eq!(shift.start_date_time(), Some(at(8, 15)));
        assert_eq!(shift.end_date_time(), None, "the out punch has not moved");
    }

    #[test]
    fn rounding_an_out_punch_moves_the_shift_end() {
        let mut shift = worked_shift();

        shift.punch_cursor(1).set_rounded_time(Some(at(16, 30)));

        assert_eq!(shift.end_date_time(), Some(at(16, 30)));
    }

    #[test]
    fn rounding_a_break_punch_moves_neither_end() {
        let mut shift = shift(vec![
            punch(1, PunchType::In, 8, 0),
            punch(2, PunchType::Break, 12, 0),
            punch(3, PunchType::Back, 12, 30),
            punch(4, PunchType::Out, 16, 0),
        ]);

        shift.punch_cursor(1).set_rounded_time(Some(at(12, 15)));

        assert_eq!(shift.start_date_time(), None);
        assert_eq!(shift.end_date_time(), None);
    }

    #[test]
    fn writing_the_same_rounded_time_fires_no_callback() {
        // Java guards on oldRoundedTime != roundedTime.
        let mut shift = worked_shift();

        shift.punch_cursor(0).set_rounded_time(Some(at(8, 0)));

        assert_eq!(
            shift.start_date_time(),
            None,
            "an unchanged value must not reset the start"
        );
    }

    #[test]
    fn the_callback_recomputes_worked_hours() {
        let mut shift = worked_shift();

        shift.punch_cursor(1).set_rounded_time(Some(at(17, 0)));

        assert_eq!(shift.worked_hours(), 9.0);
        assert_eq!(shift.net_hours(), 9.0);
    }

    #[test]
    fn worked_hours_subtract_the_break() {
        let mut shift = shift(vec![
            punch(1, PunchType::In, 8, 0),
            punch(2, PunchType::Break, 12, 0),
            punch(3, PunchType::Back, 12, 30),
            punch(4, PunchType::Out, 16, 0),
        ]);

        shift.calc_worked_hours();

        assert_eq!(shift.worked_hours(), 7.5);
    }

    #[test]
    fn breaks_are_the_shift_s_punches_paired_positionally() {
        let shift = shift(vec![
            punch(1, PunchType::In, 8, 0),
            punch(2, PunchType::Break, 12, 0),
            punch(3, PunchType::Back, 12, 30),
            punch(4, PunchType::Out, 16, 0),
        ]);

        let breaks = shift.breaks();

        assert_eq!(breaks.len(), 1);
        assert_eq!(breaks[0].start(), at(12, 0));
        assert_eq!(breaks[0].end(), at(12, 30));
    }

    #[test]
    fn a_shift_with_three_punches_or_fewer_has_no_breaks() {
        let shift = shift(vec![
            punch(1, PunchType::In, 8, 0),
            punch(2, PunchType::Break, 12, 0),
            punch(3, PunchType::Back, 12, 30),
        ]);

        assert!(shift.breaks().is_empty());
    }

    #[test]
    fn a_shift_with_no_break_has_no_breaks() {
        assert!(worked_shift().breaks().is_empty());
    }

    #[test]
    fn net_hours_fold_in_the_adjustment() {
        let mut shift = worked_shift().with_adj_hours(1.25);

        shift.calc_worked_hours();

        assert_eq!(shift.worked_hours(), 8.0);
        assert_eq!(shift.net_hours(), 9.25);
    }

    #[test]
    fn worked_and_net_hours_use_different_precisions() {
        // roundRawHours keeps 4 places, roundHours 2. A 20-second shift shows
        // the difference: 0.005555... hours.
        let mut shift = shift(vec![
            punch(1, PunchType::In, 8, 0),
            EmployeeShiftPunch::new(
                2,
                PunchType::Out,
                PunchSource::Clock,
                LocalDateTime::of(2010, 1, 2, 8, 0, 20),
            ),
        ]);

        shift.calc_worked_hours();

        // The span truncates to whole minutes first, so this is zero either
        // way — the point is that the two fields are rounded independently.
        assert_eq!(shift.worked_hours(), 0.0);
        assert_eq!(shift.net_hours(), 0.0);
    }

    #[test]
    fn a_shift_with_errors_has_no_worked_hours() {
        // An actual shift missing its out punch.
        let mut shift = shift(vec![punch(1, PunchType::In, 8, 0)]);

        shift.calc_worked_hours();

        assert!(!shift.derived_errors().is_empty());
        assert_eq!(shift.worked_hours(), 0.0);
        assert_eq!(shift.net_hours(), 0.0);
    }

    #[test]
    fn a_scheduled_shift_with_errors_still_computes_hours() {
        // calcWorkedHours short-circuits on shiftType != ACTUAL.
        let mut shift = EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Schedule,
            vec![punch(1, PunchType::In, 8, 0)],
        );

        shift.calc_worked_hours();

        assert!(!shift.derived_errors().is_empty());
        assert_eq!(shift.worked_hours(), 0.0, "one punch spans no time");
    }

    #[test]
    fn an_empty_actual_shift_is_an_error() {
        let shift = shift(Vec::new());
        assert!(shift.derived_errors().contains(&ShiftErrorType::EmptyShift));
    }

    #[rstest]
    #[case(PunchType::In, ShiftErrorType::MissingIn)]
    #[case(PunchType::Out, ShiftErrorType::MissingOut)]
    #[case(PunchType::Break, ShiftErrorType::MissingBreakOut)]
    #[case(PunchType::Back, ShiftErrorType::MissingBreakIn)]
    fn a_punch_with_no_rounded_time_reports_its_own_error(
        #[case] punch_type: PunchType,
        #[case] expected: ShiftErrorType,
    ) {
        let mut shift = shift(vec![
            punch(1, PunchType::In, 8, 0),
            punch(2, punch_type, 12, 0),
            punch(3, PunchType::Out, 16, 0),
        ]);

        shift.punch_cursor(1).set_rounded_time(None);

        assert!(shift.derived_errors().contains(&expected));
    }

    #[test]
    fn a_clean_shift_has_no_errors() {
        assert!(worked_shift().derived_errors().is_empty());
    }

    #[test]
    fn an_in_and_out_at_the_same_time_is_an_error() {
        let shift = shift(vec![
            punch(1, PunchType::In, 8, 0),
            punch(2, PunchType::Out, 8, 0),
        ]);

        assert!(shift.derived_errors().contains(&ShiftErrorType::InOutSame));
    }

    #[test]
    fn a_break_with_no_matching_back_is_unbalanced() {
        let shift = shift(vec![
            punch(1, PunchType::In, 8, 0),
            punch(2, PunchType::Break, 12, 0),
            punch(3, PunchType::Out, 16, 0),
        ]);

        assert!(
            shift
                .derived_errors()
                .contains(&ShiftErrorType::UnbalancedBreak)
        );
    }

    #[test]
    fn a_matched_break_and_back_are_balanced() {
        let shift = shift(vec![
            punch(1, PunchType::In, 8, 0),
            punch(2, PunchType::Break, 12, 0),
            punch(3, PunchType::Back, 12, 30),
            punch(4, PunchType::Out, 16, 0),
        ]);

        assert!(
            !shift
                .derived_errors()
                .contains(&ShiftErrorType::UnbalancedBreak)
        );
    }

    #[test]
    fn for_each_punch_reaches_every_punch_and_fires_each_callback() {
        // The WorkedHoursRoundingRuleImpl shape: reset every punch to its
        // adjusted time, then read hours that those writes recomputed.
        let mut shift = shift(vec![
            punch(1, PunchType::In, 8, 0),
            punch(2, PunchType::Out, 16, 0),
        ]);
        shift.punch_cursor(0).set_rounded_time(Some(at(9, 0)));
        shift.punch_cursor(1).set_rounded_time(Some(at(17, 0)));
        assert_eq!(shift.worked_hours(), 8.0);

        let mut cursor = shift.punch_cursor(0);
        cursor.for_each_punch(|punch| {
            let adj = punch.adj_time();
            punch.set_rounded_time(adj);
        });

        assert_eq!(shift.punch(0).rounded_time(), Some(at(8, 0)));
        assert_eq!(shift.punch(1).rounded_time(), Some(at(16, 0)));
        assert_eq!(
            shift.start_date_time(),
            Some(at(8, 0)),
            "the callback fired for the in punch"
        );
        assert_eq!(shift.worked_hours(), 8.0);
    }

    #[test]
    fn a_cursor_can_read_the_shift_that_owns_it() {
        // punch.getEmployeeShift() in Java.
        let mut shift = worked_shift();
        let cursor = shift.punch_cursor(0);

        assert_eq!(cursor.shift().job_id(), 200);
        assert_eq!(cursor.index(), 0);
        assert_eq!(cursor.punch_type(), PunchType::In);
        assert_eq!(cursor.adj_time(), Some(at(8, 0)));
        assert_eq!(cursor.source(), PunchSource::Clock);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn a_cursor_onto_a_punch_that_is_not_there_panics() {
        worked_shift().punch_cursor(9);
    }

    #[test]
    fn punches_sort_by_rounded_time() {
        let shift = shift(vec![
            punch(1, PunchType::Out, 16, 0),
            punch(2, PunchType::In, 8, 0),
            punch(3, PunchType::Back, 12, 30),
            punch(4, PunchType::Break, 12, 0),
        ]);

        let ids: Vec<_> = shift
            .punch_order()
            .iter()
            .map(|index| shift.punch(*index).id())
            .collect();

        assert_eq!(ids, vec![2, 4, 3, 1]);
    }

    #[test]
    fn an_out_of_order_in_punch_is_an_invalid_time() {
        // The in punch is not first once the punches are sorted by time.
        let shift = shift(vec![
            punch(1, PunchType::In, 18, 0),
            punch(2, PunchType::Out, 16, 0),
        ]);

        assert!(
            shift
                .derived_errors()
                .contains(&ShiftErrorType::InvalidTimes)
        );
    }
}
