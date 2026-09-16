//! Port of
//! `com.unifocus.watson.server.labor.calcshift.flsacalculations.FlsaData`.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/calcshift/flsacalculations/FlsaData.java`.
//!
//! One work week's FLSA regular-rate arithmetic, computed once per week and
//! parked on the time card's FLSA map for the rules to read. The regular rate
//! is total straight-time pay divided by hours worked — the figure federal
//! overtime is a multiple of, which is not the same as anybody's hourly rate
//! once tips, commissions and premiums are in the picture.
//!
//! # Read-only, deliberately
//!
//! Java's constructor is private and the only way in is
//! `createFlsaData(timeCard, dateRange)`, which runs four calculators
//! (`TipsCalculator`, `SevenICommissionsCalculator`,
//! `AdditionalFlsaEarningsCalculator`, `HoursAndWageCalculator`) over the
//! period's earnings and shifts. **That calculation is not ported here.** It is
//! not in the rules tree: the calc pipeline builds the map before any rule
//! runs, and the two `hoursdistribution` rules that reach it
//! (`WeeklyOTHrsRuleImpl`, `TwentyFourHourOTRuleImpl`) only ever read
//! [`regular_rate`](FlsaData::regular_rate),
//! [`total_regular_rate_pay`](FlsaData::total_regular_rate_pay) and
//! [`seven_i_commissions`](FlsaData::seven_i_commissions) back out.
//!
//! So this is the value the map holds, with a public constructor taking the
//! computed figures. The calculators arrive if and when a family needs them;
//! flag it if one does.

use joda_rs::LocalDate;

/// One week's FLSA figures. `FlsaData`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlsaData {
    date_range_end_date: LocalDate,
    tips: f64,
    tip_make_up: f64,
    regular_rate_tipped: f64,
    seven_i_commissions: f64,
    total_regular_rate_pay: f64,
    min_wage_make_up: f64,
    total_premium_hours_worked: f64,
    total_hours_worked: f64,
    regular_rate: f64,
    regular_rate_min_wage: f64,
}

impl FlsaData {
    /// The figures for a week, as the calc pipeline computed them.
    ///
    /// Java holds the whole `DateRange` and exposes only its end date; the end
    /// date is also the map key, so that is all that comes across.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        date_range_end_date: LocalDate,
        tips: f64,
        tip_make_up: f64,
        regular_rate_tipped: f64,
        seven_i_commissions: f64,
        total_regular_rate_pay: f64,
        min_wage_make_up: f64,
        total_premium_hours_worked: f64,
        total_hours_worked: f64,
        regular_rate: f64,
        regular_rate_min_wage: f64,
    ) -> Self {
        Self {
            date_range_end_date,
            tips,
            tip_make_up,
            regular_rate_tipped,
            seven_i_commissions,
            total_regular_rate_pay,
            min_wage_make_up,
            total_premium_hours_worked,
            total_hours_worked,
            regular_rate,
            regular_rate_min_wage,
        }
    }

    /// The last day of the week these figures cover — the map key.
    /// `getDateRangeEndDate()`.
    pub fn date_range_end_date(&self) -> LocalDate {
        self.date_range_end_date
    }

    /// `getTips()`.
    pub fn tips(&self) -> f64 {
        self.tips
    }

    /// What had to be added to reach minimum wage once tips are counted.
    /// `getTipMakeUp()`.
    pub fn tip_make_up(&self) -> f64 {
        self.tip_make_up
    }

    /// The regular rate with tips folded in. `getRegularRateTipped()`.
    pub fn regular_rate_tipped(&self) -> f64 {
        self.regular_rate_tipped
    }

    /// Commissions counting toward the FLSA 7(i) retail exemption.
    /// `getSevenICommissions()`.
    pub fn seven_i_commissions(&self) -> f64 {
        self.seven_i_commissions
    }

    /// Straight-time pay for the week, to currency precision.
    /// `getTotalRegularRatePay()`.
    pub fn total_regular_rate_pay(&self) -> f64 {
        self.total_regular_rate_pay
    }

    /// What had to be added to reach minimum wage. `getMinWageMakeUp()`.
    pub fn min_wage_make_up(&self) -> f64 {
        self.min_wage_make_up
    }

    /// `getTotalPremiumHoursWorked()`.
    pub fn total_premium_hours_worked(&self) -> f64 {
        self.total_premium_hours_worked
    }

    /// `getTotalHoursWorked()`.
    pub fn total_hours_worked(&self) -> f64 {
        self.total_hours_worked
    }

    /// Straight-time pay over hours worked. `getRegularRate()`.
    pub fn regular_rate(&self) -> f64 {
        self.regular_rate
    }

    /// The same, computed from minimum-wage pay. `getRegularRateMinWage()`.
    pub fn regular_rate_min_wage(&self) -> f64 {
        self.regular_rate_min_wage
    }

    /// Whichever rate the caller's `atMinWage` switch selects.
    /// `getEffectiveRegularRate(boolean)`.
    pub fn effective_regular_rate(&self, at_min_wage: bool) -> f64 {
        if at_min_wage {
            self.regular_rate_min_wage
        } else {
            self.regular_rate
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn week() -> FlsaData {
        FlsaData::new(
            LocalDate::of(2010, 1, 9),
            40.0,  // tips
            0.0,   // tip make-up
            13.0,  // regular rate tipped
            300.0, // 7(i) commissions
            520.0, // total regular rate pay
            0.0,   // min wage make-up
            2.0,   // premium hours
            40.0,  // hours worked
            13.0,  // regular rate
            7.25,  // regular rate at min wage
        )
    }

    #[test]
    fn the_effective_rate_follows_the_min_wage_switch() {
        assert_eq!(week().effective_regular_rate(false), 13.0);
        assert_eq!(week().effective_regular_rate(true), 7.25);
    }

    #[test]
    fn the_seven_i_exemption_reads_three_figures() {
        // The shape of WeeklyOTHrsRuleImpl.passes7iExemptionTests: commissions
        // over half the straight-time pay, and a rate above time-and-a-half of
        // minimum wage.
        let data = week();
        let half_pay = data.total_regular_rate_pay() * 0.5;

        assert!(data.seven_i_commissions() > half_pay);
        assert!(data.regular_rate() > 7.25 * 1.5);
    }

    #[test]
    fn the_end_date_is_the_map_key() {
        assert_eq!(week().date_range_end_date(), LocalDate::of(2010, 1, 9));
    }
}
