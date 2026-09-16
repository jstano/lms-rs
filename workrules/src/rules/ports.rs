//! The data the rules reach for, as traits.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/dao/`.
//!
//! Rules do not only compute — 18 of the DAOs in
//! `com.unifocus.watson.server.dao` are reachable from the algorithm packages,
//! across 38 distinct methods. They are good repository traits *by method
//! surface*: small, named, single-purpose finders. Their bodies are not
//! portable — Hibernate `Criteria`, HQL, and in a few places hand-built SQL —
//! so only the surface comes across, and the implementations belong to whatever
//! persistence layer the engine is eventually wired to.
//!
//! # Scope
//!
//! Only the ports whose signatures can be spelled with entities that exist are
//! defined here. The rest arrive with the family that needs them, because a
//! port's method set is only knowable from its call sites — `EmployeeShiftDAO`
//! is 1,497 lines of which exactly four methods are reachable from the rules
//! tree, and guessing the other seventeen ports now would be inventing an API
//! nobody calls.
//!
//! Still to define, with the methods the rules actually call:
//!
//! | DAO | Uses | Methods | Blocked on |
//! |---|---:|---|---|
//! | `AccrualAppliedAuditDAO` | 16 | `applyAmount` | — |
//! | `AccrualTransactionDAO` | 4 | `getTransactionsForEmployeeWithUnappliedHours`, `getLatestTransactionsPriorToPayPeriod` | `AccrualTransaction` |
//! | `HolidayDataDAO` | 9 | `loadHolidayEligibilityHours`, `getWeekStartOfNthPastWorkedWeek`, `getNumberDatesOfHolidayEligibility` | — |
//! | `RawPunchLogDAO` | 10 | four `get*RawPunchLogOfTypeWithinTimeRange` variants | `RawPunchLog` |
//! | `EmployeeScheduleLabelDAO` | 15 | `countEmployeeLabelsMatchingText`, `save` | `EmployeeScheduleLabel` |
//! | `EmployeeSetDAO` | 2 | `findAllForProperty`, `filterEmployees` | `EmployeeSet` |
//! | `EmployeeInOutDAO` | 2 | `isEmployeeOnClock` | — |
//! | `EmployeeTimeOffTypeDAO` | 2 | `findByID` | `EmployeeTimeOffType` |
//! | `SalaryDistDAO` | 2 | `findByID` | `SalaryDist` |
//! | `PostPunchResponseLogDAO` | 3 | `getPostPunchResponseLogForDate` | `PostPunchResponseLog` |
//! | `WatsonUserDAO` | 2 | — | `WatsonUser` |

use crate::common::enums::shift_type::ShiftType;
use crate::entity::assignment::Assignment;
use crate::entity::earning_type::EarningType;
use crate::entity::employee_earning::EmployeeEarning;
use crate::entity::holiday::Holiday;
use crate::entity::time_card::TimeCard;
use date_range_rs::DateRange;
use joda_rs::LocalDate;

/// A configurable per-site setting. `com.unifocus.watson.common.enums.PropertyDataKey`.
///
/// The Java enum is a 312-line catalogue of every setting the product has; only
/// the keys the rules and their runners actually read come across.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyDataKey {
    /// The rounding threshold in minutes — a single value, or four
    /// comma-separated values for in, out, break and back.
    PunchRoundingThreshold,
    /// The longest shift time and attendance will accept.
    MaxShiftTa,
    /// The certification a pre-shift screening rule checks for.
    PreScreenSurveyCertificationId,
    /// Whether punch matching should also look for shift errors.
    CheckForShiftErrorsWhileMatching,
    /// Whether to skip the shift-length check.
    BypassShiftLengthCheck,
    /// Whether overtime is validated.
    ValidateOvertime,
    /// Whether availability is validated.
    ValidateAvailability,
    /// Whether daily differential earnings show on empty time cards.
    ShowDailyDifferentialEarningsOnEmptyTimeCards,
}

impl PropertyDataKey {
    /// The stored key name, which is the Java enum constant's own spelling.
    pub fn key(&self) -> &'static str {
        match self {
            Self::PunchRoundingThreshold => "punchRoundingThreshold",
            Self::MaxShiftTa => "maxShiftTA",
            Self::PreScreenSurveyCertificationId => "preScreenSurveyCertificationID",
            Self::CheckForShiftErrorsWhileMatching => "checkForShiftErrorsWhileMatching",
            Self::BypassShiftLengthCheck => "bypassShiftLengthCheck",
            Self::ValidateOvertime => "validateOvertime",
            Self::ValidateAvailability => "validateAvailability",
            Self::ShowDailyDifferentialEarningsOnEmptyTimeCards => {
                "showDailyDifferentialEarningsOnEmptyTimeCards"
            }
        }
    }
}

