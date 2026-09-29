# Persistence Entities/Tables by Crate

Research pass cross-referencing lms-rs Rust crates against JPA/Hibernate `@Entity` classes
in the `taps` Java monorepo, to scope what a future persistence layer needs to load per crate.

**Source layout:** Hibernate `@Entity` classes live across 4 Gradle modules in `taps`:
`planner` (175 entities), `taps` (417, legacy `watson.server.hibernate.entity.*` + `eventlabor`),
`tippool` (153, mostly duplicate/override copies of watson entities), and `framework`
(2 base classes). The newer `rms/timekeeping` and `rms/employeeattendance` modules are
**also Hibernate-backed** — their `*RepositoryImpl` classes use Hibernate's `Criteria` API
against the same legacy `@Entity` classes, just accessed through a repository-pattern
wrapper instead of Spring Data (corrected below — an earlier pass wrongly called these
raw JDBC/jOOQ).

Confirmed via follow-up pass: no `.hbm.xml` files exist, and the underscore naming
strategy is never enabled for this domain, so **the table name is always the literal,
unmodified entity class simple name** (PascalCase) unless an explicit `@Table(name=...)`
overrides it. Known override: `JobLaborData` → table `JobclassLaborData` (see planner
section).

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
| accrual_transaction.rs | `AccrualTransaction` (table `AccrualTransaction`), read via `AccrualTransactionRepositoryImpl` in `rms/timekeeping/adapter_persistence_jpa/benefits` — same table as the legacy entity, not a new one | taps |
| holiday.rs | `Holiday` (table `Holiday`) | taps |
| calc_data_set_stat.rs | `HoursDistributionType` (table `HoursDistributionType`), newer module `rms/timekeeping/domain/hoursdistribution` | taps |
| punch_log.rs | `RawPunchLog` (table `RawPunchLog`) | taps |
| time_card.rs | **confirmed: not persisted** — `TimeCard` is a Java *interface* in `rms/timekeeping/domain/timecard/`; `ScheduleCalcDataSet` is one implementation, assembled at runtime | — |
| flsa_data.rs | **confirmed: computed in-memory** — `watson.server.labor.calcshift.flsacalculations`, no `@Entity` | — |
| time_clock_result.rs | **unconfirmed** — no Java class by this name found anywhere in `taps`; may be a Rust-only aggregate type | — |

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
| pre_schedule.rs | `PreSchedule` (table `PreSchedule`) | taps |
| pre_schedule_jobclass.rs | `PreScheduleJobclass` (table `PreScheduleJobclass`) | taps |
| schedule_calc_data_set.rs | **confirmed: computed in-memory** — `ScheduleCalcDataSet implements TimeCard`, no `@Entity`; built at runtime from underlying entity data | — |

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
| job.rs / job_shift.rs / job_min_max_coverage.rs | `MasterJob` (table `MasterJob`), `JobLaborData` (**table `JobclassLaborData`** — explicit override, does not match class name), `AssignmentMinMaxCoverage` | taps/planner |
| work_content.rs | `WorkContent`, `WorkContentDetail` | planner |
| recurring_task_standard.rs | `RecurringTaskStandard` | tippool |
| salaried_standard.rs | `SalariedStandard` | planner |
| spread_standard.rs | `SpreadStandard`, `SpreadStandardRange/Value` | tippool/planner |
| shift_standard.rs | `ShiftRelatedStandard(Value)`, `ShiftRelatedRange` | planner |
| environment.rs | `Environment` | planner |
| planner_settings.rs | `PropertyPlannerSettings`, `AssignmentPlannerSettings` | planner |
| meal_break.rs / non_meal_break.rs | **confirmed: not separate tables** — plain `Serializable` value classes (`watson.common.labor.breaks`), embedded as columns on `AssignmentPlannerSettings` | — |
| distribution_method.rs | **confirmed: enum**, not a table — `watson.common.enums.DistributionMethod`, used as a column value type on entities like `HoursDistribution`/`TORDistribution` | — |
| distribution_schedule.rs / distribution_pattern.rs, business_driver.rs / business_driver_values.rs | **unresolved — likely reference-implementation-only.** Zero hits in the real `taps` codebase; these names only exist in the bundled `lms-rs/planner/java/` folder, which is a hand-written teaching/reference implementation (`com.stano.planner.engine.*`), not production taps source. If real persistence is needed for these concepts, that mapping doesn't exist in taps yet and needs product input, not more code archaeology. | — |

