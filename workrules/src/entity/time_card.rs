//! Port of `com.unifocus.rms.timekeeping.domain.timecard.TimeCard`.
//!
//! Ground truth: `taps/src/java/com/unifocus/rms/timekeeping/domain/timecard/TimeCard.java`,
//! plus the two implementations that matter to the engine,
//! `watson/server/labor/calcshift/ActualsTimeCard.java` (with its four
//! `ActualsTimeCard*Methods` helper classes) and `ScheduleCalcDataSet.java`.
//!
//! The ambient context a rule is calculating against: an employee's shifts,
//! schedules, earnings and hours-distribution buckets for a period. In Java it
//! is an **interface** — 211 lines of accessors plus a block of `default`
//! methods that stream over them. Sixteen classes implement it; only the two
//! above ever reach a rule.
//!
//! Only what a ported rule actually calls comes across. That was one method
//! ([`schedules`](TimeCard::schedules)) through Wave 1, and is now the
//! twenty-odd the `hoursdistribution` family reads — see the scoping table in
//! `PARITY_AUDIT.md` for the call counts that decided the set. The rest of the
//! interface (schedule labels, shift add/remove, the pay-group lock, salary
//! distribution) belongs to the calc pipeline, not to rules, and is not here.
//!
//! Many rules ignore the time card entirely: the punch-rounding tests pass
//! `null` for it, which is why it reaches those rules as an `Option`.
//!
//! # `Employee` is optional
//!
//! Java's no-arg `ActualsTimeCard()` leaves it null, and `getEmployeeID()`
//! answers `0` in that case rather than throwing. [`employee`](TimeCard::employee)
//! returns `Option`, and [`employee_id`](TimeCard::employee_id) reproduces the
//! `0`.
//!
//! # The overtime and double-time buckets are looked up by **name**
//!
//! `getOTHoursDistributionTypeId` filters
//! `getName().equals(HoursDistributionType.OVERTIME_NAME)` — the literal string
//! `"Overtime"` — not on the id and not on the premium flag. A site that
//! renames the bucket silently stops getting overtime. Worse, Java declares the
//! method `int` while `filterHoursDistributionTypes` ends in `orElse(null)`, so
//! the miss does not return a sentinel: it unboxes a null and throws NPE deep
//! inside whichever rule asked. Here both return `Option<i32>`, so the miss is
//! visible at the call site.
//!
//! # Filtering shifts that the caller then mutates
//!
//! `getShiftsWithDistributionsForPeriod` returns live references, and
//! `WeeklyOTHrsRuleImpl` — the smallest real rule in the family — writes
//! through them while still reading `timeCard.getOTHoursDistributionTypeId()`
//! inside the same loop. The same aliasing problem the punch cursor solved.
//!
//! The answer here is that the **index form is the primitive**:
//! [`shift_indices_with_distributions_for_period`](TimeCard::shift_indices_with_distributions_for_period)
//! returns positions into [`shifts`](TimeCard::shifts), which a rule then walks
//! against [`shifts_mut`](TimeCard::shifts_mut) while the card itself stays
//! free to be read. The reference form
//! ([`shifts_with_distributions_for_period`](TimeCard::shifts_with_distributions_for_period))
//! is a wrapper over it, for the read-only sites.
//!
//! # One struct for both implementations
//!
//! [`TimeCardData`] stands in for `ActualsTimeCard` and `ScheduleCalcDataSet`
//! alike. Across every method on this trait the two agree, once you notice that
//! `ScheduleCalcDataSet.getShifts()` and `.getSchedules()` return the *same*
//! list — which is why its `getShiftsWithDistributionsForPeriod` iterates
//! `schedules` and its `getSchedulesWithDistributionsForPeriod` just delegates.
//! Their remaining differences (lock checks, the calculator references, the
//! availability model) are all outside the rules' reach.
//!
//! Two accessors are fields here that `ActualsTimeCard` derives:
//! `getCalculationStartDate` walks the employee's pay group and custom data for
//! a `lastPPEndFieldID` property-data entry, and `getDatasetStartDate` is that
//! minus ten days. Reproducing it needs `PayGroup`, `PropertyData` and
//! `EmployeeCustomField`, none of which any rule touches. `ScheduleCalcDataSet`
//! holds both as plain fields, and so does this.
//!
//! # `DateRange` is not ported
//!
//! Every period argument is `date_range_rs::DateRange`. Java's rules tree uses
//! three names for one behaviour — `LegacyDatePeriod` (what every
//! `HoursDistributionRuleImpl.execute` takes as its work week),
//! `ArbitraryDateRange`, and the `DefaultDateRange` both extend — which differ
//! only in `getDateRangesPerYear` and the deprecation notice. `containsDate` is
//! `startDate.isOnOrBefore(date) && endDate.isOnOrAfter(date)`, matching
//! `DateRange::contains_date` exactly.

use crate::common::enums::employee_calculation_mode::EmployeeCalculationMode;
use crate::entity::calc_data_set_stat::CalcDataSetStat;
use crate::entity::employee::Employee;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::employee_job_status::EmployeeJobStatus;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::flsa_data::FlsaData;
use crate::entity::hours_distribution::HoursDistribution;
use crate::entity::hours_distribution_type::HoursDistributionType;
use date_range_rs::DateRange;
use joda_rs::LocalDate;
use std::collections::HashMap;

/// A shift paired with one of its distributions. `ShiftToDistribution`.
///
/// Java's class owns two references; this borrows from the time card, which is
/// the same lifetime the Java objects have in practice — the pairs are consumed
/// inside the stream that produced them.
#[derive(Debug, Clone, Copy)]
pub struct ShiftToDistribution<'a> {
    shift: &'a EmployeeShift,
    distribution: &'a HoursDistribution,
}