/// Per-site settings. `PropertyDataDAO`.
///
/// `PropertyDataRoundingRuleImpl` — one of the six punch-rounding rules — reads
/// its threshold from here rather than from its own parameters, which is why
/// the simplest-looking family still needs a port.
pub trait PropertyDataPort {
    /// The raw value for a key, or `None` if unset.
    ///
    /// Java's `getValue(Property, key, default)` takes the whole entity and the
    /// default inline. This takes the id, because that is all the query uses and
    /// because a rule reaching it has navigated
    /// `punch.getEmployeeShift().getJob().getProperty()` — a chain that needs no
    /// lazy loading if only the id is wanted. `Option` replaces the inline
    /// default; every rules call site passes `null` for it anyway.
    fn value(&self, property_id: i32, key: PropertyDataKey) -> Option<String>;

    /// The value as a number. `getValueAsDouble`.
    fn value_as_double(&self, property_id: i32, key: PropertyDataKey) -> Option<f64>;

    /// The value as a flag. `getValueAsBoolean`.
    fn value_as_boolean(&self, property_id: i32, key: PropertyDataKey) -> Option<bool>;
}

/// Earning types by id. `EarningTypeDAO`.
///
/// `findByID` is the single most-called DAO method in the engine — 87 call
/// sites — because rules carry earning type *ids* in their parameters and have
/// to resolve them to price anything.
pub trait EarningTypePort {
    /// `findByID(int)`.
    fn find_by_id(&self, id: i32) -> Option<EarningType>;
}

/// Jobs by id. `AssignmentDAO`.
pub trait AssignmentPort {
    /// `findByID(int)`.
    fn find_by_id(&self, id: i32) -> Option<Assignment>;
}

/// Reading and writing earnings. `EmployeeEarningDAO`.
///
/// The rate and earning families exist to produce these, so this is the
/// engine's main write port.
pub trait EmployeeEarningPort {
    /// Persist an earning. `save` / `makePersistent`, which Java uses
    /// interchangeably at these call sites.
    fn save(&mut self, earning: EmployeeEarning);

    /// Persist several. `saveAll`.
    fn save_all(&mut self, earnings: Vec<EmployeeEarning>) {
        for earning in earnings {
            self.save(earning);
        }
    }

    /// Remove an earning. `makeTransient`.
    fn remove(&mut self, earning_id: i32);

    /// Total hours in a period across a set of earning types.
    /// `getEarningHoursForPeriodAndTypes`.
    fn earning_hours_for_period_and_types(
        &self,
        employee_id: i32,
        period: &DateRange,
        earning_type_ids: &[i32],
    ) -> Option<f64>;
}

/// Shift totals the rules need but do not hold. `EmployeeShiftDAO`.
///
/// The Java DAO is 1,497 lines; these are the methods the rules tree reaches.
pub trait EmployeeShiftPort {
    /// Net actual hours worked in a period. `getNetActualHoursInPeriod`.
    fn net_actual_hours_in_period(&self, employee_id: i32, period: &DateRange) -> f64;

    /// Is the employee scheduled at this moment, within grace?
    /// `isScheduled(Employee, LocalDateTime, int, int)`.
    fn is_scheduled(
        &self,
        employee_id: i32,
        date_time: joda_rs::LocalDateTime,
        grace_pre_schedule: i32,
        grace_post_schedule: i32,
    ) -> bool;

    /// Distributed hours over a period that the time card does not reach back
    /// far enough to hold. `getNetAndOTHoursForPeriod`.
    ///
    /// `RollingXWeeksOTHrsRuleImpl` uses it to seed a rolling multi-week
    /// window when the window starts before `getDatasetStartDate()`.
    ///
    /// # It counts by hard-coded bucket id
    ///
    /// The SQL is
    /// `SUM(CASE WHEN hd.HoursDistributionTypeID = 1 THEN OriginalHours ELSE 0 END)`
    /// for net and `= 2 ... hd.hours` for overtime — the `REGULAR_ID` and
    /// `OVERTIME_ID` constants, not the property's configured buckets. The
    /// same rule's in-card path uses `getRegularHoursDistributionTypeIds()`
    /// and counts **every** non-regular bucket as overtime, so the two halves
    /// of one calculation disagree about what a double-time hour is. See the
    /// finding in `PARITY_AUDIT.md`.
    ///
    /// # A third column is computed and thrown away
    ///
    /// The query also selects a `dtHours` sum. The rule reads `result[0]` and
    /// `result[1]` only, so it never leaves the DAO. Not carried here.
    ///
    /// `None` where Java's `result[0]` is null — an empty row set, which makes
    /// all three `SUM`s null together, so Java's separate per-element checks
    /// collapse into one.
    fn net_and_ot_hours_for_period(
        &self,
        employee_id: i32,
        period: &DateRange,
        shift_type: ShiftType,
        home_dept_only: bool,
    ) -> Option<NetAndOtHours>;
}

