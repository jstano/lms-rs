//! Port of `com.unifocus.watson.common.labor.rules.RuleClass`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/common/labor/rules/RuleClass.java`.
//!
//! The catalogue: 225 configurable rule classes, grouped into the 32
//! [`RuleType`] buckets. Each entry carries
//!
//! * a **code**, which is what the database stores in `RuleItem.RuleClassCode`
//!   and therefore the only identity that must not change;
//! * the **Java constant name**, kept for traceability — the Groovy test tables
//!   and the Java source refer to rule classes this way;
//! * the **config class name**, which is how Java finds the algorithm:
//!   `RuleImplFactory` takes this name, swaps `Config` for `Impl`, decapitalises
//!   it and asks Spring for that bean. Rust dispatches on the enum instead, but
//!   the name is retained because it is the link between a catalogue entry and
//!   the file its algorithm lives in;
//! * the rule type.
//!
//! Java also stores a resource key for display, and its `toString()`
//! reflectively instantiates the config class just to read a name off it. The
//! engine never displays anything, so neither comes across.
//!
//! Variants are `PascalCase` rather than Java's `SCREAMING_SNAKE_CASE`: 225
//! non-camel-case variants would need a blanket lint suppression, and
//! [`java_name`](RuleClass::java_name) preserves the original spelling for
//! checking against the Java.

use crate::rules::rule_type::RuleType;