impl<'a> ShiftToDistribution<'a> {
    /// Pair a shift with one of its distributions.
    pub fn new(shift: &'a EmployeeShift, distribution: &'a HoursDistribution) -> Self {
        Self {
            shift,
            distribution,
        }
    }

    /// `getShift()`.
    pub fn shift(&self) -> &'a EmployeeShift {
        self.shift
    }

    /// `getDistribution()`.
    pub fn distribution(&self) -> &'a HoursDistribution {
        self.distribution
    }
}

/// What a rule is calculating against. `TimeCard`.
///
/// The required methods are Java's abstract accessors; everything below them is
/// a `default` method there too, with the exception of the four noted in their
/// own doc comments.
pub trait TimeCard {
    // ---- state -------------------------------------------------------------

    /// The employee, if one has been set. `getEmployee()`.
    fn employee(&self) -> Option<&Employee>;

    /// The actual worked shifts. `getShifts()`.
    fn shifts(&self) -> &[EmployeeShift];

    /// The shifts, for a rule that is redistributing their hours.
    fn shifts_mut(&mut self) -> &mut Vec<EmployeeShift>;

    /// The employee's scheduled shifts. `getSchedules()`.
    fn schedules(&self) -> &[EmployeeShift];

    /// The employee's earnings. `getEarnings()`.
    fn earnings(&self) -> &[EmployeeEarning];

    /// The earnings, for a rule that is adding to them.
    ///
    /// Java hands out the live list and `CaliforniaOTHrsRuleImpl` calls
    /// `timeCard.getEarnings().add(newEarning)` on it, the same shape as
    /// [`stat_map_mut`](Self::stat_map_mut). Appending never invalidates a
    /// position a rule already holds, so the index forms of divergence 22 stay
    /// valid across a write — and an earning a rule creates is never revisited
    /// by the pass that created it.
    fn earnings_mut(&mut self) -> &mut Vec<EmployeeEarning>;

    /// The buckets hours may be distributed into — regular, overtime, double
    /// time and whatever else the property configured.
    /// `getHoursDistributionTypes()`.
    fn hours_distribution_types(&self) -> &[HoursDistributionType];

    /// The per-week FLSA figures, keyed by the week's **end** date.
    /// `getFlsaDataMap()`.
    fn flsa_data_map(&self) -> &HashMap<LocalDate, FlsaData>;

    /// The same, computed over the pay period rather than the week.
    /// `getFlsaPayPeriodDataMap()`.
    fn flsa_pay_period_data_map(&self) -> &HashMap<LocalDate, FlsaData>;

    /// Statistics rules leave for each other. `getStatMap()`.
    fn stat_map(&self) -> &HashMap<(CalcDataSetStat, LocalDate), f64>;

    /// The stat map, for the rule doing the writing.
    ///
    /// Java hands out the live map from `getStatMap()` and
    /// `ContractOTHrsRuleImpl` calls `.put` straight on it; the write needs its
    /// own accessor here.
    fn stat_map_mut(&mut self) -> &mut HashMap<(CalcDataSetStat, LocalDate), f64>;

    /// Whether this card is being calculated for time and attendance or for
    /// scheduling. `getCalculationMode()`.
    fn calculation_mode(&self) -> EmployeeCalculationMode;

    /// The first date the dataset covers. `getDatasetStartDate()`.
    fn dataset_start_date(&self) -> LocalDate;

    /// The first date still open to calculation — everything before it belongs
    /// to a closed pay period. `getCalculationStartDate()`.
    fn calculation_start_date(&self) -> LocalDate;

    /// The pay period the property is currently in.
    /// `employee.getProperty().getPayPeriod()`, which is
    /// `getPayGroup().currentPayPeriod()`.
    ///
    /// Not on Java's `TimeCard` — rules reach it through the employee's
    /// property. It sits here for the same reason
    /// [`calculation_start_date`](Self::calculation_start_date) does: both are
    /// values `PayGroup` derives, `PayGroup` is not ported, and the card is
    /// already the stand-in for what the pay group knows (divergence 24).
    ///
    /// The range carries its own period arithmetic, so
    /// [`pay_period_containing`](Self::pay_period_containing) can walk from it
    /// to any other period.
    fn current_pay_period(&self) -> &DateRange;

    // ---- identity ----------------------------------------------------------

    /// `getEmployeeID()`, which answers `0` when no employee is set.
    fn employee_id(&self) -> i32 {
        self.employee().map_or(0, Employee::id)
    }

    // ---- period filters ----------------------------------------------------

    /// Positions in [`shifts`](Self::shifts) whose **shift date** falls inside
    /// `period`. The index primitive behind `getShiftsForPeriod(DateRange)`.
    fn shift_indices_for_period(&self, period: &DateRange) -> Vec<usize> {
        self.shifts()
            .iter()
            .enumerate()
            .filter(|(_, shift)| period.contains_date(shift.shift_date()))
            .map(|(index, _)| index)
            .collect()
    }

    /// The shifts whose date falls inside `period`. `getShiftsForPeriod()`.
    fn shifts_for_period(&self, period: &DateRange) -> Vec<&EmployeeShift> {
        self.shift_indices_for_period(period)
            .into_iter()
            .map(|index| &self.shifts()[index])
            .collect()
    }