/// The two figures `getNetAndOTHoursForPeriod` yields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NetAndOtHours {
    /// Summed `originalHours` of the regular bucket.
    pub net_hours: f64,
    /// Summed `hours` of the overtime bucket.
    pub ot_hours: f64,
}

/// The minimum wage in force for a job on a date.
///
/// **Not a DAO port.** It stands in for `Employee.getMinWage(Assignment, LocalDate)`,
/// which is an entity-graph walk rather than a query:
///
/// ```java
/// double minWage = assignment == null ? 0.0 : assignment.getEffectiveMinWage(effectiveDate);
/// if (minWage == 0.0) {
///    minWage = getProperty().getMinWage();
/// }
/// ```
///
/// `getEffectiveMinWage` picks the latest `AssignmentPayRate` effective on or
/// before the date, falling back up the parent-assignment chain when the job
/// has none of its own; `Property.getMinWage()` reads it off the property's
/// `State`, answering `0.0` when no state is set. Four entities —
/// `AssignmentPayRate`, the assignment parent chain, `Property`, `State` —
/// none of which any ported rule touches for anything else, to resolve one
/// scalar.
///
/// So it comes across keyed by ids, the way divergence 18 already keyed
/// `PropertyDataPort`: a caller has navigated
/// `employee.getProperty()` and `jobStatus.getJob()` and needs a number off the
/// end of it, not the entities. See divergence 38.
///
/// One caller so far: `WeeklyOTHrsRuleImpl`'s FLSA 7(i) exemption check.
pub trait MinWagePort {
    /// `Employee.getMinWage(Assignment, LocalDate)`.
    ///
    /// `job_id` is `None` where Java passes a null assignment, which short
    /// circuits to the property's own figure.
    fn min_wage(&self, property_id: i32, job_id: Option<i32>, effective_date: LocalDate) -> f64;
}

/// A property's holiday calendar. `HolidayDAO`.
///
/// Fourteen call sites across the tree; `HolidayDTHrsRuleImpl` is the first
/// ported one and needs only the property-wide finder. `findAllForPeriod`
/// arrives with the holiday families.
pub trait HolidayPort {
    /// Every holiday configured at a property, across all years.
    /// `findAllForProperty(int)`.
    ///
    /// Callers key the result by date, so a property with two holidays on one
    /// date is a configuration this returns and the caller has to decide about
    /// — Java's caller throws.
    fn find_all_for_property(&self, property_id: i32) -> Vec<Holiday>;
}

/// How long a run of worked days was already going before the time card starts.
/// `EmployeeShiftConsecutiveDaysDAO`.
///
/// The two overtime prior-days calculators need to look further back than the
/// time card holds. Both methods count back from an anchor date to the earliest
/// day of the unbroken run that reaches it, and answer how many days that is —
/// zero when the anchor day itself was not worked.
///
/// # The two anchors differ
///
/// `getPriorConsecutiveDayWorked` counts back from
/// `timeCard.getDatasetStartDate()`; `getPriorConsecutiveDaysWorkedIncludingEarnings`
/// counts back from `timeCard.getCalculationStartDate()` — ten days later. Both
/// are cached in the stat map under the *dataset* start date, so the keys do not
/// collide only because the two `CalcDataSetStat` constants differ. Whether the
/// asymmetry is deliberate is not decidable from the source; it is reproduced.
///
/// # What the implementation does
///
/// Both are hand-built SQL windowing over `HoursDistribution` joined to
/// `EmployeeShift`, excluding shifts carrying the has-errors flag and matching
/// shift type to the calculation mode (`ACTUAL` for
/// [`Ta`](crate::common::enums::employee_calculation_mode::EmployeeCalculationMode::Ta),
/// `SCHEDULE` otherwise). The earnings variant unions in `EmployeeEarning` rows
/// of the configured types, and falls back to the shifts-only query when the
/// caller passes no earning type ids.
///
/// # The cache is not ported
///
/// Java wraps each query in
/// `timeCard.getStatMap().computeIfAbsent(key, supplier).intValue()`. The stat
/// map is the cache and nothing else reads either of those two constants, so
/// this is divergence 15's case again — a memo over a pure function. An
/// implementation that wants it can memoize internally.
pub trait EmployeeShiftConsecutiveDaysPort {
    /// Consecutive days worked up to the dataset start date, counting shifts
    /// only. `getPriorConsecutiveDayWorked`.
    fn prior_consecutive_days_worked(&self, time_card: &dyn TimeCard) -> i32;