macro_rules! rule_classes {
    (
        $(
            $variant:ident => ($code:literal, $java_name:literal, $config_class:literal, $rule_type:expr)
        ),+ $(,)?
    ) => {
        /// A configurable rule class. `RuleClass`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum RuleClass {
            $($variant,)+
        }

        impl RuleClass {
            /// Every rule class, in the order Java declares them. `values()`.
            pub const VALUES: &'static [Self] = &[$(Self::$variant,)+];

            /// The persisted code, as stored in `RuleItem.RuleClassCode`.
            /// `getCode()`.
            pub fn code(&self) -> &'static str {
                match self { $(Self::$variant => $code,)+ }
            }

            /// The Java constant name, for cross-checking against the Java
            /// source and its test tables. No Java equivalent — there it is
            /// `name()`.
            pub fn java_name(&self) -> &'static str {
                match self { $(Self::$variant => $java_name,)+ }
            }

            /// The simple name of the Java `*RuleConfig` class.
            /// `getRuleConfigClass().getSimpleName()`.
            pub fn config_class_name(&self) -> &'static str {
                match self { $(Self::$variant => $config_class,)+ }
            }

            /// Which bucket this rule class belongs to. `getRuleType()`.
            pub fn rule_type(&self) -> RuleType {
                match self { $(Self::$variant => $rule_type,)+ }
            }

            /// Look up by persisted code. `RuleClass.fromCode(String)`.
            ///
            /// `None` where Java throws `IllegalArgumentException`.
            pub fn from_code(code: &str) -> Option<Self> {
                match code { $($code => Some(Self::$variant),)+ _ => None }
            }

            /// Look up by Java constant name. Java gets this from `valueOf`.
            pub fn from_java_name(name: &str) -> Option<Self> {
                match name { $($java_name => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

rule_classes! {
    // --- EMPLOYEE_ALERT ---
    ApproachBreakAlert => ("BPP_ALERT", "APPROACH_BREAK_ALERT", "ApproachingBreakPeriodAlertRuleConfig", RuleType::EmployeeAlert),
    ComingInSoonAlert => ("CIS_ALERT", "COMING_IN_SOON_ALERT", "ComingInSoonAlertRuleConfig", RuleType::EmployeeAlert),
    LateInAlert => ("LI_ALERT", "LATE_IN_ALERT", "LateInAlertRuleConfig", RuleType::EmployeeAlert),
    LateOutAlert => ("LO_ALERT", "LATE_OUT_ALERT", "LateOutAlertRuleConfig", RuleType::EmployeeAlert),
    ApproacDailyOtAlert => ("ADOT_ALERT", "APPROAC_DAILY_OT_ALERT", "ApproachingDailyOTAlertRuleConfig", RuleType::EmployeeAlert),
    UnscheduledAlert => ("OCU_ALERT", "UNSCHEDULED_ALERT", "OnClockUnscheduledAlertRuleConfig", RuleType::EmployeeAlert),
    PreScreenEpr => ("PS_ALERT", "PRE_SCREEN_EPR", "PreScreenSurveyRuleConfig", RuleType::EmployeeAlert),

    // --- SHIFT_DIFFERENTIAL ---
    TimeOfDayDifRateDif => ("TDDR_DIF", "TIME_OF_DAY_DIF_RATE_DIF", "TimeOfDayDifferentialRateRuleConfig", RuleType::ShiftDifferential),
    TodPctDifRateDif => ("TDPDR_DIF", "TOD_PCT_DIF_RATE_DIF", "TimeOfDayPctDifferentialRateRuleConfig", RuleType::ShiftDifferential),
    HrsThreshFixedDif => ("HTF_DIF", "HRS_THRESH_FIXED_DIF", "HoursThresholdFixedRuleConfig", RuleType::ShiftDifferential),
    SplitShiftDifRateDif => ("SSDR_DIF", "SPLIT_SHIFT_DIF_RATE_DIF", "SplitShiftDifferentialRateRuleConfig", RuleType::ShiftDifferential),
    BreakPeriodPenaltyDif => ("BPP_DIF", "BREAK_PERIOD_PENALTY_DIF", "BreakPeriodPenaltyRuleConfig", RuleType::ShiftDifferential),
    BreakPerPenaltyHrsDif => ("BPPH_DIF", "BREAK_PER_PENALTY_HRS_DIF", "BreakPeriodPenaltyHoursRuleConfig", RuleType::ShiftDifferential),
    NoDif => ("NO_DIF", "NO_DIF", "NoEarningRuleConfig", RuleType::ShiftDifferential),
    DowFlatDif => ("DOW_FLAT_DIF", "DOW_FLAT_DIF", "DOWFlatRuleConfig", RuleType::ShiftDifferential),
    DowFactorDif => ("DOW_FAC_DIF", "DOW_FACTOR_DIF", "DOWRateFactorRuleConfig", RuleType::ShiftDifferential),
    DowHolidayDif => ("DOWH_DIF", "DOW_HOLIDAY_DIF", "DOWHolidayRuleConfig", RuleType::ShiftDifferential),
    ShiftStartFlatDif => ("SSF_DIF", "SHIFT_START_FLAT_DIF", "ShiftStartTimeFlatRuleConfig", RuleType::ShiftDifferential),
    WklyJobHrsFixedDif => ("WJHF_DIF", "WKLY_JOB_HRS_FIXED_DIF", "WeeklyJobHrsFixedRuleConfig", RuleType::ShiftDifferential),
    PdBreakFixedHrsDif => ("PBFH_DIF", "PD_BREAK_FIXED_HRS_DIF", "PaidBreakFixedHrsRuleConfig", RuleType::ShiftDifferential),
    HolidayFactorDif => ("HF_DIF", "HOLIDAY_FACTOR_DIF", "HolidayRateFactorRuleConfig", RuleType::ShiftDifferential),
    HolidayTodDif => ("HTOD_DIF", "HOLIDAY_TOD_DIF", "HolidayTimeOfDayRuleConfig", RuleType::ShiftDifferential),
    ContractGuaranteeDif => ("CG_DIF", "CONTRACT_GUARANTEE_DIF", "ContractGuaranteeRuleConfig", RuleType::ShiftDifferential),
    ContractToFullDif => ("CTF_DIF", "CONTRACT_TO_FULL_DIF", "ContractToFullTimeRuleConfig", RuleType::ShiftDifferential),
    StartEndBetweenDif => ("SEB_DIF", "START_END_BETWEEN_DIF", "StartEndBetweenPremiumRuleConfig", RuleType::ShiftDifferential),
    MinimumHoursWorkedShiftDif => ("MHWS_DIF", "MINIMUM_HOURS_WORKED_SHIFT_DIF", "MinimumHoursWorkedShiftDifferentialRuleConfig", RuleType::ShiftDifferential),
    DailySpreadShiftDif => ("DSS_DIF", "DAILY_SPREAD_SHIFT_DIF", "DailySpreadShiftDifferentialRuleConfig", RuleType::ShiftDifferential),
    HrsThreshFixedWithMaxCapDif => ("HTFWMC_DIF", "HRS_THRESH_FIXED_WITH_MAX_CAP_DIF", "HoursThresholdFixedWithMaxCapRuleConfig", RuleType::ShiftDifferential),
    AnnWrkdDif => ("ANNW_DIF", "ANN_WRKD_DIF", "AnniversaryWorkedShiftDifferentialRuleConfig", RuleType::ShiftDifferential),

    // --- DAILY_DIFFERENTIAL ---
    NoDdf => ("NO_DDF", "NO_DDF", "NoDailyEarningRuleConfig", RuleType::DailyDifferential),
    HolidayDifDdf => ("HD_DDF", "HOLIDAY_DIF_DDF", "HolidayDifferentialRuleConfig", RuleType::DailyDifferential),
    MinHoursWorkedDdf => ("MHW_DDF", "MIN_HOURS_WORKED_DDF", "MinimumHoursWorkedDifferentialRuleConfig", RuleType::DailyDifferential),
    DaysWorkedDdf => ("DW_DDF", "DAYS_WORKED_DDF", "DaysWorkedDailyDifferentialRuleConfig", RuleType::DailyDifferential),
    MinDailyHrsDdf => ("MDH_DDF", "MIN_DAILY_HRS_DDF", "MinDailyHoursEarningRuleConfig", RuleType::DailyDifferential),
    NoDailyHrsDdf => ("NDHWL_DDF", "NO_DAILY_HRS_DDF", "NoDailyHoursWithLabelEarningRuleConfig", RuleType::DailyDifferential),
    MinDailyWrkdHrsDdf => ("MDWH_DDF", "MIN_DAILY_WRKD_HRS_DDF", "MinDailyWorkedHoursEarningRuleConfig", RuleType::DailyDifferential),
    DlySalAllowDdf => ("DLY_DDF", "DLY_SAL_ALLOW_DDF", "DailySalariedAllowanceRuleConfig", RuleType::DailyDifferential),
    AnniDdf => ("ANNI_DDF", "ANNI_DDF", "AnniversaryEarningRuleConfig", RuleType::DailyDifferential),

    // --- WEEKLY_DIFFERENTIAL ---
    NoWdf => ("NO_WDF", "NO_WDF", "NoWeeklyEarningRuleConfig", RuleType::WeeklyDifferential),
    WklyHrsFixedWdf => ("WHF_WDF", "WKLY_HRS_FIXED_WDF", "WeeklyHrsFixedRuleConfig", RuleType::WeeklyDifferential),
    DowWeeklyHrsFactorWdf => ("DOWWHRF_WDF", "DOW_WEEKLY_HRS_FACTOR_WDF", "DOWWeeklyHrsRateFactorRuleConfig", RuleType::WeeklyDifferential),
    DowHrsFixedWdf => ("DOWF_WDF", "DOW_HRS_FIXED_WDF", "DOWHrsFixedRuleConfig", RuleType::WeeklyDifferential),
    DailySpreadWdf => ("DS_WDF", "DAILY_SPREAD_WDF", "DailySpreadRuleConfig", RuleType::WeeklyDifferential),
    SalaryMinWageWdf => ("SMW_WDF", "SALARY_MIN_WAGE_WDF", "SalariedMinWageRuleConfig", RuleType::WeeklyDifferential),
    HrsThreshFixedWdf => ("HTF_WDF", "HRS_THRESH_FIXED_WDF", "HoursThresholdFixedWeeklyRuleConfig", RuleType::WeeklyDifferential),
    DailyWeeklyGuaranteedHrsWdf => ("DWGH_WDF", "DAILY_WEEKLY_GUARANTEED_HRS_WDF", "DailyWeeklyGuaranteedHoursRuleConfig", RuleType::WeeklyDifferential),

    // --- TIME_OFF_EARNING ---
    NoToe => ("NO_TOE", "NO_TOE", "NoTimeOffEarningRuleConfig", RuleType::TimeOffEarning),
    AccrualPriorityToe => ("AP_TOE", "ACCRUAL_PRIORITY_TOE", "AccrualPriorityTOERuleConfig", RuleType::TimeOffEarning),
    OspSspToe => ("OSP_SSP_TOE", "OSP_SSP_TOE", "OspSspTOERuleConfig", RuleType::TimeOffEarning),

    // --- SHIFT_ADJUST ---
    MinBreakSad => ("MBK_SAD", "MIN_BREAK_SAD", "MinBreakRuleConfig", RuleType::ShiftAdjust),
    MinDailyHrsSad => ("MDH_SAD", "MIN_DAILY_HRS_SAD", "MinDailyHrsRuleConfig", RuleType::ShiftAdjust),
    AutoBreakSad => ("ABK_SAD", "AUTO_BREAK_SAD", "AutoBreakRuleConfig", RuleType::ShiftAdjust),
    DstAdjustmentSad => ("DST_SAD", "DST_ADJUSTMENT_SAD", "DSTAdjustmentRuleConfig", RuleType::ShiftAdjust),
    PaidBreakSad => ("PBK_SAD", "PAID_BREAK_SAD", "PaidBreakRuleConfig", RuleType::ShiftAdjust),
    NoSad => ("NO_SAD", "NO_SAD", "NoAdjustmentRuleConfig", RuleType::ShiftAdjust),
    TotalBreakLengthSad => ("TBL_SAD", "TOTAL_BREAK_LENGTH_SAD", "TotalBreakLengthRuleConfig", RuleType::ShiftAdjust),

    // --- HOURS_DISTRIBUTION ---
    RegOnlyHdr => ("REG_ONLY_HDR", "REG_ONLY_HDR", "RegHrsOnlyRuleConfig", RuleType::HoursDistribution),
    RollingXWeeksOtHdr => ("RXW_OT_HDR", "ROLLING_X_WEEKS_OT_HDR", "RollingXWeeksOTHrsRuleConfig", RuleType::HoursDistribution),
    WeeklyOtHdr => ("WKLY_OT_HDR", "WEEKLY_OT_HDR", "WeeklyOTHrsRuleConfig", RuleType::HoursDistribution),
    WeeklyOtSecJobHdr => ("WOTSJ_HDR", "WEEKLY_OT_SEC_JOB_HDR", "WeeklyOTSecJobHrsRuleConfig", RuleType::HoursDistribution),
    CaliforniaHdr => ("CAL_HDR", "CALIFORNIA_HDR", "CaliforniaOTHrsRuleConfig", RuleType::HoursDistribution),
    CaExtHdr => ("CAL_EXT_HDR", "CA_EXT_HDR", "CaliforniaExtendedOTHrsRuleConfig", RuleType::HoursDistribution),
    CaExtSpJobHdr => ("CA_SPJOB_HDR", "CA_EXT_SP_JOB_HDR", "CaliforniaExtSpecialJobOTHrsRuleConfig", RuleType::HoursDistribution),
    Dw6ot7dtHdr => ("DW6OT7DT_HDR", "DW_6OT_7DT_HDR", "DailyWeekly6thOT7thDTHrsRuleConfig", RuleType::HoursDistribution),
    DwocotMinBrHdr => ("DWOCOT_MIN_BR_HDR", "DWOCOT_MIN_BR_HDR", "DlyWklyOffConsecOTMinBreakRuleConfig", RuleType::HoursDistribution),
    DwcotMinBrSpanMnHdr => ("DWCOT_MIN_BR_SPAN_MN_HDR", "DWCOT_MIN_BR_SPAN_MN_HDR", "DlyWklyConsecOTMinBreakSpanningMidnightRuleConfig", RuleType::HoursDistribution),
    Dw7dtHdr => ("DW7DT_HDR", "DW_7DT_HDR", "DailyWeekly7thDTHrsRuleConfig", RuleType::HoursDistribution),
    Dw6ot7dtNcsHdr => ("DW6OT7DTNCS_HDR", "DW_6OT_7DT_NCS_HDR", "DailyWeekly6thDayOT7thDayDTNonConsecRuleConfig", RuleType::HoursDistribution),
    PerMonthOtHdr => ("PER_MONTH_OT_HDR", "PER_MONTH_OT_HDR", "PerMonthOTHrsRuleConfig", RuleType::HoursDistribution),
    ContractOtHdr => ("CONTRACT_OT_HDR", "CONTRACT_OT_HDR", "ContractOTHrsRuleConfig", RuleType::HoursDistribution),
    HolidayDtHdr => ("HOLIDAY_DT_HDR", "HOLIDAY_DT_HDR", "HolidayDTHrsRuleConfig", RuleType::HoursDistribution),
    ScheduledShiftOtHdr => ("SSOT_HDR", "SCHEDULED_SHIFT_OT_HDR", "ScheduledShiftOTRuleConfig", RuleType::HoursDistribution),
    TwentyFourHourHdr => ("TFH_HDR", "TWENTY_FOUR_HOUR_HDR", "TwentyFourHourOTRuleConfig", RuleType::HoursDistribution),
    MinHrsFullTimeOtHdr => ("MHFFTOT_HDR", "MIN_HRS_FULL_TIME_OT_HDR", "MinHrsForFullTimeOTRuleConfig", RuleType::HoursDistribution),

    // --- EARNING_RATE ---
    HomeJobErr => ("HOME_JOB_ERR", "HOME_JOB_ERR", "HomeJobRateRuleConfig", RuleType::EarningRate),
    FlsaErr => ("FLSA_ERR", "FLSA_ERR", "FLSAEarningRateRuleConfig", RuleType::EarningRate),
    FixedRateErr => ("FIXED_RATE_ERR", "FIXED_RATE_ERR", "EarningFixedRateRuleConfig", RuleType::EarningRate),
    HomeDeptErr => ("HOME_DEPT_ERR", "HOME_DEPT_ERR", "HomeDeptRateRuleConfig", RuleType::EarningRate),
    ContractDailyErr => ("CONTRACT_DAILY_ERR", "CONTRACT_DAILY_ERR", "ContractDailyRateRuleConfig", RuleType::EarningRate),
    AvgXWeeksWorkedErr => ("AXWW_ERR", "AVG_X_WEEKS_WORKED_ERR", "AvgDayXWeeksRateRuleConfig", RuleType::EarningRate),
    EarnOverrideJobErr => ("EARN_OVERRIDE_JOB_ERR", "EARN_OVERRIDE_JOB_ERR", "EarningOverrideJobRateRuleConfig", RuleType::EarningRate),
    FactorErr => ("FACTOR_ERR", "FACTOR_ERR", "EarningFactorRateRuleConfig", RuleType::EarningRate),
    PriorBalancesErr => ("PRIOR_BALANCES_ERR", "PRIOR_BALANCES_ERR", "PriorBalancesRateRuleConfig", RuleType::EarningRate),
    CalcAccrualErr => ("CALC_ACCRUAL_ERR", "CALC_ACCRUAL_ERR", "CalculatedAccrualRateRuleConfig", RuleType::EarningRate),

    // --- REGULAR_RATE ---
    JobRrr => ("JOB_RRR", "JOB_RRR", "JobRegRateRuleConfig", RuleType::RegularRate),
    HomeDeptRrr => ("HOME_DEPT_RRR", "HOME_DEPT_RRR", "HomeDeptRegRateRuleConfig", RuleType::RegularRate),
    HomeJobRrr => ("HOME_JOB_RRR", "HOME_JOB_RRR", "HomeJobRegRateRuleConfig", RuleType::RegularRate),
    AsohwRrr => ("ASOHW_RRR", "ASOHW_RRR", "AnnualSalaryOverHoursRegRateRuleConfig", RuleType::RegularRate),
    ScmwRrr => ("SCMW_RRR", "SCMW_RRR", "ShiftCategoryMinWageRegRateRuleConfig", RuleType::RegularRate),
    FactorJobRrr => ("FAC_JOB_RRR", "FACTOR_JOB_RRR", "FactorJobRegRateRuleConfig", RuleType::RegularRate),
    CombinationJobRrr => ("COMBINATION_JOB_RRR", "COMBINATION_JOB_RRR", "CombinationJobsRegRateRuleConfig", RuleType::RegularRate),
    ShiftCatRrr => ("SHIFT_CAT_RRR", "SHIFT_CAT_RRR", "ShiftCategoryRegRateRuleConfig", RuleType::RegularRate),

    // --- OVERTIME_RATE ---
    GuaranteedWageOrr => ("GUARANTEED_WAGE_ORR", "GUARANTEED_WAGE_ORR", "GuaranteedWageOTRateRuleConfig", RuleType::OvertimeRate),
    JobOrr => ("JOB_ORR", "JOB_ORR", "JobOTRateRuleConfig", RuleType::OvertimeRate),
    HomeDeptOrr => ("HOME_DEPT_ORR", "HOME_DEPT_ORR", "HomeDeptOTRateRuleConfig", RuleType::OvertimeRate),
    HomeJobOrr => ("HOME_JOB_ORR", "HOME_JOB_ORR", "HomeJobOTRateRuleConfig", RuleType::OvertimeRate),
    FlsaOrr => ("FLSA_ORR", "FLSA_ORR", "FLSAOTRateRuleConfig", RuleType::OvertimeRate),
    FlsaWeightedOrr => ("FLSA_W_ORR", "FLSA_WEIGHTED_ORR", "FLSAWeightedOTRateRuleConfig", RuleType::OvertimeRate),
    WeightedOrr => ("WEIGHTED_ORR", "WEIGHTED_ORR", "WeightedOTRateRuleConfig", RuleType::OvertimeRate),
    ComBasedOrr => ("COM_BASE_ORR", "COM_BASED_ORR", "CommissionBasedOTRateRuleConfig", RuleType::OvertimeRate),

    // --- DOUBLE_TIME_RATE ---
    JobDrr => ("JOB_DRR", "JOB_DRR", "JobDTRateRuleConfig", RuleType::DoubleTimeRate),
    HomeDeptDrr => ("HOME_DEPT_DRR", "HOME_DEPT_DRR", "HomeDeptDTRateRuleConfig", RuleType::DoubleTimeRate),
    HomeJobDrr => ("HOME_JOB_DRR", "HOME_JOB_DRR", "HomeJobDTRateRuleConfig", RuleType::DoubleTimeRate),
    FlsaDrr => ("FLSA_DRR", "FLSA_DRR", "FLSADTRateRuleConfig", RuleType::DoubleTimeRate),
    ComBasedDrr => ("COM_BASE_DRR", "COM_BASED_DRR", "CommissionBasedDTRateRuleConfig", RuleType::DoubleTimeRate),

    // --- HOLIDAY_ELIGIBILITY ---
    NoHer => ("NO_HER", "NO_HER", "NoHolidayRuleConfig", RuleType::HolidayEligibility),
    FixHrsHer => ("FIX_HRS_HER", "FIX_HRS_HER", "HolidayFixedHoursRuleConfig", RuleType::HolidayEligibility),
    NetHrsFactorHer => ("NHF_HER", "NET_HRS_FACTOR_HER", "HolidayHrsFactorHolidayRuleConfig", RuleType::HolidayEligibility),
    AvgHrsHer => ("AVG_HRS_HER", "AVG_HRS_HER", "AvgHrsRuleConfig", RuleType::HolidayEligibility),
    AvgHrsWithWorkedBeforeAndAfterHer => ("AVG_HRS_WITH_WORKED_BEFORE_AND_AFTER_HER", "AVG_HRS_WITH_WORKED_BEFORE_AND_AFTER_HER", "AvgHrsWithBeforeAndAfterRuleConfig", RuleType::HolidayEligibility),
    MinMonthlyHrsHer => ("MIN_MTH_HRS_HER", "MIN_MONTHLY_HRS_HER", "MinMonthlyHrsRuleConfig", RuleType::HolidayEligibility),
    PercentOfPayHer => ("PERCENT_OF_PAY_HER", "PERCENT_OF_PAY_HER", "PercentOfPayRuleConfig", RuleType::HolidayEligibility),
    MinDaysPaidHer => ("MIN_DAYS_PAID_HER", "MIN_DAYS_PAID_HER", "MinDaysPaidRuleConfig", RuleType::HolidayEligibility),
    MinDaysWorkedHer => ("MIN_DAYS_WORKED_HER", "MIN_DAYS_WORKED_HER", "MinDaysRuleConfig", RuleType::HolidayEligibility),
    AvgWeeklyHoursHer => ("AVG_WEEKLY_HOURS_HER", "AVG_WEEKLY_HOURS_HER", "AvgWeeklyHoursRuleConfig", RuleType::HolidayEligibility),
    NetMinMaxHoursWorkedHer => ("NET_MIN_MAX_HOURS_WORKED_HER", "NET_MIN_MAX_HOURS_WORKED_HER", "HolidayNetMinMaxHrsWorkedRuleConfig", RuleType::HolidayEligibility),
    AvgHrsMinMaxHoursWorkedHer => ("AVG_HRS_MIN_MAX_HOURS_WORKED_HER", "AVG_HRS_MIN_MAX_HOURS_WORKED_HER", "HolidayAvgHrsMinMaxHrsWorkedRuleConfig", RuleType::HolidayEligibility),

    // --- PUNCH_VALIDATION ---
    NoIpvr => ("NO_IPVR", "NO_IPVR", "NoInPunchValidationRuleConfig", RuleType::PunchValidation),
    SchLockoutIpvr => ("SL_IPVR", "SCH_LOCKOUT_IPVR", "SchedLockoutInPunchValidationRuleConfig", RuleType::PunchValidation),
    SchLockoutOpvr => ("SL_OPVR", "SCH_LOCKOUT_OPVR", "SchedLockoutOutPunchValidationRuleConfig", RuleType::PunchValidation),
    SchLockAllowUnIpvr => ("SLUA_IPVR", "SCH_LOCK_ALLOW_UN_IPVR", "SchedLockAllowUnschedIPVRuleConfig", RuleType::PunchValidation),

    // --- RECONCILE_EMPLOYEE ---
    MissSalDistRer => ("MSD_RER", "MISS_SAL_DIST_RER", "MissingSalaryDistRuleConfig", RuleType::ReconcileEmployee),
    AnnualRateManipulationRer => ("ARM_RER", "ANNUAL_RATE_MANIPULATION_RER", "ManipulateAnnualRateRuleConfig", RuleType::ReconcileEmployee),

    // --- SHIFT_CORRECTION ---
    OutPunchScr => ("OP_SCR", "OUT_PUNCH_SCR", "OutPunchCorrectionRuleConfig", RuleType::ShiftCorrection),
    ShiftCategoryScr => ("SC_SCR", "SHIFT_CATEGORY_SCR", "ShiftCategoryCorrectionRuleConfig", RuleType::ShiftCorrection),
    SplitToScheduleScr => ("STS_SCR", "SPLIT_TO_SCHEDULE_SCR", "SplitToScheduleCorrectionRuleConfig", RuleType::ShiftCorrection),

    // --- SCHEDULE_LUNCH ---
    LunchAdjustEndTime => ("SL_ADJUST_END_TIME", "LUNCH_ADJUST_END_TIME", "LunchAdjustEndTimeRuleConfig", RuleType::ScheduleLunch),
    LunchSimple => ("SL_SIMPLE", "LUNCH_SIMPLE", "LunchSimpleRuleConfig", RuleType::ScheduleLunch),
    LunchStartTimeAndLength => ("SL_START_TIME_AND_LENGTH", "LUNCH_START_TIME_AND_LENGTH", "LunchStartTimeAndLengthRuleConfig", RuleType::ScheduleLunch),

    // --- POST_CALC ---
    ApproveBySchedPcr => ("ABS_PCR", "APPROVE_BY_SCHED_PCR", "ApproveByScheduleRuleConfig", RuleType::PostCalc),
    MinWageCompliancePcr => ("MWC_PCR", "MIN_WAGE_COMPLIANCE_PCR", "MinWageComplianceRuleConfig", RuleType::PostCalc),
    TipMakeUpPcr => ("TMU_PCR", "TIP_MAKE_UP_PCR", "TipMakeUpRuleConfig", RuleType::PostCalc),
    ContractGuaranteePcr => ("CG_PCR", "CONTRACT_GUARANTEE_PCR", "ContractGuaranteePostCalcRuleConfig", RuleType::PostCalc),
    BreakPeriodPenaltyPcr => ("BPP_PCR", "BREAK_PERIOD_PENALTY_PCR", "BreakPeriodPenaltyPostCalcRuleConfig", RuleType::PostCalc),
    BreakPeriodPenaltyGroupByDayPcr => ("BPP_GBD_PCR", "BREAK_PERIOD_PENALTY_GROUP_BY_DAY_PCR", "BreakPeriodPenaltyGroupByDayPostCalcRuleConfig", RuleType::PostCalc),
    GtdHoursInPayPeriod => ("GTD_HPP", "GTD_HOURS_IN_PAY_PERIOD", "GuaranteedHoursInPayPeriodRuleConfig", RuleType::PostCalc),

    // --- EMPLOYEE_EVENT ---
    NoEer => ("NO_EER", "NO_EER", "NoEmpEventRuleConfig", RuleType::EmployeeEvent),
    PvEer => ("PV_EER", "PV_EER", "PointsViolationEventRuleConfig", RuleType::EmployeeEvent),
    PaEer => ("PA_EER", "PA_EER", "PointsAwardEventRuleConfig", RuleType::EmployeeEvent),

    // --- EMPLOYEE_POINTS ---
    NoEpr => ("NO_EPR", "NO_EPR", "NoEmpPointsRuleConfig", RuleType::EmployeePoints),
    MissingPunchEpr => ("MP_EPR", "MISSING_PUNCH_EPR", "MissingPunchRuleConfig", RuleType::EmployeePoints),
    LongShiftEpr => ("LS_EPR", "LONG_SHIFT_EPR", "LongShiftRuleConfig", RuleType::EmployeePoints),
    ShortShiftEpr => ("SS_EPR", "SHORT_SHIFT_EPR", "ShortShiftRuleConfig", RuleType::EmployeePoints),
    AbsentEpr => ("AB_EPR", "ABSENT_EPR", "AbsentRuleConfig", RuleType::EmployeePoints),
    AbsentSimpleEpr => ("ABS_EPR", "ABSENT_SIMPLE_EPR", "AbsentSimpleRuleConfig", RuleType::EmployeePoints),
    NotScheduledEpr => ("NS_EPR", "NOT_SCHEDULED_EPR", "NotScheduledRuleConfig", RuleType::EmployeePoints),
    EarlyInEpr => ("EI_EPR", "EARLY_IN_EPR", "EarlyInRuleConfig", RuleType::EmployeePoints),
    LateInEpr => ("LI_EPR", "LATE_IN_EPR", "LateInRuleConfig", RuleType::EmployeePoints),
    EarlyOutEpr => ("EO_EPR", "EARLY_OUT_EPR", "EarlyOutRuleConfig", RuleType::EmployeePoints),
    LateOutEpr => ("LO_EPR", "LATE_OUT_EPR", "LateOutRuleConfig", RuleType::EmployeePoints),
    NoBreakEpr => ("NB_EPR", "NO_BREAK_EPR", "NoBreakRuleConfig", RuleType::EmployeePoints),
    LongBreakEpr => ("LB_EPR", "LONG_BREAK_EPR", "LongBreakRuleConfig", RuleType::EmployeePoints),
    ShortBreakEpr => ("SB_EPR", "SHORT_BREAK_EPR", "ShortBreakRuleConfig", RuleType::EmployeePoints),
    DailyHoursEpr => ("DH_EPR", "DAILY_HOURS_EPR", "DailyHoursRuleConfig", RuleType::EmployeePoints),
    WeeklyHoursEpr => ("WH_EPR", "WEEKLY_HOURS_EPR", "WeeklyHoursRuleConfig", RuleType::EmployeePoints),
    TimeOfDayEpr => ("TOD_EPR", "TIME_OF_DAY_EPR", "TimeOfDayPointsRuleConfig", RuleType::EmployeePoints),
    OvertimeHoursPeriodEpr => ("OHP_EPR", "OVERTIME_HOURS_PERIOD_EPR", "OvertimeHoursPeriodRuleConfig", RuleType::EmployeePoints),

    // --- BENEFIT_ACCRUAL ---
    NoBar => ("NO_BAR", "NO_BAR", "NoBenefitAccrualRuleConfig", RuleType::BenefitAccrual),
    AnniversaryYearBar => ("AY_BAR", "ANNIVERSARY_YEAR_BAR", "AnniversaryYearAccrualRuleConfig", RuleType::BenefitAccrual),
    AnnHolEntitlementBar => ("AHE_BAR", "ANN_HOL_ENTITLEMENT_BAR", "AnnualHolidayEntitlementRuleConfig", RuleType::BenefitAccrual),
    DayOfMonthBar => ("DM_BAR", "DAY_OF_MONTH_BAR", "DayOfMonthAccrualRuleConfig", RuleType::BenefitAccrual),
    DayOfYearBar => ("DY_BAR", "DAY_OF_YEAR_BAR", "DayOfYearAccrualRuleConfig", RuleType::BenefitAccrual),
    FixedPeriodBar => ("FP_BAR", "FIXED_PERIOD_BAR", "FixedPeriodAccrualRuleConfig", RuleType::BenefitAccrual),
    CalcCostsBar => ("CALC_COSTS_BAR", "CALC_COSTS_BAR", "CalculatedCostsAccrualRuleConfig", RuleType::BenefitAccrual),
    HourlyBar => ("HY_BAR", "HOURLY_BAR", "HourlyAccrualRuleConfig", RuleType::BenefitAccrual),
    HoursDistBar => ("HRS_DIST_BAR", "HOURS_DIST_BAR", "HoursDistributionAccrualRuleConfig", RuleType::BenefitAccrual),
    HolidayBar => ("HDY_BAR", "HOLIDAY_BAR", "HolidayAccrualRuleConfig", RuleType::BenefitAccrual),
    WeeklyRestDaysBar => ("WRD_BAR", "WEEKLY_REST_DAYS_BAR", "WeeklyRestDaysAccrualRuleConfig", RuleType::BenefitAccrual),
    HolidayTodBar => ("HTOD_BAR", "HOLIDAY_TOD_BAR", "HolidayTimeOfDayAccrualRuleConfig", RuleType::BenefitAccrual),
    HolidayWrkdBar => ("HDY_WRK_BAR", "HOLIDAY_WRKD_BAR", "HolidayWorkedAccrualRuleConfig", RuleType::BenefitAccrual),
    GrossWagesBar => ("GRS_WG_BAR", "GROSS_WAGES_BAR", "GrossWagesAccrualRuleConfig", RuleType::BenefitAccrual),
    BirthdayBar => ("BDAY_BAR", "BIRTHDAY_BAR", "BirthdayAccrualRuleConfig", RuleType::BenefitAccrual),

    // --- BENEFIT_EXPIRATION ---
    AnniversaryYearBer => ("AY_BER", "ANNIVERSARY_YEAR_BER", "AnniversaryYearBERuleConfig", RuleType::BenefitExpiration),
    PriorYearFixedDateBer => ("PYFD_BER", "PRIOR_YEAR_FIXED_DATE_BER", "PriorYearFixedDateBERuleConfig", RuleType::BenefitExpiration),
    CalendarYearBer => ("CY_BER", "CALENDAR_YEAR_BER", "CalendarYearBERuleConfig", RuleType::BenefitExpiration),
    UnusedCreditBer => ("UC_BER", "UNUSED_CREDIT_BER", "UnusedCreditBERuleConfig", RuleType::BenefitExpiration),
    UnusedDebitBer => ("UD_BER", "UNUSED_DEBIT_BER", "UnusedDebitBERuleConfig", RuleType::BenefitExpiration),
    MinWorkedPriorAnnivYearTransBer => ("MWPAYT_BER", "MIN_WORKED_PRIOR_ANNIV_YEAR_TRANS_BER", "MinWorkedPriorAnniversaryYearTransferBERuleConfig", RuleType::BenefitExpiration),
    PriorPayPeriodBer => ("PPP_BER", "PRIOR_PAY_PERIOD_BER", "PriorPayPeriodBERuleConfig", RuleType::BenefitExpiration),

    // --- PUNCH_ROUNDING ---
    PropertyDataPrr => ("PD_PRR", "PROPERTY_DATA_PRR", "PropertyDataRoundingRuleConfig", RuleType::PunchRounding),
    MinutePrr => ("MIN_PRR", "MINUTE_PRR", "MinuteRoundingRuleConfig", RuleType::PunchRounding),
    WorkedHoursPrr => ("WHR_PRR", "WORKED_HOURS_PRR", "WorkedHoursRoundingRuleConfig", RuleType::PunchRounding),
    InToSchedPrr => ("ITS_PRR", "IN_TO_SCHED_PRR", "RoundInToScheduleRuleConfig", RuleType::PunchRounding),
    OutToSchedPrr => ("OTS_PRR", "OUT_TO_SCHED_PRR", "RoundOutToScheduleRuleConfig", RuleType::PunchRounding),
    BackGracePrr => ("BG_PRR", "BACK_GRACE_PRR", "BackFromBreakWithGraceRuleConfig", RuleType::PunchRounding),

    // --- CLOSE_PAY_PERIOD ---
    NoCpr => ("NO_CPR", "NO_CPR", "NoCPRuleConfig", RuleType::ClosePayPeriod),
    PrepayContractCpr => ("PC_CPR", "PREPAY_CONTRACT_CPR", "PrepayContractCPRuleConfig", RuleType::ClosePayPeriod),

    // --- MEAL_PUNCH ---
    NoMpr => ("NO_MPR", "NO_MPR", "NoMealPunchRuleConfig", RuleType::MealPunch),
    FixedEarningMpr => ("FE_MPR", "FIXED_EARNING_MPR", "FixedMealEarningRuleConfig", RuleType::MealPunch),

    // --- POST_PUNCH ---
    NoOpPpr => ("NO_PPR", "NO_OP_PPR", "NoOpPostPunchRuleConfig", RuleType::PostPunch),
    PromptFixedHrsEarnPpr => ("PFHE_PPR", "PROMPT_FIXED_HRS_EARN_PPR", "PromptForFixedHoursEarningRuleConfig", RuleType::PostPunch),
    UnderReportedTipsOnOutPpr => ("URTOO_PPR", "UNDER_REPORTED_TIPS_ON_OUT_PPR", "UnderReportedTipsOnOutRuleConfig", RuleType::PostPunch),
    CertExpirationPpr => ("CERT_EXPIRATION_PPR", "CERT_EXPIRATION_PPR", "CertificationExpirationRuleConfig", RuleType::PostPunch),
    MissingMealBreakPpr => ("MISSING_MEAL_BREAK_PPR", "MISSING_MEAL_BREAK_PPR", "MissingMealBreakRuleConfig", RuleType::PostPunch),
    AssignmentSelectionPpr => ("ASSIGNMENT_SELECTION_PPR", "ASSIGNMENT_SELECTION_PPR", "AssignmentSelectionRuleConfig", RuleType::PostPunch),
    ShortBreakPpr => ("SHORT_BREAK_PPR", "SHORT_BREAK_PPR", "ShortBreakPostPunchRuleConfig", RuleType::PostPunch),
    MissingBreakPpr => ("MISSING_BREAK_PPR", "MISSING_BREAK_PPR", "MissingBreakPostPunchRuleConfig", RuleType::PostPunch),
    ScheduleValidationAttestationPpr => ("SCHEDULE_VALIDATION_ATTESTATION_PPR", "SCHEDULE_VALIDATION_ATTESTATION_PPR", "ScheduleValidationAttestationPostPunchRuleConfig", RuleType::PostPunch),

    // --- SCHEDULE_RESTRICTION ---
    NoSrr => ("NO_SRR", "NO_SRR", "NoRestrictionRuleConfig", RuleType::ScheduleRestriction),
    MonthlyRequiredDaysOffSrr => ("MRDO_SSR", "MONTHLY_REQUIRED_DAYS_OFF_SRR", "MonthlyRequiredDaysOffRuleConfig", RuleType::ScheduleRestriction),
    MaxHoursOnDaySrr => ("MHOD_SSR", "MAX_HOURS_ON_DAY_SRR", "MaxHoursOnDayRuleConfig", RuleType::ScheduleRestriction),
    MaxHoursPerWeekSsr => ("MHPW_SSR", "MAX_HOURS_PER_WEEK_SSR", "MaxHoursPerWeekRuleConfig", RuleType::ScheduleRestriction),
    EarliestStartLatestEndSrr => ("ESLE_SSR", "EARLIEST_START_LATEST_END_SRR", "EarliestStartLatestEndTimeRuleConfig", RuleType::ScheduleRestriction),
    MaxDaysWorkedPerWeek => ("MDWPW_SSR", "MAX_DAYS_WORKED_PER_WEEK", "MaxDaysWorkedPerWeekRuleConfig", RuleType::ScheduleRestriction),

    // --- TIME_OFF_DISTRIBUTION ---
    NoToed => ("NO_TOED", "NO_TOED", "NoTimeOffEarningDistributionRuleConfig", RuleType::TimeOffDistribution),
    FrontLoadToed => ("FL_TOED", "FRONT_LOAD_TOED", "FrontLoadTimeOffDistributionRuleConfig", RuleType::TimeOffDistribution),
    EvenSpreadToed => ("ES_TOED", "EVEN_SPREAD_TOED", "EvenSpreadTimeOffDistributionRuleConfig", RuleType::TimeOffDistribution),

    // --- REGULAR_HOURS_DISTRIBUTION ---
    RegHoursRhd => ("D_RHD", "REG_HOURS_RHD", "RegularHoursOnShiftDateRuleConfig", RuleType::RegularHoursDistribution),
    RegHoursByWorkWeekRhd => ("REG_HRS_WRK_WK_RHD", "REG_HOURS_BY_WORK_WEEK_RHD", "RegularHoursByWorkWeekRuleConfig", RuleType::RegularHoursDistribution),
    RegHoursByDayRhd => ("REG_HRS_DAY_RHD", "REG_HOURS_BY_DAY_RHD", "RegularHoursByDayRuleConfig", RuleType::RegularHoursDistribution),

    // --- SCHEDULE_LABEL ---
    NoLabelLb => ("NO_LABEL_LB", "NO_LABEL_LB", "NoLabelRuleConfig", RuleType::ScheduleLabel),
    UnavailLb => ("UNAVAIL_LB", "UNAVAIL_LB", "UnavailableAllDayRuleConfig", RuleType::ScheduleLabel),
    AvailNotSchedLb => ("AVAIL_NOT_SCHED_LB", "AVAIL_NOT_SCHED_LB", "AvailableNotScheduledRuleConfig", RuleType::ScheduleLabel),
    NotSchedLb => ("NOT SCHED LB", "NOT_SCHED_LB", "NotScheduledOrOffRuleConfig", RuleType::ScheduleLabel),
    ApprovedTimeOffLb => ("APPROVED_TIME_OFF_LB", "APPROVED_TIME_OFF_LB", "ApprovedTimeOffLabelRuleConfig", RuleType::ScheduleLabel),

    // --- SCHEDULE_LABEL_ADDENDUM ---
    NoOpSchAddnm => ("NO_OP_SCH_CNT", "NO_OP_SCH_ADDNM", "NoOpScheduleLabelAddendumRuleConfig", RuleType::ScheduleLabelAddendum),
    BtwnDtsLblCntSchAddnm => ("BTWN_DTS_LBL_CNT_SCH_ADDNM", "BTWN_DTS_LBL_CNT_SCH_ADDNM", "BetweenDatesScheduleLabelCountRuleConfig", RuleType::ScheduleLabelAddendum),
    AccrualBalanceSchAddnm => ("ACCRUAL_BALANCE_SCH_ADDNM", "ACCRUAL_BALANCE_SCH_ADDNM", "AccrualBalanceLabelAddendumRuleConfig", RuleType::ScheduleLabelAddendum),

    // --- SCHEDULE_CHANGE_VALIDATION ---
    AddHoursSchedValidation => ("ADD_HOURS_SCHED_VALIDATION", "ADD_HOURS_SCHED_VALIDATION", "AddHoursPenaltyRuleConfig", RuleType::ScheduleChangeValidation),
    RemoveHoursSchedValidation => ("REMOVE_HOURS_SCHED_VALIDATION", "REMOVE_HOURS_SCHED_VALIDATION", "RemoveHoursPenaltyRuleConfig", RuleType::ScheduleChangeValidation),
    ShiftModificationSchedValidation => ("SHIFT_MODIFICATION_SCHED_VALIDATION", "SHIFT_MODIFICATION_SCHED_VALIDATION", "ShiftModificationPenaltyRuleConfig", RuleType::ScheduleChangeValidation),

    // --- POST_CALC ---
    PremiumHoursRounding => ("PREMIUM_HOURS_ROUNDING", "PREMIUM_HOURS_ROUNDING", "PremiumHoursRoundingRuleConfig", RuleType::PostCalc),

    // --- SHIFT_DIFF_OT ---
    ShiftDifferenceOtSdo => ("SHIFT_DIFF_OT", "SHIFT_DIFFERENCE_OT_SDO", "ShiftDifferenceOTRuleConfig", RuleType::ShiftDiffOt),

    // --- HOURS_DISTRIBUTION ---
    PayPeriodOtHdr => ("PPOT", "PAY_PERIOD_OT_HDR", "PayPeriodOTHrsRuleConfig", RuleType::HoursDistribution),

    // --- SHIFT_DIFFERENTIAL ---
    TimeBetweenShiftsDif => ("TIME_BETWEEN_SHIFTS_DIF", "TIME_BETWEEN_SHIFTS_DIF", "TimeBetweenShiftsRuleConfig", RuleType::ShiftDifferential),
}

impl RuleClass {
    /// Every rule class in a bucket. `getRuleClassesFromType(RuleType)`.
    pub fn of_type(rule_type: RuleType) -> Vec<Self> {
        Self::VALUES
            .iter()
            .copied()
            .filter(|rule_class| rule_class.rule_type() == rule_type)
            .collect()
    }

    /// The rule classes a time clock can evaluate while offline.
    /// `getOfflineTimeRuleClasses()` — a hardcoded pair in Java, not derived
    /// from any property of the entries.
    pub fn offline_time_rule_classes() -> &'static [Self] {
        &[Self::ShortBreakPpr, Self::PromptFixedHrsEarnPpr]
    }

    /// The name of the Java `*RuleImpl` class that implements this rule.
    ///
    /// Reproduces the mangling in `RuleImplFactory.createRuleImpl`: take the
    /// config class name and replace `Config` with `Impl`. Java then
    /// decapitalises the result and looks it up as a Spring bean; Rust
    /// dispatches on the enum, so this exists to locate the algorithm's source
    /// file rather than to resolve anything at runtime.
    pub fn impl_class_name(&self) -> String {
        self.config_class_name().replace("Config", "Impl")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_two_hundred_and_twenty_five_rule_classes_are_present() {
        assert_eq!(RuleClass::VALUES.len(), 225);
    }

    #[test]
    fn every_code_round_trips() {
        for rule_class in RuleClass::VALUES {
            assert_eq!(RuleClass::from_code(rule_class.code()), Some(*rule_class));
        }
    }

    #[test]
    fn every_java_name_round_trips() {
        for rule_class in RuleClass::VALUES {
            assert_eq!(
                RuleClass::from_java_name(rule_class.java_name()),
                Some(*rule_class)
            );
        }
    }

    #[test]
    fn the_codes_are_unique() {
        let mut codes: Vec<_> = RuleClass::VALUES.iter().map(|r| r.code()).collect();
        codes.sort_unstable();
        let count = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), count, "duplicate rule class code");
    }

    #[test]
    fn the_java_names_are_unique() {
        let mut names: Vec<_> = RuleClass::VALUES.iter().map(|r| r.java_name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate rule class name");
    }

    #[test]
    fn an_unknown_code_is_none_rather_than_a_panic() {
        assert_eq!(RuleClass::from_code("NO_SUCH_RULE"), None);
        assert_eq!(RuleClass::from_java_name("NO_SUCH_RULE"), None);
    }

    #[test]
    fn a_code_is_not_generally_the_same_as_its_java_name() {
        // The code is a short database token; the name is the Java constant.
        assert_eq!(RuleClass::MinutePrr.code(), "MIN_PRR");
        assert_eq!(RuleClass::MinutePrr.java_name(), "MINUTE_PRR");
    }

    #[test]
    fn every_rule_type_has_at_least_one_rule_class() {
        for rule_type in RuleType::VALUES {
            assert!(
                !RuleClass::of_type(*rule_type).is_empty(),
                "{rule_type:?} has no rule classes"
            );
        }
    }

    #[test]
    fn the_rule_classes_of_a_type_partition_the_catalogue() {
        let total: usize = RuleType::VALUES
            .iter()
            .map(|rule_type| RuleClass::of_type(*rule_type).len())
            .sum();
        assert_eq!(total, RuleClass::VALUES.len());
    }

    #[test]
    fn punch_rounding_holds_six_rounding_rules() {
        // The punchrounding package has seven `*RuleConfig` classes, but
        // `PunchRoundingRuleConfig` is the abstract base the other six extend
        // and is not a catalogue entry. Wave 1 ports these six.
        let punch_rounding = RuleClass::of_type(RuleType::PunchRounding);
        assert_eq!(punch_rounding.len(), 6);
        assert_eq!(
            punch_rounding,
            vec![
                RuleClass::PropertyDataPrr,
                RuleClass::MinutePrr,
                RuleClass::WorkedHoursPrr,
                RuleClass::InToSchedPrr,
                RuleClass::OutToSchedPrr,
                RuleClass::BackGracePrr,
            ]
        );
    }

    #[test]
    fn the_impl_class_name_follows_javas_config_to_impl_mangling() {
        assert_eq!(
            RuleClass::MinutePrr.config_class_name(),
            "MinuteRoundingRuleConfig"
        );
        assert_eq!(
            RuleClass::MinutePrr.impl_class_name(),
            "MinuteRoundingRuleImpl"
        );
    }

    #[test]
    fn every_config_class_name_ends_in_rule_config() {
        // The mangling in impl_class_name depends on it.
        for rule_class in RuleClass::VALUES {
            assert!(
                rule_class.config_class_name().ends_with("RuleConfig"),
                "{} does not end in RuleConfig",
                rule_class.config_class_name()
            );
        }
    }

    #[test]
    fn the_offline_rules_are_the_hardcoded_pair() {
        assert_eq!(
            RuleClass::offline_time_rule_classes(),
            &[RuleClass::ShortBreakPpr, RuleClass::PromptFixedHrsEarnPpr]
        );
    }
}