    /// Positions in [`shifts`](Self::shifts) carrying at least one distribution
    /// dated inside `period`. The index primitive behind
    /// `getShiftsWithDistributionsForPeriod(DateRange)`.
    ///
    /// Note the filter is on the **distribution** dates, not the shift date: an
    /// overnight shift dated the 6th distributing hours into the 7th is in a
    /// week starting the 7th by this test and out of it by
    /// [`shift_indices_for_period`](Self::shift_indices_for_period).
    fn shift_indices_with_distributions_for_period(&self, period: &DateRange) -> Vec<usize> {
        self.shifts()
            .iter()
            .enumerate()
            .filter(|(_, shift)| shift.has_distribution_within_period(period))
            .map(|(index, _)| index)
            .collect()
    }

    /// The shifts carrying a distribution inside `period`.
    /// `getShiftsWithDistributionsForPeriod()`.
    fn shifts_with_distributions_for_period(&self, period: &DateRange) -> Vec<&EmployeeShift> {
        self.shift_indices_with_distributions_for_period(period)
            .into_iter()
            .map(|index| &self.shifts()[index])
            .collect()
    }

    /// The **scheduled** shifts carrying a distribution inside `period`.
    /// `getSchedulesWithDistributionsForPeriod()`.
    fn schedules_with_distributions_for_period(&self, period: &DateRange) -> Vec<&EmployeeShift> {
        self.schedules()
            .iter()
            .filter(|schedule| schedule.has_distribution_within_period(period))
            .collect()
    }

    /// The earnings dated inside `period`. `getEarningsForPeriod()`.
    fn earnings_for_period(&self, period: &DateRange) -> Vec<&EmployeeEarning> {
        self.earnings()
            .iter()
            .filter(|earning| period.contains_date(earning.earning_date()))
            .collect()
    }

    // ---- open for editing --------------------------------------------------

    /// Whether `date` is still open to calculation. `isOpenForEditingOn()`.
    ///
    /// Abstract in Java, but both implementations reduce to the same test —
    /// `ActualsTimeCardValidationMethods.isOpenForEditingOn` is
    /// `date.isOnOrAfter(getCalculationStartDate(employee))` and
    /// `ScheduleCalcDataSet`'s is the same against its field — so it is a
    /// provided method here.
    fn is_open_for_editing_on(&self, date: LocalDate) -> bool {
        date >= self.calculation_start_date()
    }

    /// Whether a shift's date is open. `isOpenForEditingFor(EmployeeShift)`.
    fn is_open_for_editing_for_shift(&self, shift: &EmployeeShift) -> bool {
        self.is_open_for_editing_on(shift.shift_date())
    }

    /// Whether an earning's date is open. `isOpenForEditingFor(EmployeeEarning)`.
    fn is_open_for_editing_for_earning(&self, earning: &EmployeeEarning) -> bool {
        self.is_open_for_editing_on(earning.earning_date())
    }

    /// The employee's job status for the job a shift was worked in, on that
    /// shift's date. `EmployeeShift.getEmployeeJobStatus()`.
    ///
    /// Java hangs this off the shift — `getEmployee().getEmployeeJobStatus(getJob(), getShiftDate())`,
    /// a `@Transient` derived getter, reachable because a shift holds a
    /// back-reference to its employee. The entity model here is one-way
    /// (see `entity`), so the question is asked of the card, which has the
    /// employee.
    ///
    /// `None` where Java would return null — no employee on the card, or no
    /// status covering that job on that date. Java's callers are split on
    /// whether they check: `WeeklyOTSecJobHrsRuleImpl` guards explicitly,
    /// `ConsecutiveDaysCalculator` and `EarningMapper` do not and throw. See
    /// divergence 32.
    fn employee_job_status_for_shift(&self, shift: &EmployeeShift) -> Option<&EmployeeJobStatus> {
        self.employee()
            .and_then(|employee| employee.employee_job_status(shift.job_id(), shift.shift_date()))
    }

    /// Whether a shift was worked in a job the employee is not salaried-exempt
    /// in — the filter three of the family's helpers and rules apply, each
    /// spelled out inline in Java.
    ///
    /// A shift with no job status answers `false`; divergence 32.
    fn shift_is_not_salaried_exempt(&self, shift: &EmployeeShift) -> bool {
        self.employee_job_status_for_shift(shift)
            .is_some_and(|status| status.pay_type().is_not_salaried_exempt())
    }

    /// The same question of an earning, against its job on its earning date.
    ///
    /// `CaliforniaOTHrsRuleImpl` applies both spellings side by side —
    /// `employeeJobStatusIsNotSalariedExemptForShift` and
    /// `…ForEarning` — differing only in which date and job they read. An
    /// earning with no job status answers `false`; divergence 32 again.
    fn earning_is_not_salaried_exempt(&self, earning: &EmployeeEarning) -> bool {
        self.employee()
            .and_then(|employee| {
                employee.employee_job_status(earning.job_id(), earning.earning_date())
            })
            .is_some_and(|status| status.pay_type().is_not_salaried_exempt())
    }

    /// The pay period containing `date`, walked out from the current one.
    /// `getPayPeriod().getDateRangeContainingDate(date)`.
    fn pay_period_containing(&self, date: LocalDate) -> DateRange {
        self.current_pay_period().range_containing_date(date)
    }

    /// Whether this calculation is building a schedule rather than
    /// reconciling actuals.
    ///
    /// `ConsecutiveDaysCalculator.isRunFromScheduling(EmployeeCalculationMode)`,
    /// which is the engine's own spelling of the question. It also stands in
    /// for `ScheduledShiftOTRuleImpl`'s `timeCard instanceof ScheduleCalcDataSet`
    /// — see divergence 41 for why those two are not provably the same test.
    fn is_run_from_scheduling(&self) -> bool {
        matches!(
            self.calculation_mode(),
            EmployeeCalculationMode::AutoSchedule | EmployeeCalculationMode::EditSchedule
        )
    }