    /// The same up to the **calculation** start date, also counting days that
    /// carry an earning of one of `earning_type_ids`.
    /// `getPriorConsecutiveDaysWorkedIncludingEarnings`.
    ///
    /// With no earning type ids this is `getPriorConsecutiveDayWorked`, anchor
    /// date included — Java delegates to it outright.
    fn prior_consecutive_days_worked_including_earnings(
        &self,
        time_card: &dyn TimeCard,
        earning_type_ids: &[i32],
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakePropertyData(HashMap<&'static str, String>);

    impl PropertyDataPort for FakePropertyData {
        fn value(&self, _property_id: i32, key: PropertyDataKey) -> Option<String> {
            self.0.get(key.key()).cloned()
        }
        fn value_as_double(&self, property_id: i32, key: PropertyDataKey) -> Option<f64> {
            self.value(property_id, key)?.parse().ok()
        }
        fn value_as_boolean(&self, property_id: i32, key: PropertyDataKey) -> Option<bool> {
            Some(self.value(property_id, key)?.eq_ignore_ascii_case("true"))
        }
    }

    struct CollectingEarnings(Vec<EmployeeEarning>);

    impl EmployeeEarningPort for CollectingEarnings {
        fn save(&mut self, earning: EmployeeEarning) {
            self.0.push(earning);
        }
        fn remove(&mut self, earning_id: i32) {
            self.0.retain(|e| e.id() != earning_id);
        }
        fn earning_hours_for_period_and_types(
            &self,
            _employee_id: i32,
            _period: &DateRange,
            _earning_type_ids: &[i32],
        ) -> Option<f64> {
            Some(self.0.iter().map(EmployeeEarning::hours).sum())
        }
    }

    #[test]
    fn property_data_keys_carry_their_stored_spelling() {
        assert_eq!(
            PropertyDataKey::PunchRoundingThreshold.key(),
            "punchRoundingThreshold"
        );
        assert_eq!(PropertyDataKey::MaxShiftTa.key(), "maxShiftTA");
    }

    #[test]
    fn an_unset_property_value_is_none() {
        let data = FakePropertyData(HashMap::new());
        assert_eq!(
            data.value(11, PropertyDataKey::PunchRoundingThreshold),
            None
        );
    }

    #[test]
    fn a_property_value_reads_back_in_each_form() {
        let data = FakePropertyData(HashMap::from([
            ("punchRoundingThreshold", "15".to_string()),
            ("validateOvertime", "true".to_string()),
        ]));

        assert_eq!(
            data.value(11, PropertyDataKey::PunchRoundingThreshold),
            Some("15".to_string())
        );
        assert_eq!(
            data.value_as_double(11, PropertyDataKey::PunchRoundingThreshold),
            Some(15.0)
        );
        assert_eq!(
            data.value_as_boolean(11, PropertyDataKey::ValidateOvertime),
            Some(true)
        );
    }

    #[test]
    fn the_rounding_threshold_may_carry_four_comma_separated_values() {
        // PropertyDataRoundingRuleImpl splits on "," and expects four.
        let data = FakePropertyData(HashMap::from([(
            "punchRoundingThreshold",
            "15,10,5,5".to_string(),
        )]));

        let raw = data
            .value(11, PropertyDataKey::PunchRoundingThreshold)
            .unwrap();
        let parts: Vec<_> = raw.split(',').collect();

        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "15");
    }

    #[test]
    fn saving_earnings_collects_them() {
        use crate::common::enums::earning_source::EarningSource;
        let mut port = CollectingEarnings(Vec::new());

        port.save_all(vec![
            EmployeeEarning::new(
                1,
                100,
                200,
                5,
                joda_rs::LocalDate::of(2010, 1, 2),
                8.0,
                12.5,
                EarningSource::Rule,
            ),
            EmployeeEarning::new(
                2,
                100,
                200,
                5,
                joda_rs::LocalDate::of(2010, 1, 3),
                4.0,
                12.5,
                EarningSource::Rule,
            ),
        ]);

        assert_eq!(port.0.len(), 2);

        port.remove(1);
        assert_eq!(port.0.len(), 1);
        assert_eq!(port.0[0].id(), 2);
    }
}
