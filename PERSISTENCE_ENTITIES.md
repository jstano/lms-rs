# Persistence Entities/Tables by Crate

Research pass cross-referencing lms-rs Rust crates against JPA/Hibernate `@Entity` classes
in the `taps` Java monorepo, to scope what a future persistence layer needs to load per crate.

**Source layout:** Hibernate `@Entity` classes live across 4 Gradle modules in `taps`:
`planner` (175 entities), `taps` (417, legacy `watson.server.hibernate.entity.*` + `eventlabor`),
`tippool` (153, mostly duplicate/override copies of watson entities), and `framework`
(2 base classes). The newer `rms/timekeeping` and `rms/employeeattendance` modules are
**not JPA** — they're hexagonal/JDBC adapters with table names buried in raw SQL, not annotations.

Almost none of the watson-domain `@Entity` classes carry an explicit `@Table(name=...)` —
Hibernate defaults to the class simple name as the table name. Real table names need
confirming against migration DDL, not just Java source.

---

## workrules

| Rust struct | Java entity | Module |
|---|---|---|
| employee.rs | `Employee` (extends `BasicEmployee`) | taps/planner |
| employee_shift.rs | `EmployeeShift` | taps |
| employee_shift_punch.rs | `EmployeeShiftPunch` | tippool |
| earning_type.rs | `EarningType` | tippool |
| employee_earning.rs | `EmployeeEarning` | planner |
| employee_job_status.rs | `EmployeeJobStatus` | planner |
| rule_set.rs / rule_item.rs | `RuleSet`, `RuleItem` | taps / tippool |
| assignment.rs / assignment_pay_rate.rs | `Assignment`, `AssignmentPayRate` | taps / planner |
| hours_distribution.rs | `HoursDistribution` | planner |
| shift_category.rs | `ShiftCategory` | tippool |
| property.rs | `Property` | taps |
| planned_shift.rs | `PlannedShift` | planner |
| accrual_transaction.rs | not JPA — lives in newer `rms/timekeeping/domain/benefits` JDBC module | — |
| time_card.rs, holiday.rs, calc_data_set_stat.rs, flsa_data.rs, time_clock_result.rs, punch_log.rs | **no `@Entity` match found** — likely computed in-memory or table names buried in the newer adapter modules | — |

## scheduler

Shares the core (`Employee`, `EmployeeShift`, `EmployeeJobStatus`, `HoursDistribution`,
`Property`, `PlannedShift`, `Assignment`) with workrules, plus:

| Rust struct | Java entity | Module |
|---|---|---|
| availability.rs / avail_period.rs | `EmployeeAvailability` | planner |
| day_off_plan.rs / day_off_pattern.rs | `DayOffPlan`, `DayOffPattern` | planner |
| rotation_plan.rs | `RotationPlan` | tippool |
| schedule_group.rs | `ScheduleGroup` | planner |
| employee_regular_period.rs | `EmployeeRegularPeriod` | planner |
| employee_status.rs | `EmployeeStatus`, `EmployeeStatusReason` | planner |
| employee_time_off.rs | `EmployeeTimeOff` | tippool |
| assignment_sort_order.rs | `AssignmentSortOrder` | planner |
| pre_schedule.rs, schedule_calc_data_set.rs | **no match found** — likely staging/derived data | — |

## forecaster

No entity dir — driven by `io::ports` traits in `forecaster/src/io/ports.rs`:

| Port trait | Java entity | Module |
|---|---|---|
| KbiLoaderPort / KbiStat*Port | `KBI`, `KBIConfig` (+`KBIConfigFormula/Percent/Regression/Statistical/TAES`, `KBILaborConfig`, `KBISet`), `KBIStat` | planner/taps |
| FinancialYearPeriodPort | `FNYPeriod` | planner |
| (kbi_stat_data.rs) | `EnvStat`, `Environment`, `RevenueCenter`, `RevenueCenterPeriod(Day)` | planner |
| (loader.rs) | `FlowPlan`, `FlowPatternActual/Period`, `MarketSegment`, `MarketGroup` | tippool/planner |

## planner

| Rust struct | Java entity | Module |
|---|---|---|
| job.rs / job_shift.rs / job_min_max_coverage.rs | `MasterJob`, `JobLaborData`, `AssignmentMinMaxCoverage` | taps/planner |
| work_content.rs | `WorkContent`, `WorkContentDetail` | planner |
| recurring_task_standard.rs | `RecurringTaskStandard` | tippool |
| salaried_standard.rs | `SalariedStandard` | planner |
| spread_standard.rs | `SpreadStandard`, `SpreadStandardRange/Value` | tippool/planner |
| shift_standard.rs | `ShiftRelatedStandard(Value)`, `ShiftRelatedRange` | planner |
| environment.rs | `Environment` | planner |
| planner_settings.rs | `PropertyPlannerSettings`, `AssignmentPlannerSettings` | planner |
| distribution_schedule/pattern/method.rs, business_driver*.rs, meal_break.rs | **no direct match found** — needs a follow-up pass | — |

## timeclock, timeoff, ta

These crates are just stub `lib.rs` files — no domain code yet. Likely Java sources
identified but not yet mapped:

- **timeoff**: `TimeOffRequest`, `TimeOffRequestDistribution`, `TimeOffRequestType`,
  `TimeOffStatusStatDate` (newer `rms/timeoff` module), plus legacy
  `EmployeeTimeOff`/`EmployeeTimeOffType`.
- **timeclock**: `TimeClock`, `TimeClockAssignmentRestriction`,
  `EmployeeTimeClockRestriction`, `TKCode` (planner) + `AssignmentTKCode` (newer non-JPA
  `timekeeping` module).
- **ta**: `rms/employeeattendance/domain/points`, `.../events` — entirely non-JPA; tables
  live in raw SQL inside `PointsRepositoryImpl`/`AttendanceEventRepositoryImpl`.

## Shared core (3+ crates)

`Employee`, `EmployeeShift`, `EmployeeJobStatus`, `Property`, `PlannedShift`,
`HoursDistribution`, `Assignment` — good candidates for one shared persistence module
rather than per-crate duplication.

## Gaps / follow-up needed

1. Almost no watson entities have explicit `@Table(name=...)` — real table names need
   confirming against migration DDL.
2. Several Rust structs (`TimeCard`, `FlsaData`, `Holiday`, `BusinessDriver*`,
   `PreSchedule`, etc.) have no matching `@Entity` — likely computed in-memory, or
   persisted via the newer JDBC/jOOQ adapter modules instead.
3. `AccrualTransaction` and points/attendance data live in the newer hexagonal modules
   (JDBC, not Hibernate) — a follow-up pass through `*RepositoryImpl.java`/`*Sql.java`
   files in `rms/timekeeping` and `rms/employeeattendance` would pin down real table
   names for these gaps.