    /// `hasShiftsOrEarnings()`.
    fn has_shifts_or_earnings(&self) -> bool {
        !self.shifts().is_empty() || !self.earnings().is_empty()
    }

    // ---- distribution buckets ----------------------------------------------

    /// Every configured bucket's id, in configuration order.
    /// `getHoursDistributionTypeIds()`.
    fn hours_distribution_type_ids(&self) -> Vec<i32> {
        self.hours_distribution_types()
            .iter()
            .map(HoursDistributionType::id)
            .collect()
    }

    /// The ids of the buckets that are **not** premium — the ones regular hours
    /// land in. `getRegularHoursDistributionTypeIds()`.
    fn regular_hours_distribution_type_ids(&self) -> Vec<i32> {
        self.hours_distribution_types()
            .iter()
            .filter(|distribution_type| !distribution_type.premium())
            .map(HoursDistributionType::id)
            .collect()
    }

    /// The overtime bucket, found by the name `"Overtime"`.
    /// `getOTHoursDistributionTypeId()` — see the module note on why this is an
    /// `Option` where Java declares an `int`.
    fn ot_hours_distribution_type_id(&self) -> Option<i32> {
        self.hours_distribution_type_named(HoursDistributionType::OVERTIME_NAME)
    }

    /// The double-time bucket, found by the name `"Double Time"`.
    /// `getDTHoursDistributionTypeId()`.
    fn dt_hours_distribution_type_id(&self) -> Option<i32> {
        self.hours_distribution_type_named(HoursDistributionType::DOUBLE_TIME_NAME)
    }

    /// The first bucket with this name. `filterHoursDistributionTypes()`,
    /// whose `Predicate` argument only ever arrives as one of the two
    /// name comparisons above.
    fn hours_distribution_type_named(&self, name: &str) -> Option<i32> {
        self.hours_distribution_types()
            .iter()
            .find(|distribution_type| distribution_type.name() == name)
            .map(HoursDistributionType::id)
    }

    /// Whether a distribution sits in a premium bucket.
    /// `distributionIsPremium()`.
    ///
    /// Note it is defined as *not* regular, so a distribution with no type id
    /// — or one naming a bucket this property has not configured — counts as
    /// premium.
    fn distribution_is_premium(&self, distribution: &HoursDistribution) -> bool {
        !self.distribution_is_regular(distribution)
    }

    /// Whether a distribution sits in a non-premium bucket.
    /// `distributionIsRegular()`.
    fn distribution_is_regular(&self, distribution: &HoursDistribution) -> bool {
        distribution
            .hours_distribution_type_id()
            .is_some_and(|id| self.regular_hours_distribution_type_ids().contains(&id))
    }

    /// Whether a distribution sits in the overtime bucket. `distributionIsOT()`.
    fn distribution_is_ot(&self, distribution: &HoursDistribution) -> bool {
        self.ot_hours_distribution_type_id()
            .is_some_and(|id| distribution.is_of_type(id))
    }

    /// Whether a distribution sits in the double-time bucket.
    /// `distributionIsDT()`.
    fn distribution_is_dt(&self, distribution: &HoursDistribution) -> bool {
        self.dt_hours_distribution_type_id()
            .is_some_and(|id| distribution.is_of_type(id))
    }

    // ---- hours totals ------------------------------------------------------

    /// Every distribution on every **actual** shift dated inside `period`.
    /// `getHoursDistributionsStream()`.
    fn hours_distributions_in(&self, period: &DateRange) -> Vec<&HoursDistribution> {
        self.shifts()
            .iter()
            .flat_map(EmployeeShift::hours_distributions)
            .filter(|distribution| distribution.falls_within_period(period))
            .collect()
    }

    /// One shift's premium hours. `getTotalPremiumHours(EmployeeShift)`.
    ///
    /// Unlike the period form this does **not** filter by date: every
    /// distribution the shift owns is counted.
    fn total_premium_hours_for_shift(&self, shift: &EmployeeShift) -> f64 {
        shift
            .hours_distributions()
            .iter()
            .filter(|distribution| self.distribution_is_premium(distribution))
            .map(HoursDistribution::hours)
            .sum()
    }

    /// `shiftHasPremiumHours()`.
    fn shift_has_premium_hours(&self, shift: &EmployeeShift) -> bool {
        self.total_premium_hours_for_shift(shift) > 0.0
    }

    /// Premium hours across the period. `getTotalPremiumHours(DateRange)`.
    fn total_premium_hours(&self, period: &DateRange) -> f64 {
        self.hours_distributions_in(period)
            .into_iter()
            .filter(|distribution| self.distribution_is_premium(distribution))
            .map(HoursDistribution::hours)
            .sum()
    }

    /// Regular hours across the period. `getTotalRegularHours()`.
    fn total_regular_hours(&self, period: &DateRange) -> f64 {
        self.hours_distributions_in(period)
            .into_iter()
            .filter(|distribution| self.distribution_is_regular(distribution))
            .map(HoursDistribution::hours)
            .sum()
    }

    /// Every distributed hour in the period, premium and regular alike.
    /// `getNetHours()`.
    fn net_hours(&self, period: &DateRange) -> f64 {
        self.hours_distributions_in(period)
            .into_iter()
            .map(HoursDistribution::hours)
            .sum()
    }