## timeclock, timeoff, ta

These crates are just stub `lib.rs` files — no domain code yet. Likely Java sources
identified but not yet mapped:

All three read/write the **same underlying legacy tables** already listed under
`workrules`/`scheduler` above — the newer `rms/*` modules are a repository-pattern
wrapper over the existing schema, not new tables:

- **timeoff**: `rms/timeoff/domain/entities.TimeOffRequest` → table **`EmployeeTimeOff`**;
  `TimeOffRequestDistribution` → table **`TORDistribution`**; `TimeOffRequestType` →
  table **`EmployeeTimeOffType`**; `TimeOffStatusStatDate` — plain value object, no
  `@Entity`, not persisted.
- **timeclock**: `TimeClock`, `TimeClockAssignmentRestriction`,
  `EmployeeTimeClockRestriction`, `TKCode` (all in `planner/.../hibernate/entity/`, table
  name = class name) + `AssignmentTKCode` (`rms/timekeeping/domain/assignmenttimekeepingcode`)
  — plain value object, no `@Entity`, not persisted.
- **ta**: `rms.employeeattendance.domain.points.Points` (via `PointsRepositoryImpl`) →
  table **`EmployeePoints`**; `rms.employeeattendance.domain.events.AttendanceEvent` (via
  `AttendanceEventRepositoryImpl`) → table **`EmployeeEvent`**; `EventDocumentTemplate`
  (via `EventDocumentTemplateRepositoryImpl`) → table **`EmployeeEventDocumentTemplate`**.
  All Hibernate-backed, not raw SQL.

## Shared core (3+ crates)

`Employee`, `EmployeeShift`, `EmployeeJobStatus`, `Property`, `PlannedShift`,
`HoursDistribution`, `Assignment` — good candidates for one shared persistence module
rather than per-crate duplication.

## Follow-up pass results

1. **Table naming resolved.** No `.hbm.xml` files exist and the underscore naming
   strategy is never enabled for this domain, so table name = entity class simple name
   verbatim (PascalCase), confirmed against 48 entities with explicit `@Table`. Only
   known exception found: `JobLaborData` → `JobclassLaborData`.
2. **Unmatched structs resolved** (see inline entries above for `holiday.rs`,
   `calc_data_set_stat.rs`, `punch_log.rs`, `pre_schedule.rs`,
   `pre_schedule_jobclass.rs`, `meal_break.rs`, `distribution_method.rs`). Confirmed
   genuinely non-persisted (computed at runtime): `time_card.rs`, `flsa_data.rs`,
   `schedule_calc_data_set.rs`. Still unresolved: `time_clock_result.rs` (no Java
   counterpart found — possibly Rust-only), and `distribution_schedule.rs` /
   `distribution_pattern.rs` / `business_driver*.rs` (exist only in the bundled
   `planner/java/` reference implementation, not in real `taps` source — not a taps
   entity to find, a product-design question to resolve).
3. **Correction:** the newer `rms/timekeeping` and `rms/employeeattendance` modules are
   **not** raw JDBC/jOOQ — their repositories use Hibernate `Criteria` against the same
   legacy `@Entity` classes. `AccrualTransaction`, `EmployeePoints`, `EmployeeEvent`,
   `EmployeeEventDocumentTemplate`, `EmployeeTimeOff`, `TORDistribution`,
   `EmployeeTimeOffType` are all real Hibernate-backed tables, not new schema to design.