    /// The `(shift, distribution)` **positions** in `period` that sit in one
    /// bucket. The index primitive behind
    /// `distributionsWithShiftDuringPeriodMatchingType()`.
    ///
    /// The first element indexes [`shifts`](Self::shifts), the second that
    /// shift's own [`hours_distributions`](EmployeeShift::hours_distributions).
    /// A rule that is about to rewrite the distributions it selected needs
    /// this form; see divergence 22.
    fn distribution_indices_during_period_matching_type(
        &self,
        period: &DateRange,
        hours_distribution_type_id: i32,
    ) -> Vec<(usize, usize)> {
        self.shifts()
            .iter()
            .enumerate()
            .flat_map(|(shift_index, shift)| {
                shift
                    .hours_distributions()
                    .iter()
                    .enumerate()
                    .filter(|(_, distribution)| {
                        distribution.is_of_type(hours_distribution_type_id)
                            && distribution.falls_within_period(period)
                    })
                    .map(move |(distribution_index, _)| (shift_index, distribution_index))
            })
            .collect()
    }

    /// The (shift, distribution) pairs in `period` that sit in one bucket.
    /// `distributionsWithShiftDuringPeriodMatchingType()`.
    fn distributions_with_shift_during_period_matching_type(
        &self,
        period: &DateRange,
        hours_distribution_type_id: i32,
    ) -> Vec<ShiftToDistribution<'_>> {
        self.shifts()
            .iter()
            .flat_map(|shift| {
                shift
                    .hours_distributions()
                    .iter()
                    .map(move |distribution| ShiftToDistribution::new(shift, distribution))
            })
            .filter(|pair| pair.distribution().is_of_type(hours_distribution_type_id))
            .filter(|pair| pair.distribution().falls_within_period(period))
            .collect()
    }
}

/// A time card holding its state directly.
///
/// Stands in for both `ActualsTimeCard` and `ScheduleCalcDataSet` — see the
/// module documentation for why one struct covers the pair.
#[derive(Debug)]
pub struct TimeCardData {
    employee: Option<Employee>,
    shifts: Vec<EmployeeShift>,
    schedules: Vec<EmployeeShift>,
    earnings: Vec<EmployeeEarning>,
    hours_distribution_types: Vec<HoursDistributionType>,
    flsa_data_map: HashMap<LocalDate, FlsaData>,
    flsa_pay_period_data_map: HashMap<LocalDate, FlsaData>,
    stat_map: HashMap<(CalcDataSetStat, LocalDate), f64>,
    calculation_mode: EmployeeCalculationMode,
    dataset_start_date: LocalDate,
    calculation_start_date: LocalDate,
    current_pay_period: DateRange,
}

impl Default for TimeCardData {
    /// An empty card in TA mode, open for editing from 1900.
    ///
    /// `ActualsTimeCard.getCalculationMode()` is the constant
    /// [`EmployeeCalculationMode::Ta`], so that is the default here.
    ///
    /// Both dates default to 1900-01-01 rather than to an `Option`. Java's two
    /// implementations always have them — one derives them from the pay group,
    /// the other is handed them — so a missing date is not a state a rule can
    /// see, and this keeps
    /// [`is_open_for_editing_on`](TimeCard::is_open_for_editing_on) answering
    /// `true` for the tests that do not care about period closing.
    fn default() -> Self {
        Self {
            employee: None,
            shifts: Vec::new(),
            schedules: Vec::new(),
            earnings: Vec::new(),
            hours_distribution_types: Vec::new(),
            flsa_data_map: HashMap::new(),
            flsa_pay_period_data_map: HashMap::new(),
            stat_map: HashMap::new(),
            calculation_mode: EmployeeCalculationMode::Ta,
            dataset_start_date: LocalDate::of(1900, 1, 1),
            calculation_start_date: LocalDate::of(1900, 1, 1),
            current_pay_period: DateRange::new(
                LocalDate::of(1900, 1, 1),
                LocalDate::of(1900, 1, 14),
            ),
        }
    }
}

impl TimeCardData {
    /// An empty card — see [`Default`] for the state it starts in.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach the employee.
    #[must_use]
    pub fn with_employee(mut self, employee: Employee) -> Self {
        self.employee = Some(employee);
        self
    }

    /// Attach the actual worked shifts.
    #[must_use]
    pub fn with_shifts(mut self, shifts: Vec<EmployeeShift>) -> Self {
        self.shifts = shifts;
        self
    }

    /// Attach the scheduled shifts.
    #[must_use]
    pub fn with_schedules(mut self, schedules: Vec<EmployeeShift>) -> Self {
        self.schedules = schedules;
        self
    }

    /// Attach the earnings.
    #[must_use]
    pub fn with_earnings(mut self, earnings: Vec<EmployeeEarning>) -> Self {
        self.earnings = earnings;
        self
    }

    /// Attach the configured distribution buckets.
    #[must_use]
    pub fn with_hours_distribution_types(mut self, types: Vec<HoursDistributionType>) -> Self {
        self.hours_distribution_types = types;
        self
    }

    /// Attach the per-week FLSA figures, keyed by each week's end date.
    #[must_use]
    pub fn with_flsa_data(mut self, flsa_data: HashMap<LocalDate, FlsaData>) -> Self {
        self.flsa_data_map = flsa_data;
        self
    }

    /// Attach the per-pay-period FLSA figures.
    #[must_use]
    pub fn with_flsa_pay_period_data(mut self, flsa_data: HashMap<LocalDate, FlsaData>) -> Self {
        self.flsa_pay_period_data_map = flsa_data;
        self
    }

    /// Set the calculation mode. `ActualsTimeCard` is always
    /// [`EmployeeCalculationMode::Ta`]; `ScheduleCalcDataSet` carries a field.
    #[must_use]
    pub fn with_calculation_mode(mut self, mode: EmployeeCalculationMode) -> Self {
        self.calculation_mode = mode;
        self
    }

    /// Set the calculation start date, and the dataset start date to the ten
    /// days before it that `ActualsTimeCardDateMethods.getDatasetStartDate`
    /// subtracts.
    #[must_use]
    pub fn with_calculation_start_date(mut self, date: LocalDate) -> Self {
        self.calculation_start_date = date;
        self.dataset_start_date = date.minus_days(10);
        self
    }

    /// Set the property's current pay period.
    #[must_use]
    pub fn with_current_pay_period(mut self, pay_period: DateRange) -> Self {
        self.current_pay_period = pay_period;
        self
    }

    /// Set the dataset start date on its own, for a card whose two dates are
    /// not ten days apart.
    #[must_use]
    pub fn with_dataset_start_date(mut self, date: LocalDate) -> Self {
        self.dataset_start_date = date;
        self
    }

    /// Which FLSA map a rule's `calculateOverWeeks` switch selects.
    /// `getFlsaDataMap(boolean)`.
    ///
    /// Not on the trait: Java's version also consults
    /// `employee.getProperty().getPayPeriodType()`, and `Property` is reached
    /// through an id here rather than a reference. It arrives with the first
    /// rule that passes `false`; both current call sites read
    /// [`flsa_data_map`](TimeCard::flsa_data_map) directly.
    pub fn flsa_data_map_for(&self, calculating_weekly: bool) -> &HashMap<LocalDate, FlsaData> {
        if calculating_weekly {
            &self.flsa_data_map
        } else {
            &self.flsa_pay_period_data_map
        }
    }
}

impl TimeCard for TimeCardData {
    fn employee(&self) -> Option<&Employee> {
        self.employee.as_ref()
    }

    fn shifts(&self) -> &[EmployeeShift] {
        &self.shifts
    }

    fn shifts_mut(&mut self) -> &mut Vec<EmployeeShift> {
        &mut self.shifts
    }

    fn schedules(&self) -> &[EmployeeShift] {
        &self.schedules
    }

    fn earnings(&self) -> &[EmployeeEarning] {
        &self.earnings
    }

    fn earnings_mut(&mut self) -> &mut Vec<EmployeeEarning> {
        &mut self.earnings
    }

    fn hours_distribution_types(&self) -> &[HoursDistributionType] {
        &self.hours_distribution_types
    }

    fn flsa_data_map(&self) -> &HashMap<LocalDate, FlsaData> {
        &self.flsa_data_map
    }

    fn flsa_pay_period_data_map(&self) -> &HashMap<LocalDate, FlsaData> {
        &self.flsa_pay_period_data_map
    }

    fn stat_map(&self) -> &HashMap<(CalcDataSetStat, LocalDate), f64> {
        &self.stat_map
    }

    fn stat_map_mut(&mut self) -> &mut HashMap<(CalcDataSetStat, LocalDate), f64> {
        &mut self.stat_map
    }

    fn calculation_mode(&self) -> EmployeeCalculationMode {
        self.calculation_mode
    }

    fn dataset_start_date(&self) -> LocalDate {
        self.dataset_start_date
    }

    fn calculation_start_date(&self) -> LocalDate {
        self.calculation_start_date
    }

    fn current_pay_period(&self) -> &DateRange {
        &self.current_pay_period
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::shift_type::ShiftType;

    const REGULAR: i32 = HoursDistributionType::REGULAR_ID;
    const OVERTIME: i32 = HoursDistributionType::OVERTIME_ID;
    const DOUBLE_TIME: i32 = HoursDistributionType::DT_ID;

    fn week() -> DateRange {
        DateRange::new(LocalDate::of(2010, 1, 3), LocalDate::of(2010, 1, 9))
    }

    fn shift(id: i32, date: LocalDate, distributions: Vec<HoursDistribution>) -> EmployeeShift {
        EmployeeShift::new(id, 100, 200, date, ShiftType::Actual, Vec::new())
            .with_hours_distributions(distributions)
    }

    fn distribution(date: LocalDate, type_id: i32, hours: f64) -> HoursDistribution {
        HoursDistribution::new(11, date, Some(type_id), hours, 10.0)
    }

    /// One shift a day, Monday to Wednesday, eight regular hours each, plus a
    /// fourth shift the following week.
    fn card() -> TimeCardData {
        let shifts = (4..=6)
            .map(|day| {
                let date = LocalDate::of(2010, 1, day);
                shift(day, date, vec![distribution(date, REGULAR, 8.0)])
            })
            .chain(std::iter::once({
                let date = LocalDate::of(2010, 1, 11);
                shift(11, date, vec![distribution(date, REGULAR, 8.0)])
            }))
            .collect();

        TimeCardData::new()
            .with_shifts(shifts)
            .with_hours_distribution_types(HoursDistributionType::system_generated())
    }

    #[test]
    fn a_card_with_no_employee_reports_employee_zero() {
        // Java's ActualsTimeCard.getEmployeeID() answers 0 rather than throwing.
        assert_eq!(TimeCardData::new().employee_id(), 0);
        assert!(TimeCardData::new().employee().is_none());
    }

    #[test]
    fn a_card_carries_its_schedules() {
        let card = TimeCardData::new().with_schedules(vec![EmployeeShift::new(
            1,
            100,
            200,
            LocalDate::of(2010, 1, 2),
            ShiftType::Schedule,
            Vec::new(),
        )]);

        assert_eq!(card.schedules().len(), 1);
        assert_eq!(card.schedules()[0].shift_type(), ShiftType::Schedule);
    }

    #[test]
    fn a_card_may_hold_no_schedules() {
        assert!(
            TimeCardData::new()
                .with_schedules(Vec::new())
                .schedules()
                .is_empty()
        );
    }

    #[test]
    fn shifts_for_period_filters_on_the_shift_date() {
        let card = card();
        let ids: Vec<i32> = card
            .shifts_for_period(&week())
            .iter()
            .map(|shift| shift.id())
            .collect();

        assert_eq!(ids, vec![4, 5, 6], "the 11th is the following week");
    }

    #[test]
    fn shifts_with_distributions_for_period_filters_on_the_distribution_date() {
        let overnight = LocalDate::of(2010, 1, 2);
        let card = TimeCardData::new().with_shifts(vec![shift(
            99,
            overnight,
            // Dated the Saturday, distributing into the Sunday the week starts.
            vec![distribution(LocalDate::of(2010, 1, 3), REGULAR, 4.0)],
        )]);

        assert!(
            card.shifts_for_period(&week()).is_empty(),
            "the shift date is outside the week"
        );
        assert_eq!(
            card.shifts_with_distributions_for_period(&week()).len(),
            1,
            "but its hours land inside it"
        );
    }

    #[test]
    fn the_index_form_addresses_the_same_shifts() {
        let card = card();

        assert_eq!(
            card.shift_indices_with_distributions_for_period(&week()),
            vec![0, 1, 2]
        );
        assert_eq!(card.shift_indices_for_period(&week()), vec![0, 1, 2]);
    }

    #[test]
    fn a_rule_writes_through_the_indices_it_filtered() {
        // The WeeklyOTHrsRuleImpl shape: filter, then mutate, while still
        // reading bucket ids off the card.
        let mut card = card();
        let overtime = card.ot_hours_distribution_type_id().unwrap();
        let indices = card.shift_indices_with_distributions_for_period(&week());

        for index in indices {
            let date = card.shifts()[index].shift_date();
            card.shifts_mut()[index].add_hours_distribution(distribution(date, overtime, 2.0));
        }

        assert_eq!(card.total_regular_hours(&week()), 24.0);
        assert_eq!(card.total_premium_hours(&week()), 6.0);
        assert_eq!(card.net_hours(&week()), 30.0);
    }

    #[test]
    fn schedules_with_distributions_reads_the_schedules_not_the_shifts() {
        let date = LocalDate::of(2010, 1, 4);
        let card = TimeCardData::new()
            .with_shifts(vec![shift(1, date, vec![distribution(date, REGULAR, 8.0)])])
            .with_schedules(vec![
                shift(2, date, vec![distribution(date, REGULAR, 7.5)]),
                shift(3, LocalDate::of(2010, 1, 20), Vec::new()),
            ]);

        let scheduled = card.schedules_with_distributions_for_period(&week());

        assert_eq!(scheduled.len(), 1);
        assert_eq!(scheduled[0].id(), 2);
    }

    #[test]
    fn the_overtime_bucket_is_found_by_name() {
        let card = card();

        assert_eq!(card.ot_hours_distribution_type_id(), Some(OVERTIME));
        assert_eq!(card.dt_hours_distribution_type_id(), Some(DOUBLE_TIME));
    }

    #[test]
    fn a_renamed_overtime_bucket_is_not_found() {
        // Java declares getOTHoursDistributionTypeId() an int over an
        // orElse(null), so this case is an NPE inside whichever rule asked.
        let card = TimeCardData::new().with_hours_distribution_types(vec![
            HoursDistributionType::new(REGULAR, "Regular", false),
            HoursDistributionType::new(OVERTIME, "OT", true),
        ]);

        assert_eq!(card.ot_hours_distribution_type_id(), None);
        assert!(!card.distribution_is_ot(&distribution(LocalDate::of(2010, 1, 4), OVERTIME, 2.0)));
    }

    #[test]
    fn regular_buckets_are_the_ones_that_are_not_premium() {
        let card = card();

        assert_eq!(card.regular_hours_distribution_type_ids(), vec![REGULAR]);
        assert_eq!(
            card.hours_distribution_type_ids(),
            vec![REGULAR, OVERTIME, DOUBLE_TIME]
        );
    }

    #[test]
    fn premium_is_defined_as_not_regular() {
        let card = card();
        let date = LocalDate::of(2010, 1, 4);

        assert!(card.distribution_is_regular(&distribution(date, REGULAR, 8.0)));
        assert!(card.distribution_is_premium(&distribution(date, OVERTIME, 2.0)));
        assert!(
            card.distribution_is_premium(&HoursDistribution::new(11, date, None, 1.0, 10.0)),
            "an untyped distribution is in no regular bucket, so it counts as premium"
        );
        assert!(
            card.distribution_is_premium(&distribution(date, 99, 1.0)),
            "so does one naming a bucket this property has not configured"
        );
    }

    #[test]
    fn a_shifts_premium_hours_ignore_the_period() {
        let date = LocalDate::of(2010, 1, 4);
        let card = card();
        let shift = shift(
            1,
            date,
            vec![
                distribution(date, REGULAR, 8.0),
                distribution(date, OVERTIME, 2.0),
                // Outside the week, and still counted by the shift form.
                distribution(LocalDate::of(2010, 2, 1), DOUBLE_TIME, 1.0),
            ],
        );

        assert_eq!(card.total_premium_hours_for_shift(&shift), 3.0);
        assert!(card.shift_has_premium_hours(&shift));
    }

    #[test]
    fn a_shift_with_only_regular_hours_has_no_premium_hours() {
        let date = LocalDate::of(2010, 1, 4);
        let shift = shift(1, date, vec![distribution(date, REGULAR, 8.0)]);

        assert!(!card().shift_has_premium_hours(&shift));
    }

    #[test]
    fn the_period_totals_count_only_distributions_inside_it() {
        let card = card();

        assert_eq!(card.net_hours(&week()), 24.0, "the 11th is excluded");
        assert_eq!(
            card.net_hours(&DateRange::new(
                LocalDate::of(2010, 1, 3),
                LocalDate::of(2010, 1, 16)
            )),
            32.0
        );
    }

    #[test]
    fn distributions_are_paired_with_the_shift_that_owns_them() {
        let mut card = card();
        let overtime = card.ot_hours_distribution_type_id().unwrap();
        card.shifts_mut()[1].add_hours_distribution(distribution(
            LocalDate::of(2010, 1, 5),
            overtime,
            2.0,
        ));

        let pairs = card.distributions_with_shift_during_period_matching_type(&week(), overtime);

        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].shift().id(), 5);
        assert_eq!(pairs[0].distribution().hours(), 2.0);
    }

    #[test]
    fn a_date_before_the_calculation_start_is_closed() {
        let card = TimeCardData::new().with_calculation_start_date(LocalDate::of(2010, 1, 3));

        assert!(!card.is_open_for_editing_on(LocalDate::of(2010, 1, 2)));
        assert!(
            card.is_open_for_editing_on(LocalDate::of(2010, 1, 3)),
            "the start date itself is open"
        );
        assert!(card.is_open_for_editing_on(LocalDate::of(2010, 1, 4)));
    }

    #[test]
    fn the_dataset_starts_ten_days_before_the_calculation() {
        let card = TimeCardData::new().with_calculation_start_date(LocalDate::of(2010, 1, 11));

        assert_eq!(card.calculation_start_date(), LocalDate::of(2010, 1, 11));
        assert_eq!(card.dataset_start_date(), LocalDate::of(2010, 1, 1));
    }

    #[test]
    fn a_shift_and_an_earning_are_open_by_their_own_dates() {
        use crate::common::enums::earning_source::EarningSource;

        let card = TimeCardData::new().with_calculation_start_date(LocalDate::of(2010, 1, 3));
        let closed = shift(1, LocalDate::of(2010, 1, 2), Vec::new());
        let open = shift(2, LocalDate::of(2010, 1, 3), Vec::new());
        let earning = EmployeeEarning::new(
            1,
            100,
            200,
            7,
            LocalDate::of(2010, 1, 2),
            4.0,
            10.0,
            EarningSource::Manual,
        );

        assert!(!card.is_open_for_editing_for_shift(&closed));
        assert!(card.is_open_for_editing_for_shift(&open));
        assert!(!card.is_open_for_editing_for_earning(&earning));
    }

    #[test]
    fn earnings_are_filtered_by_their_earning_date() {
        use crate::common::enums::earning_source::EarningSource;

        let earning = |id: i32, date: LocalDate| {
            EmployeeEarning::new(id, 100, 200, 7, date, 4.0, 10.0, EarningSource::Manual)
        };
        let card = TimeCardData::new().with_earnings(vec![
            earning(1, LocalDate::of(2010, 1, 4)),
            earning(2, LocalDate::of(2010, 1, 20)),
        ]);

        let inside: Vec<i32> = card
            .earnings_for_period(&week())
            .iter()
            .map(|earning| earning.id())
            .collect();

        assert_eq!(inside, vec![1]);
    }

    #[test]
    fn a_card_knows_whether_it_holds_anything() {
        assert!(!TimeCardData::new().has_shifts_or_earnings());
        assert!(card().has_shifts_or_earnings());
    }

    #[test]
    fn the_stat_map_is_written_through_its_own_accessor() {
        let mut card = TimeCardData::new();
        let key = (
            CalcDataSetStat::GapToContractByPeriodStartDate,
            LocalDate::of(2010, 1, 3),
        );

        assert!(card.stat_map().is_empty());
        card.stat_map_mut().insert(key, 2.5);

        assert_eq!(card.stat_map().get(&key), Some(&2.5));
    }

    #[test]
    fn the_flsa_map_is_keyed_by_the_weeks_end_date() {
        let end = LocalDate::of(2010, 1, 9);
        let data = FlsaData::new(end, 0.0, 0.0, 12.0, 0.0, 480.0, 0.0, 0.0, 40.0, 12.0, 7.25);
        let card = TimeCardData::new().with_flsa_data(HashMap::from([(end, data)]));

        assert_eq!(card.flsa_data_map()[&end].regular_rate(), 12.0);
        assert!(card.flsa_pay_period_data_map().is_empty());
        assert_eq!(card.flsa_data_map_for(true).len(), 1);
        assert!(card.flsa_data_map_for(false).is_empty());
    }

    #[test]
    fn a_card_defaults_to_the_ta_calculation_mode() {
        assert_eq!(
            TimeCardData::new()
                .with_calculation_mode(EmployeeCalculationMode::Ta)
                .calculation_mode(),
            EmployeeCalculationMode::Ta
        );
    }

    #[test]
    fn a_time_card_is_usable_behind_a_trait_object() {
        // The punch families take Option<&dyn TimeCard>, so the trait has to
        // stay dyn-safe as it grows.
        let card = card();
        let dynamic: &dyn TimeCard = &card;

        assert_eq!(dynamic.net_hours(&week()), 24.0);
    }
}
