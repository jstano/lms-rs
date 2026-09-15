# `workcontent` Data Model — Complete Design

> Target design for the `planner::workcontent` module, covering full parity with the legacy Java
> engine (`planner/java/engine/...`). This document describes the **finished** data model — what
> the module needs to fully generate `WorkContent`/`LaborData` from KBI-driven and salaried
> standards, and to feed that output downstream into scheduled `PlannedShift`s. Section 5 tracks
> how much of this exists in Rust today; everything else in this document is proposed, not built.

## 1. Overview

`workcontent` turns a snapshot of planning configuration (`PlannerModel`) into staffing
requirements. Two shapes of output exist:

- **`WorkContent`** — discrete blocks of required work on a date/time axis (e.g. "need 2 people,
  6:00–10:00, front desk"), computed from KBI-driven volume forecasts distributed across the day.
- **`LaborData`** — flat daily hour totals, computed for salaried roles that don't need
  time-of-day granularity.

`WorkContent` is not the end of the pipeline — it is later converted into `PlannedShift`s (actual
schedulable shift blocks), and every intermediate calculation can be captured into a
`WorkContentLog` audit tree for debugging/traceability.

### Flow

```
PlannerModel
  └─ for each Job (leaf Assignment in the labor-level tree)
       └─ resolve PlannerSettings (Assignment-level → Property-level → inherited/merged)
            └─ dispatch on StandardType
                 ├─ NONE      → no-op
                 ├─ BASIC     → non-flowed distribution over ShiftRelatedStandard/TaskStandard
                 ├─ ADVANCED  → KBI-driven flow/spread/recurring-task distribution engine
                 └─ SALARIED  → fixed hours-per-week/year calculation
            └─ recurse into child Assignments beneath this Job
       └─ [optional] record every step into WorkContentLog (audit)
       └─ produce WorkContent[] / LaborData[]
            └─ (downstream) PlannedShiftCreator → PlannedShift[] → PlannedShiftWorkContent links
```

### Rust ↔ Java `StandardType` mapping

The Rust `StandardType` enum (`NONE | BASIC | ADVANCED | SALARIED`) and the Java one
(`NONE | KBI_RELATED | SALARIED | SIMPLE_NON_FLOWED`) are not spelled the same but line up
one-to-one, inferred from behavior (`PlannerSettings.non_flowed_distribution_method` already
exists in Rust and only makes sense for the non-flowed path):

| Rust | Java | Generator behavior |
|---|---|---|
| `NONE` | `NONE` | No-op. |
| `BASIC` | `SIMPLE_NON_FLOWED` | Builds `WorkContent` directly from standards using a fixed `NonFlowedDistributionMethod` (`BEGINNING`/`MIDDLE`/`END`/`EVEN`/`VARYING`) — no KBI flow curve needed. |
| `ADVANCED` | `KBI_RELATED` | Full engine: resolves KBI volume per date/environment, looks up the matching standard range/value, distributes it across time periods via a `FlowPlan`/`Distributor` keyed by `DistributionMethod`, applies breaks, collapses into `WorkContent` blocks per `PlanType`. |
| `SALARIED` | `SALARIED` | Already implemented in Rust — sums `hours_per_week`/`hours_per_year`/`vacation_hours_per_year` per `SalaryMode` into `LaborData`. |

---

## 2. Entity Catalog

Conventions used throughout:
- IDs are newtype wrappers via the existing `id_type!` macro (`common/id_type.rs`), uuid-based.
  Java's mixed `int`/`UUID` PKs are intentionally normalized to uuid everywhere.
- Related entities are referenced **by id**, not embedded by value (see §4).
- Dates/times use `joda_rs` (`LocalDate`, `LocalTime`, `LocalDateTime`, `DayOfWeek`) and
  `date_range_rs::DateRange`, matching current usage.
- Java's constant-holder classes/interfaces become Rust enums.

### 2.1 Organizational hierarchy

#### `Property`
*Java: `Property.java`* — root aggregate for a physical location; owns calendars, environments,
units, standard sets, shift categories, revenue centers, KBIs, flow plans, and property-level
settings. **Reconciliation:** supersedes/absorbs Rust's current `Location` (id-only stub) — the
existing `LocationId` becomes `PropertyId`.

| Field | Type | Description |
|---|---|---|
| `id` | `PropertyId` | |
| `name` | `String` | |
| `code` | `String` | |
| `number_rooms` | `u32` | |
| `period_start_date` / `period_end_date` | `LocalDate` | |
| `week_end_day` | `DayOfWeek` | |
| `weeks_out` | `u32` | |
| `planning_period_start_day_of_month` | `u32` | |
| `start_of_day` | `LocalTime` | |
| `period_length` | `PeriodLength` | |
| `default_shift_category_id` | `Option<ShiftCategoryId>` | |
| `forecast_standard_set_id` | `Option<StandardSetId>` | |
| `analysis_standard_set_id` | `Option<StandardSetId>` | |
| `use_actual_flows_for_standard_hours` | `bool` | |

#### `LaborLevel`
*Java: `LaborLevel.java`* — defines one level of the Property's organizational tree (e.g.
Property → Department → Job).

| Field | Type | Description |
|---|---|---|
| `id` | `LaborLevelId` | |
| `property_id` | `PropertyId` | |
| `parent_labor_level_id` | `Option<LaborLevelId>` | self-reference |
| `level` | `u32` | depth/ordinal |
| `is_job` | `bool` | true when assignments at this level are leaf jobs |
| `plural_name` / `singular_name` | `String` | |

#### `Assignment`
*Java: `Assignment.java`* — node in the labor-structure tree. **Reconciliation:** generalizes
Rust's current flat `Job`. `Job` becomes the job-level node of this tree (`labor_level.is_job ==
true`); generation still starts at each `Job` but now recurses into child `Assignment`s beneath
it, matching Java's `generateStandardsForAssignment` recursion. Stored as a flat `Vec<Assignment>`
on `PlannerModel`, traversed via `parent_assignment_id`; a `job_for(assignment_id)` helper walks
up the parent chain to the nearest `is_job` node (mirrors Java's `Assignment.getJob()`).

| Field | Type | Description |
|---|---|---|
| `id` | `AssignmentId` | |
| `property_id` | `PropertyId` | |
| `parent_assignment_id` | `Option<AssignmentId>` | |
| `labor_level_id` | `LaborLevelId` | |
| `name` / `code` | `String` | |

`Job` (existing Rust struct) narrows to: `id: JobId`, `assignment_id: AssignmentId`,
`property_id: PropertyId`. Its current `shifts`/`salaried_standards` fields move to being looked
up from the standards catalog (§2.7) keyed by `assignment_id`, rather than embedded.

### 2.2 Settings & configuration

#### `PlannerSettings`
*Java: `PlannerSettings` interface, implemented by `AssignmentPlannerSettings` /
`PropertyPlannerSettings` / `InheritedPlannerSettings`* — resolved as a single Rust struct
produced by a resolution chain (assignment-level override → property-level default → merged/
inherited), rather than three separate implementer types. **Reconciliation:** matches the
existing Rust `PlannerSettings` struct field-for-field; note the existing `Job::planner_settings()`
getter bug (§5) that must be fixed as part of wiring this resolution chain in.

| Field | Type | Description |
|---|---|---|
| `standard_type` | `StandardType` | |
| `period_length` | `u32` | |
| `min_shift_length` / `max_shift_length` | `f64` | |
| `rounding_threshold_below_one` / `rounding_threshold_above_one` | `f64` | |
| `meal_break` | `Option<MealBreak>` | |
| `non_meal_break` | `Option<NonMealBreak>` | |
| `effective_dates` | `DateRange` | |
| `generate_long_shifts` | `bool` | |
| `limit_shift_to_max_shift` | `bool` | |
| `truncate_max_coverage` | `bool` | |
| `non_flowed_distribution_method` | `NonFlowedDistributionMethod` | |

Resolution sources, each producing the struct above:
- `AssignmentPlannerSettings` — keyed by `(assignment_id, standard_set_id)`, explicit override.
- `PropertyPlannerSettings` — keyed by `(property_id, standard_set_id)`, property-wide default
  (Java falls back `standard_type = NONE` and a synthetic full-year `effective_dates`).
- `InheritedPlannerSettings` — the merged result actually used at generation time.

#### `StandardSet`
*Java: `StandardSet.java`* — named configuration bucket (e.g. "Forecast", "Actual"). Already
exists in Rust as an id-only stub; extend with:

| Field | Type | Description |
|---|---|---|
| `id` | `StandardSetId` | |
| `name` | `String` | |
| `generate_projected_hours` | `bool` | |
| `generate_standard_hours` | `bool` | |
| `kbi_set_id` | `Option<KbiSetId>` | |

#### Supporting enums

| Enum | Java | Variants |
|---|---|---|
| `StandardType` | `StandardType.java` | `None \| Basic \| Advanced \| Salaried` (see mapping table above) |
| `PeriodLength` | `PeriodLength.java` | `Five(5) \| Ten(10) \| Fifteen(15) \| Thirty(30)` minutes; carries `periods_per_hour()`/`periods_per_day()` |
| `GeneratorMode` | `GeneratorMode.java` | `WorkContentOnly \| FullPlanClearScheduled \| FullPlanKeepScheduled \| UpdatePlan \| ReflowPlan \| ReflowPlanNoRecalcWork` |
| `PlannerMode` | `PlannerMode.java` | `Projected \| Standard \| ProjectedForecastOnly \| ReProject` |
| `PropertyDataKey` | `PropertyDataKey.java` | typed property config keys with defaults: `PlannerEngineMaxDurationForDynamicWork(4)`, `OverrideCurrentSystemDate`, `FlowAssumptionThreshold(1)`, `SyncArrivalTime(false)`, `SystemPlannedShiftAudit(false)`, `LegacyFlowPatternRounding(false)`, `EnableDynamicStandardSets(false)` |
| `NonFlowedDistributionMethod` | `NonFlowedDistributionMethod.java` | `Beginning \| Middle \| End \| Even \| Varying` — **already exists in Rust** |

`MealBreak` (`pub break_after: f64, pub break_length: f64`) and `NonMealBreak`
(`pub break_every: f64, pub break_length: f64`) already exist in Rust and need no change.

### 2.3 Calendar & time context

#### `CalendarPlan` / `CalendarPlanDate`
*Java: `CalendarPlan.java`, `CalendarPlanDate.java`* — property-level open/closed calendar.

| `CalendarPlan` field | Type | Description |
|---|---|---|
| `id` | `CalendarPlanId` | |
| `property_id` | `PropertyId` | |
| `name` | `String` | |
| `year` | `u32` | |
| `open_days` | `[bool; 7]` | Sun..Sat open flags |
| `closed_dates` | `Vec<CalendarPlanDateId>` | |

| `CalendarPlanDate` field | Type | Description |
|---|---|---|
| `id` | `CalendarPlanDateId` | |
| `calendar_plan_id` | `CalendarPlanId` | |
| `start_date` / `end_date` | `LocalDate` | |

#### `Season` / `SeasonPeriod`
*Java: `Season.java`, `SeasonPeriod.java`* — named grouping of date ranges, referenced by
`TaskStandard`.

| `Season` field | Type |
|---|---|
| `id` | `SeasonId` |
| `property_id` | `PropertyId` |
| `name` | `String` |
| `periods` | `Vec<SeasonPeriodId>` |

| `SeasonPeriod` field | Type |
|---|---|
| `id` | `SeasonPeriodId` |
| `season_id` | `SeasonId` |
| `start_date` / `end_date` | `LocalDate` |

#### `Environment` / `Environments`
*Java: `Environment.java`, `Environments.java`* — a named scenario/context (day-of-week-based or
"normal") used pervasively to filter per-context values (coverage, spread values, task
frequencies).

| `Environment` field | Type | Description |
|---|---|---|
| `id` | `EnvironmentId` | |
| `property_id` | `PropertyId` | |
| `name` | `String` | |
| `day_of_week` | `Option<DayOfWeek>` | present when `!ignore_dow` |
| `ignore_dow` | `bool` | |
| `system_env` | `bool` | |
| `comment` | `Option<String>` | |

`Environments` is a lookup helper, not a persisted entity:
`day_of_week_environments: Vec<EnvironmentId>`, `normal_environments: Vec<EnvironmentId>`, plus a
derived `by_day_of_week(&self) -> HashMap<DayOfWeek, EnvironmentId>` and
`resolve(&self, date: LocalDate) -> EnvironmentId`.

#### `EnvStat`
*Java: `EnvStat.java`* — links a KBI/date to its forecast vs. actual `Environment`.

| Field | Type |
|---|---|
| `id` | `EnvStatId` |
| `kbi_id` | `KbiId` |
| `date` | `LocalDate` |
| `forecast_environment_id` | `EnvironmentId` |
| `actual_environment_id` | `EnvironmentId` |

### 2.4 KBI / volume drivers

`KBI` (Key Business Indicator) is the abstraction for "the volume that drives labor" — covers,
transactions, occupied rooms, etc. It is referenced pervasively by the standards catalog (§2.7).

#### `KBI`
*Java: `KBI.java` (record)*

| Field | Type |
|---|---|
| `id` | `KbiId` |
| `property_id` | `PropertyId` |
| `name` / `code` | `String` |
| `unit_id` | `UnitId` |

#### `KbiSet`, `KbiConfig`, `KbiConfigEnvFlowPlan`, `KbiLaborConfig`
*Java: `KBISet.java`, `KBIConfig.java`, `KBIConfigEnvFlowPlan.java`, `KBILaborConfig.java`*

| `KbiSet` field | Type |
|---|---|
| `id` | `KbiSetId` |
| `property_id` | `PropertyId` |
| `name` | `String` |

| `KbiConfig` field | Type | Description |
|---|---|---|
| `id` | `KbiConfigId` | |
| `kbi_id` | `KbiId` | |
| `kbi_set_id` | `KbiSetId` | |
| `main_type` | `KbiType` | |
| `is_env` / `is_primary` | `bool` | |
| `notes` | `Option<String>` | |

| `KbiConfigEnvFlowPlan` field | Type |
|---|---|
| `id` | `KbiConfigEnvFlowPlanId` |
| `kbi_config_id` | `KbiConfigId` |
| `environment_id` | `EnvironmentId` |
| `flow_plan_id` | `FlowPlanId` |

| `KbiLaborConfig` field | Type |
|---|---|
| `id` | `KbiLaborConfigId` |
| `kbi_id` | `KbiId` |
| `flow_plan_id` | `FlowPlanId` |
| `actual_flow_plan_id` | `FlowPlanId` |

`KbiType` enum: `Calculated \| Separate \| Input \| PercentOfBase \| Statistical`.

#### `KbiStat`
*Java: `KBIStat.java`* — daily KBI value + forecast-accuracy record.

| Field | Type |
|---|---|
| `id` | `KbiStatId` |
| `kbi_id` | `KbiId` |
| `property_id` | `PropertyId` |
| `date` | `LocalDate` |
| `estimated_value` / `forecast_value` / `adjusted_value` / `actual_value` | `Option<i32>` |
| `forecast_mape_inclusive` / `forecast_mad_inclusive` | `Option<i32>` |
| `adjusted_mape_inclusive` / `adjusted_mad_inclusive` | `Option<i32>` |
| `forecast_mape_exclusive` / `forecast_mad_exclusive` | `Option<i32>` |
| `adjusted_mape_exclusive` / `adjusted_mad_exclusive` | `Option<i32>` |
| `last_accuracy_calculated_at` / `last_forecast_calculated_at` | `Option<LocalDateTime>` |

#### `DynamicSpreadValue`, `TempFlowPattern`
*Java: `DynamicSpreadValue.java`, `TempFlowPattern.java`* — scratch/computed values tied to a
KBI+date, used mid-calculation.

| `DynamicSpreadValue` field | Type |
|---|---|
| `id` | `DynamicSpreadValueId` |
| `kbi_id` | `KbiId` |
| `stat_date` | `LocalDate` |
| `value_type` | `ValueType` (`Forecast \| Actuals`) |
| `period_no` | `u32` |
| `value` | `i32` |

| `TempFlowPattern` field | Type |
|---|---|
| `id` | `TempFlowPatternId` |
| `kbi_id` | `KbiId` |
| `date` | `LocalDate` |
| `flow_pattern_id` | `FlowPatternId` |

#### `Unit`
*Java: `Unit.java`, `Units.java`*

| Field | Type |
|---|---|
| `id` | `UnitId` |
| `property_id` | `PropertyId` |
| `plural_name` / `singular_name` / `symbol` | `String` |

`Units` enum (measurement kind, distinct from the `Unit` entity above): `Hours \| Minutes \|
HoursPerUnit \| MinutesPerUnit \| UnitsPerHour \| UnitsPerMinute \| UnitsPerShift`.

### 2.5 Flow distribution

Models the intraday percentage curve that spreads a daily volume across time periods — the core
mechanism behind the `ADVANCED`/KBI-related generator.

#### `FlowPattern` / `FlowPatternPeriod` / `FlowPatternActual`
*Java: `FlowPattern.java`, `FlowPatternPeriod.java`, `FlowPatternActual.java`*

| `FlowPattern` field | Type |
|---|---|
| `id` | `FlowPatternId` |
| `property_id` | `PropertyId` |
| `name` | `String` |
| `total_percent` | `f64` |
| `total_units` | `u32` |
| `periods` | `Vec<FlowPatternPeriodId>` |

| `FlowPatternPeriod` field | Type |
|---|---|
| `id` | `FlowPatternPeriodId` |
| `flow_pattern_id` | `FlowPatternId` |
| `pattern_value` | `f64` |
| `actual_value` | `Option<i32>` |
| `period_no` | `u32` |

| `FlowPatternActual` field | Type | Description |
|---|---|---|
| `id` | `FlowPatternActualId` | |
| `flow_pattern_id` | `FlowPatternId` | |
| `stat_date` | `LocalDate` | |
| `period_no` | `u32` | |
| `actual_value` | `Option<i32>` | distinct per-date observation, vs. `FlowPatternPeriod`'s single built-in value |

#### `FlowPlan`
*Java: `FlowPlan.java`* — maps each day-of-week to a `FlowPattern` over a date range.

| Field | Type |
|---|---|
| `id` | `FlowPlanId` |
| `property_id` | `PropertyId` |
| `name` / `code` | `String` |
| `start_date` / `end_date` | `LocalDate` |
| `flow_pattern_by_day` | `HashMap<DayOfWeek, FlowPatternId>` (replaces Java's 7 discrete `sunFlowPattern..satFlowPattern` fields) |

### 2.6 Shift definitions

#### `AssignmentShift` / `AssignmentShiftDetail`
*Java: `AssignmentShift.java`, `AssignmentShiftDetail.java`* — **Reconciliation:** supersedes
Rust's current `JobShift`/`JobShiftDefinition`. Fixes two issues noted in §5: definitions are
referenced by id rather than embedding a full parent `JobShift`, and start/end times are typed
`LocalTime` (not `LocalDate`).

| `AssignmentShift` field | Type | Description |
|---|---|---|
| `id` | `AssignmentShiftId` | (replaces `JobShiftId`) |
| `assignment_id` | `AssignmentId` | |
| `standard_set_id` | `StandardSetId` | |
| `shift_name` | `String` | |
| `shift_no` | `u32` | sequence, replaces existing `sequence` field |
| `wage` | `f64` | |
| `details` | `Vec<AssignmentShiftDetailId>` | (replaces embedded `shift_definitions: Vec<JobShiftDefinition>`) |

| `AssignmentShiftDetail` field | Type | Description |
|---|---|---|
| `id` | `AssignmentShiftDetailId` | |
| `assignment_shift_id` | `AssignmentShiftId` | fixes the current embedded-`JobShift` design smell |
| `environment_id` | `EnvironmentId` | |
| `day_of_week` | `DayOfWeek` | |
| `start_time` / `end_time` | `LocalTime` | fixes current `LocalDate` typing bug |
| `hours_before` / `hours_after` | `f64` | |
| `min_number_of_shifts` | `u32` | |
| `shift_grouping_for_same_event` | `u32` | |
| `weighting_factor` | `u32` | |

#### `AssignmentEffectiveDate`
*Java: `AssignmentEffectiveDate.java`* — date range during which an `Assignment`'s `StandardSet`
config applies (drives dynamic/per-date `PlannerSettings` resolution).

| Field | Type |
|---|---|
| `id` | `AssignmentEffectiveDateId` |
| `assignment_id` | `AssignmentId` |
| `standard_set_id` | `StandardSetId` |
| `start_date` / `end_date` | `LocalDate` |

#### `AssignmentMinMaxCoverage`
*Java: `AssignmentMinMaxCoverage.java`* — min/max staffing bounds per environment/period.

| Field | Type |
|---|---|
| `id` | `AssignmentMinMaxCoverageId` |
| `assignment_id` | `AssignmentId` |
| `standard_set_id` | `StandardSetId` |
| `environment_id` | `EnvironmentId` |
| `staff_values` | `Vec<(u32, i32)>` (period → value) |

#### `ShiftCategory`, `ShiftType`, `ShiftSource`, `PlanType`

`ShiftCategory` (Java: `ShiftCategory.java`):

| Field | Type |
|---|---|
| `id` | `ShiftCategoryId` |
| `property_id` | `PropertyId` |
| `name` / `code` | `String` |
| `training_shift_category_id` | `Option<ShiftCategoryId>` (self-ref) |
| `include_in_productivity` | `bool` |
| `is_training_shift` / `is_contract_shift` | `bool` |
| `auto_sync_scheduled_hours` / `auto_calculate_costs` | `bool` |

| Enum | Java | Variants |
|---|---|---|
| `ShiftType` | `ShiftType.java` | `Actual \| Generated \| Schedule` |
| `ShiftSource` | `ShiftSource.java` | `Auto \| Manual \| Rule` — **already exists in Rust** |
| `PlanType` | `PlanType.java` | `Forecast \| Generated \| Original \| Standard` — **already exists in Rust** |

### 2.7 Standards catalog

The four families of "how much work is required" definitions, all hung off `Assignment`, plus the
task-checklist variant. `DistributionMethod` (on `ShiftRelatedStandard`) picks the `Distributor`
used during generation (§3).

#### `ShiftRelatedStandard` / `ShiftRelatedRange` / `ShiftRelatedStandardValue`
*Java: `ShiftRelatedStandard.java`, `ShiftRelatedRange.java`, `ShiftRelatedStandardValue.java`* —
the primary KBI-driven "how much work" standard used by the `ADVANCED` generator.

| `ShiftRelatedStandard` field | Type | Description |
|---|---|---|
| `id` | `ShiftRelatedStandardId` | |
| `assignment_id` | `AssignmentId` | |
| `standard_set_id` | `StandardSetId` | |
| `assignment_shift_id` | `AssignmentShiftId` | |
| `kbi_id` | `KbiId` | |
| `work_type` | `WorkType` | |
| `units` | `Units` | |
| `distribution_method` | `DistributionMethod` | |
| `non_flowed_distribution_method` | `NonFlowedDistributionMethod` | |
| `flow_plan_id` | `Option<FlowPlanId>` | |
| `ignore_retention` | `bool` | |
| `earliest_work_start_time` / `latest_work_end_time` | `Option<LocalTime>` | |
| `name` | `String` | |
| `suppress_value` | `i32` | |
| `ranges` | `Vec<ShiftRelatedRangeId>` | |
| `task_standard_details` | `Vec<TaskStandardDetailId>` | |

| `ShiftRelatedRange` field | Type |
|---|---|
| `id` | `ShiftRelatedRangeId` |
| `shift_related_standard_id` | `ShiftRelatedStandardId` |
| `from_volume` / `to_volume` | `i32` |
| `values` | `Vec<ShiftRelatedStandardValueId>` |

| `ShiftRelatedStandardValue` field | Type |
|---|---|
| `id` | `ShiftRelatedStandardValueId` |
| `shift_related_range_id` | `ShiftRelatedRangeId` |
| `environment_id` | `EnvironmentId` |
| `value` | `f64` |

#### `SpreadStandard` / `SpreadStandardRange` / `SpreadStandardValue` / `DynamicSpreadStandard`
*Java: `SpreadStandard.java`, `SpreadStandardRange.java`, `SpreadStandardValue.java`,
`DynamicSpreadStandard.java`, `FilterableSpreadStandard.java`* — alternate KBI-driven standard
that spreads a value across ranges/environments, either fixed or dynamically computed.

| `SpreadStandard` field | Type |
|---|---|
| `id` | `SpreadStandardId` |
| `assignment_id` | `AssignmentId` |
| `standard_set_id` | `StandardSetId` |
| `assignment_shift_id` | `AssignmentShiftId` |
| `kbi_id` | `KbiId` |
| `dynamic_spread_unit_type` | `Units` |
| `spread_standard_type` | `SpreadStandardType` (`Fixed \| Dynamic`) |
| `ranges` | `Vec<SpreadStandardRangeId>` |

| `SpreadStandardRange` field | Type | Description |
|---|---|---|
| `id` | `SpreadStandardRangeId` | |
| `spread_standard_id` | `SpreadStandardId` | |
| `from_volume` / `to_volume` | `i32` | |
| `fixed_values` | `Vec<SpreadStandardValueId>` | used when `spread_standard_type == Fixed` |
| `dynamic_values` | `Vec<DynamicSpreadStandardId>` | used when `spread_standard_type == Dynamic` |

`SpreadStandardValue` (fixed) and `DynamicSpreadStandard` (computed) both implement the
`FilterableSpreadStandard` behavior — `environment_id() -> EnvironmentId` — so a range can filter
either kind uniformly by environment; modeled in Rust as an enum rather than a Java-style
interface:

```rust
enum SpreadValue {
    Fixed(SpreadStandardValue),       // { id, spread_standard_range_id, environment_id, spread_values: Vec<i32> }
    Dynamic(DynamicSpreadStandard),   // { id, environment_id, spread_standard_range_id, value: f64 }
}
```

#### `RecurringTaskStandard` / `RecurringVariableWork`
*Java: `RecurringTaskStandard.java`, `RecurringVariableWork.java`,
`RecurringTaskStandardCalculationResult.java`* — rule for recurring tasks (daily/weekly/monthly),
with either fixed hours or a KBI-volume-tiered formula.

| `RecurringTaskStandard` field | Type | Description |
|---|---|---|
| `id` | `RecurringTaskStandardId` | |
| `assignment_id` | `AssignmentId` | |
| `standard_set_id` | `StandardSetId` | |
| `kbi_id` | `Option<KbiId>` | |
| `name` | `String` | |
| `initial_date` | `LocalDate` | |
| `frequency_type` | `FrequencyType` | |
| `daily_interval` | `u32` | |
| `weekly_interval` | `u32` | |
| `weekly_days_of_week` | `Vec<DayOfWeek>` | |
| `monthly_interval_type` | `MonthlyIntervalType` | |
| `monthly_day_of_month` | `u32` | |
| `monthly_every_nth_week` | `u32` | |
| `monthly_day_of_week` | `Option<DayOfWeek>` | |
| `monthly_selected_months` | `Vec<Month>` | |
| `occurrence_type` | `OccurrenceType` | |
| `occurs_during_shift_id` | `Option<AssignmentShiftId>` | |
| `occurs_at_time` | `Option<LocalTime>` | |
| `occurs_every_n_hours` | `Option<f64>` | |
| `occurs_starting_at_time` / `occurs_ending_at_time` | `Option<LocalTime>` | |
| `duration_type` | `DurationType` | |
| `fixed_hours` | `f64` | used when `duration_type == Fixed` |
| `variable_works` | `Vec<RecurringVariableWorkId>` | used when `duration_type == Variable` |

| `RecurringVariableWork` field | Type |
|---|---|
| `id` | `RecurringVariableWorkId` |
| `recurring_task_standard_id` | `RecurringTaskStandardId` |
| `from_volume` / `to_volume` | `i32` |
| `base_hours` / `additional_hours` | `f64` |
| `per_number_units` | `u32` |

`RecurringTaskStandardCalculationResult` is a transient (non-persisted) calc-result value:
`total_work_hours: f64, formula_description: String, recurring_variable_work_used:
Option<RecurringVariableWorkId>`.

#### `SalariedStandard` (already exists in Rust)
*Java: `SalariedStandard.java`* — **Reconciliation:** matches the existing Rust struct, with two
changes: reference `AssignmentShiftId` by id (not an embedded `JobShift`), and rename `job_id` →
`assignment_id` to match the generalized tree.

| Field | Type |
|---|---|
| `job_id` → `assignment_id` | `AssignmentId` |
| `standard_set_id` | `StandardSetId` |
| `shift` → `assignment_shift_id` | `AssignmentShiftId` |
| `salary_mode` | `SalaryMode` (`Weekly \| Monthly`) — unchanged |
| `hours_per_week` / `vacation_hours_per_year` / `hours_per_year` | `f64` — unchanged |

#### `TaskStandard` and friends
*Java: `TaskStandard.java`, `TaskStandardStaffingLevels.java`, `TaskStandardDetail.java`,
`TaskStandardRange.java`, `TaskStandardFrequency.java`* — checklist/task-based staffing, distinct
from the KBI-flow standards above.

| `TaskStandard` field | Type |
|---|---|
| `id` | `TaskStandardId` |
| `assignment_id` | `AssignmentId` |
| `standard_set_id` | `StandardSetId` |
| `name` | `String` |
| `task_type` | `TaskType` (`Recurring \| VolumeRelated`) |
| `generates_full_shift` / `rotation_based` / `include_on_checklist` | `bool` |
| `schedule_code` | `Option<String>` |
| `allow_slide_earlier_minutes` / `allow_slide_later_minutes` | `u32` |
| `start_no_earlier_than` / `finish_no_later_than` | `u32` |
| `allow_splits` | `bool` |
| `season_id` | `Option<SeasonId>` |
| `productivity_percentage` | `Option<f64>` |
| `staffing_levels` | `Vec<TaskStandardStaffingLevelsId>` |

| `TaskStandardStaffingLevels` field | Type |
|---|---|
| `id` | `TaskStandardStaffingLevelsId` |
| `task_standard_id` | `TaskStandardId` |
| `cover_count_type` | `CoverCountType` (`Guaranteed \| Set \| Service`) |
| `from_covers` / `to_covers` | `i32` |
| `min_employees` / `max_employees` / `number_employees` / `additional_employees` | `u32` |
| `per_number_guests` | `u32` |
| `allowed_service_overage_percent` | `Option<f64>` |

| `TaskStandardDetail` field | Type |
|---|---|
| `id` | `TaskStandardDetailId` |
| `shift_related_standard_id` | `ShiftRelatedStandardId` |
| `name` / `description` | `String` |
| `number_of_items` | `u32` |
| `reasonable_expectancy` | `f64` |
| `ranges` | `Vec<TaskStandardRangeId>` |

| `TaskStandardRange` field | Type |
|---|---|
| `id` | `TaskStandardRangeId` |
| `task_standard_detail_id` | `TaskStandardDetailId` |
| `from_volume` / `to_volume` | `i32` |
| `frequencies` | `Vec<TaskStandardFrequencyId>` |

| `TaskStandardFrequency` field | Type |
|---|---|
| `id` | `TaskStandardFrequencyId` |
| `task_standard_range_id` | `TaskStandardRangeId` |
| `environment_id` | `EnvironmentId` |
| `frequency` | `u32` |

#### Supporting enums

| Enum | Java | Variants |
|---|---|---|
| `DistributionMethod` | `DistributionMethod.java` | `Flowed \| FillGaps \| NonFlowed \| Opening \| Closing \| ShareWith` |
| `WorkType` | `WorkType.java` | `Daily \| Weekly \| Variable \| Staff \| RecurringTask \| Task \| ShareWith` |
| `FrequencyType` | `FrequencyType.java` | `Daily \| Weekly \| Monthly` |
| `MonthlyIntervalType` | `MonthlyIntervalType.java` | `DayNOfEveryMonth \| NthDayOfEveryNMonths` |
| `OccurrenceType` | `OccurrenceType.java` | `SingleOccurrence \| MultipleOccurrences \| DuringShift` |
| `DurationType` | `DurationType.java` | `Fixed \| Variable` |
| `TaskType` | `TaskType.java` | `Recurring \| VolumeRelated` |
| `CoverCountType` | `CoverCountType.java` | `Guaranteed \| Set \| Service` |
| `SpreadStandardType` | `SpreadStandardType.java` | `Fixed \| Dynamic` |

### 2.8 Revenue/capacity

Volume/capacity source context — where an environment's open hours and capture-ratio settings
come from.

#### `RevenueCenter` / `RevenueCenterPeriod` / `RevenueCenterPeriodStandardSet` / `RevenueCenterPeriodDay`
*Java: `RevenueCenter.java`, `RevenueCenterPeriod.java`, `RevenueCenterPeriodStandardSet.java`,
`RevenueCenterPeriodDay.java`*

| `RevenueCenter` field | Type |
|---|---|
| `id` | `RevenueCenterId` |
| `property_id` | `PropertyId` |
| `name` | `String` |
| `periods` | `Vec<RevenueCenterPeriodId>` |

| `RevenueCenterPeriod` field | Type | Description |
|---|---|---|
| `id` | `RevenueCenterPeriodId` | |
| `revenue_center_id` | `RevenueCenterId` | |
| `kbi_id` | `KbiId` | |
| `unit_id` | `UnitId` | |
| `period_name` | `String` | |
| `period_no` | `u32` | |
| `forecast_accuracy_moderate_threshold` / `forecast_accuracy_critical_threshold` | `Option<i32>` | |
| `standard_sets` | `Vec<RevenueCenterPeriodStandardSetId>` | |

| `RevenueCenterPeriodStandardSet` field | Type |
|---|---|
| `id` | `RevenueCenterPeriodStandardSetId` |
| `standard_set_id` | `StandardSetId` |
| `revenue_center_period_id` | `RevenueCenterPeriodId` |
| `calendar_plan_id` | `Option<CalendarPlanId>` |
| `capture_ratio_kbi_id` | `Option<KbiId>` |
| `days` | `Vec<RevenueCenterPeriodDayId>` |

| `RevenueCenterPeriodDay` field | Type | Description |
|---|---|---|
| `id` | `RevenueCenterPeriodDayId` | |
| `revenue_center_period_standard_set_id` | `RevenueCenterPeriodStandardSetId` | |
| `day_of_week` | `DayOfWeek` | |
| `start_time` / `end_time` | `LocalTime` | |
| `capacity` | `u32` | |
| `retention` | `LocalTime` | |
| `utilization` | `f64` | |
| `is_open` | `bool` | |
| `min_volume` | `u32` | |

### 2.9 Generation outputs

#### `WorkContent` (already exists in Rust)
*Java: `WorkContent.java`* — the culminating output of the `ADVANCED`/`BASIC` generators: a
discrete block of required work. **Reconciliation:** matches the existing Rust struct closely;
add `work_content_type`, `assignment_id` (distinct from `job_id` — Java nulls one or the other
depending on whether the standard was defined at the job level or a child assignment), and links
to the audit trail and detail breakdown.

| Field | Type | Description |
|---|---|---|
| `id` | `WorkContentId` | unchanged |
| `job_id` | `JobId` | unchanged |
| `assignment_id` | `Option<AssignmentId>` | new — set when the originating standard is below job level |
| `property_id` | `PropertyId` | renamed from `location_id`... |
| `work_content_type` | `WorkContentType` (`NonEventRelated`) | new |
| `shift_type` | `PlanType` | unchanged |
| `shift_date` | `LocalDate` | unchanged |
| `earliest_start_date_time` / `preferred_start_date_time` / `latest_end_date_time` | `LocalDateTime` | unchanged |
| `calculated_start_date_time` / `calculated_end_date_time` | `LocalDateTime` | unchanged |
| `calculated_hours` / `adjusted_hours` | `f64` | unchanged |
| `locked` | `bool` | unchanged |
| `description` | `String` | unchanged |
| `min_number_employees` / `min_skill_level` | `u32` | unchanged |
| `distributed_to_date_time` | `LocalDateTime` | unchanged |
| `work_content_log_id` | `Option<WorkContentLogId>` | new — §2.10 |
| `details` | `Vec<WorkContentDetailId>` | new — §below |

#### `WorkContentDetail`
*Java: `WorkContentDetail.java`* — per-KBI breakdown of a `WorkContent`'s hours.

| Field | Type |
|---|---|
| `id` | `WorkContentDetailId` |
| `property_id` | `PropertyId` |
| `work_content_id` | `WorkContentId` |
| `kbi_id` | `KbiId` |
| `total_hours` | `f64` |
| `notes` | `Option<String>` |

#### `LaborData` (already exists in Rust) / `JobLaborData`
*Java: `JobLaborData.java`* — **Reconciliation:** existing Rust `LaborData { job_id, date, hours }`
is the computed value; Java's `JobLaborData` additionally distinguishes forecast vs. standard
hours. Recommend widening the Rust struct rather than adding a parallel type:

| Field | Type | Description |
|---|---|---|
| `job_id` | `JobId` | unchanged |
| `date` | `LocalDate` | unchanged |
| `forecast_hours` | `Option<f64>` | new (was flattened into `hours`) |
| `standard_hours` | `f64` | replaces the current single `hours` field |

### 2.10 Audit trail

Drives directly from `WorkContentLogger.java`'s behavior: one `WorkContentLog` is created per
`(assignment, PlanType, shift_date, assignment_shift)` combination when audit logging is enabled
(`PlannerModel.log_calculations`), and every standard evaluated against that combination appends a
`WorkContentLogDetail`, which in turn can carry one or more period-by-period `TimeValuePair` arrays
(flow percents, minutes, minutes-with-retention) for debugging.

#### `WorkContentLog`
*Java: `WorkContentLog.java`*

| Field | Type |
|---|---|
| `id` | `WorkContentLogId` |
| `job_id` | `JobId` |
| `property_id` | `PropertyId` |
| `assignment_id` | `AssignmentId` |
| `assignment_shift_id` | `AssignmentShiftId` |
| `plan_type` | `PlanType` |
| `shift_date` | `LocalDate` |
| `min_shift` / `max_shift` | `f64` |
| `arrays` | `Vec<WorkContentLogArrayId>` |
| `details` | `Vec<WorkContentLogDetailId>` |

#### `WorkContentLogArray`
*Java: `WorkContentLogArray.java`* — a named time-series array attached directly to the log (e.g.
staff totals before/after breaks).

| Field | Type |
|---|---|
| `id` | `WorkContentLogArrayId` |
| `work_content_log_id` | `WorkContentLogId` |
| `array_type` | `WorkContentLogArrayType` |
| `values` | `Vec<TimeValuePair>` |

`WorkContentLogArrayType` enum: `NsTotalNoBreak \| NsBreak \| NsTotalWithBreak \| NsBodies \|
NsMin \| NsMax \| StaffTotalNoBreak \| NsSTotalNoBreak \| NsSBreak \| NsSTotalWithBreaks \|
BodiesInitial \| BodiesMin \| BodiesMax \| BodiesFinal`.

#### `WorkContentLogDetail`
*Java: `WorkContentLogDetail.java`* — per-standard calculation trace (records which
`ShiftRelatedStandard`/`SpreadStandard`/`RecurringTaskStandard` was evaluated and the inputs used).

| Field | Type | Description |
|---|---|---|
| `id` | `WorkContentLogDetailId` | |
| `work_content_log_id` | `WorkContentLogId` | |
| `kbi_id` | `Option<KbiId>` | |
| `kbi_value` | `Option<i32>` | |
| `suppress_value` | `Option<i32>` | |
| `work_type` | `WorkType` | |
| `units` | `Units` | |
| `from_volume` / `to_volume` | `Option<i32>` | |
| `standard_value` | `Option<f64>` | |
| `work_in_minutes` | `Option<f64>` | |
| `formula` | `Option<String>` | |
| `distribution_method` | `DistributionMethod` | |
| `non_flowed_distribution_method` | `Option<NonFlowedDistributionMethod>` | |
| `flow_pattern_id` | `Option<FlowPatternId>` | |
| `flow_pattern_actual_data_used` | `Option<bool>` | |
| `flow_pattern_id_used_for_actual_data` | `Option<i32>` | |
| `ignore_retention` | `Option<bool>` | |
| `earliest_work_start_time` / `latest_work_end_time` | `Option<LocalTime>` | |
| `arrays` | `Vec<WorkContentLogDetailArrayId>` | |

#### `WorkContentLogDetailArray`
*Java: `WorkContentLogDetailArray.java`*

| Field | Type |
|---|---|
| `id` | `WorkContentLogDetailArrayId` |
| `work_content_log_detail_id` | `WorkContentLogDetailId` |
| `array_type` | `WorkContentLogDetailArrayType` (`FlowPercents \| Minutes \| MinutesWithRetention`) |
| `values` | `Vec<TimeValuePair>` |

`TimeValuePair` (shared value object, both array types): `{ period_no: u32, value: f64 }`.

### 2.11 Downstream consumers (scheduling)

`WorkContent` is not the final artifact — a `PlannedShiftCreator` step turns it into actual
`PlannedShift`s, which is what employees eventually get scheduled against.

#### `PlannedShift` (already exists in Rust, as a stub)
*Java: `PlannedShift.java`* — **Reconciliation:** the existing Rust struct has several drift
issues to fix (see §5): `id` should use the existing-but-unused `PlannedShiftId` (uuid) instead of
`i32`; `assignment_id` should be its own id type, not reuse `JobId`; `property`/`shift_type` should
reference `PropertyId`/keep `PlanType`.

| Field | Type | Description |
|---|---|---|
| `id` | `PlannedShiftId` | fixes current `i32` typing |
| `job_id` | `Option<JobId>` | unchanged |
| `assignment_id` | `Option<AssignmentId>` | new, replaces current `assignment_id: Option<JobId>` |
| `property_id` | `Option<PropertyId>` | replaces `property: Option<Location>` |
| `shift_type` | `Option<PlanType>` | unchanged |
| `shift_category_id` | `Option<ShiftCategoryId>` | new |
| `shift_date` | `Option<LocalDate>` | unchanged |
| `date_shift_generated_from` | `Option<LocalDate>` | unchanged |
| `start_date_time` / `end_date_time` | `Option<LocalDateTime>` | unchanged |
| `duration` | `f64` | unchanged |
| `source` | `Option<ShiftSource>` | unchanged |

`PlannedShiftAudit` (Java: empty marker interface) — modeled as a marker trait/newtype only if a
concrete auditable-change wrapper is needed; otherwise omit, since it carries no data in Java.

#### `PlannedShiftWorkContent`
*Java: `PlannedShiftWorkContent.java`* — join entity linking a generated `PlannedShift` back to
the `WorkContent` it was created to satisfy.

| Field | Type |
|---|---|
| `id` | `PlannedShiftWorkContentId` |
| `planned_shift_id` | `PlannedShiftId` |
| `work_content_id` | `WorkContentId` |
| `work_start` / `work_end` | `LocalDateTime` |
| `work_hours` | `f64` |

#### `PlannedShiftWeightingCalculator`
*Java: `PlannedShiftWeightingCalculator.java`* — property-level override of how heavily each
weighting factor counts when choosing/ranking planned shifts.

| Field | Type |
|---|---|
| `id` | `PlannedShiftWeightingCalculatorId` |
| `property_id` | `PropertyId` |
| `calculator_type` | `PlannedShiftWeightingCalculatorType` |
| `override_master_weighting_factor` | `Option<i32>` |

`PlannedShiftWeightingCalculatorCategory` enum: `WorkContent \| ShiftAllocator`.
`PlannedShiftWeightingCalculatorType` enum (each carries a category + default factor):
`RoomPriority \| ConstrainedWork \| PreferredStart \| ShiftWeighting \| ShiftOverlap \|
TaskPriority \| WorkDuration`.

#### `ShareWithSchedule`
*Java: `ShareWithSchedule.java`* — lets one assignment "share" an employee's schedule slot with
another assignment (used by `DistributionMethod::ShareWith`).

| Field | Type |
|---|---|
| `id` | `ShareWithScheduleId` |
| `employee_id` | `EmployeeId` |
| `day_of_week` | `DayOfWeek` |
| `start_time` / `end_time` | `LocalTime` |
| `job_id` | `JobId` |
| `assignment_id` | `AssignmentId` |

#### `Employee`, `EmployeeShift`, `EmployeeShiftRequest`
*Java: `Employee.java`, `EmployeeShift.java`, `EmployeeShiftRequest.java`* — the actual
scheduling layer that consumes `PlannedShift`s. Included for completeness since work content's
purpose is ultimately to drive these, but out of scope for `workcontent`'s own generation logic.

| `Employee` field | Type |
|---|---|
| `id` | `EmployeeId` |
| `property_id` | `PropertyId` |
| `emp_id` | `String` |
| `name` | `String` |
| `hire_date` | `LocalDate` |
| `seniority_date` | `Option<LocalDate>` |
| `hours_available` | `Option<f64>` |

| `EmployeeShift` field | Type | Description |
|---|---|---|
| `id` | `EmployeeShiftId` | |
| `employee_id` | `EmployeeId` | |
| `assignment_id` | `AssignmentId` | |
| `property_id` | `PropertyId` | |
| `shift_date` | `LocalDate` | |
| `shift_type` | `ShiftType` | |
| `shift_category_id` | `Option<ShiftCategoryId>` | |
| `source` | `ShiftSource` | |
| `created_at` | `LocalDateTime` | |
| `planned_shift_id` | `Option<PlannedShiftId>` | |
| `start_date_time` / `end_date_time` | `Option<LocalDateTime>` | |
| `worked_hours` / `adjusted_hours` / `net_hours` | `f64` | |
| `avg_rate` / `net_dollars` | `f64` | |
| `regular_hours` / `regular_rate` / `regular_dollars` | `f64` | |
| `auto_ot_hours` / `adjusted_ot_hours` / `ot_hours` / `ot_rate` / `ot_dollars` | `f64` | |
| `auto_dt_hours` / `adjusted_dt_hours` / `dt_hours` / `dt_rate` / `dt_dollars` | `f64` | |
| `employee_approved_at` / `manager_approved_at` | `Option<LocalDateTime>` | |
| `notes` | `Option<String>` | |
| `flags` | `u32` | |
| `overlap` | `bool` | |

| `EmployeeShiftRequest` field | Type |
|---|---|
| `id` | `EmployeeShiftRequestId` |
| `requested_by_employee_id` | `EmployeeId` |
| `entered_on` | `LocalDateTime` |
| `give_away_shift_id` | `EmployeeShiftId` |
| `planned_shift_id` | `PlannedShiftId` |
| `employee_comments` | `Option<String>` |
| `manager_comments` | `Option<String>` |
| `status_changed_on` / `acknowledged_on` | `Option<LocalDateTime>` |
| `email_sent` | `bool` |
| `email_sent_on` | `Option<LocalDateTime>` |
| `hours_tardy` | `Option<f64>` |
| `auto_approve` | `bool` |
| `expires_on` | `Option<LocalDateTime>` |

---

## 3. Orchestration / Generation Flow

Maps the Java `PlannerService` → `WorkContentProcessor` pipeline onto Rust's existing
`WorkGenerator` trait and `work_generators::create` factory (`generators/work_generators.rs`),
which stay the right shape for this design — only their implementations need to grow.

1. **Entry** (existing, `main::generate_work_content`) — for each `Job` in `PlannerModel`, resolve
   its `PlannerSettings` via the resolution chain (§2.2, fixing the current `Job::planner_settings()`
   bug), then dispatch on `standard_type` to a `WorkGenerator`.
2. **Recursion** (new) — after handling a `Job`, recurse into its child `Assignment`s
   (`Assignment.parent_assignment_id`), resolving `PlannerSettings` independently per assignment
   (an assignment's settings can differ from its parent job's).
3. **Settings resolution mode** — fixed (`StandardSet` resolved once) vs. dynamic
   (`PropertyDataKey::EnableDynamicStandardSets`), where settings are re-resolved per date, filtered
   by `AssignmentEffectiveDate`.
4. **`BASIC` / non-flowed path** — for each `(date, AssignmentShift)`, evaluate each standard
   attached to the assignment (`ShiftRelatedStandard` with `distribution_method ==
   NonFlowed`/`Opening`/`Closing`, or `RecurringTaskStandard`), and distribute the resulting
   hours across the shift window per `PlannerSettings.non_flowed_distribution_method`
   (`Beginning \| Middle \| End \| Even \| Varying`) directly into `WorkContent` blocks — no KBI
   flow curve needed. This directly replaces the current placeholder `"BASIC:{date}:SHIFT:{id}"`
   string markers in `BasicStandardsProcessorImpl`.
5. **`ADVANCED` / KBI-related path** — for each `(date, AssignmentShift, Environment)`:
   - Resolve the KBI volume for the date (via `KbiStat`/`DynamicSpreadValue`/environment-aware
     `EnvStat`).
   - Look up the matching `ShiftRelatedRange`/`SpreadStandardRange` (by `from_volume..to_volume`)
     and its `ShiftRelatedStandardValue`/`SpreadValue` for the resolved `Environment`.
   - Select a `Distributor` strategy keyed by `DistributionMethod` (flowed via `FlowPlan`/
     `FlowPattern`, fill-gaps, non-flowed, opening, closing, share-with) to spread the value across
     time periods, producing a `Vec<TimeValuePair>`.
   - Apply `MealBreak`/`NonMealBreak` insertion.
   - Scan the resulting per-period body-count array for contiguous non-zero runs; each run becomes
     one `WorkContent` block (one per body/head-count layer), cloned per configured `PlanType`.
   - When `PlannerModel.log_calculations` is set, record every intermediate array/value into the
     current `WorkContentLog`/`WorkContentLogDetail`/`WorkContentLogDetailArray` tree (mirrors
     `WorkContentLogger.java`).
6. **`SALARIED` path** (existing, unchanged) — sum `hours_per_week`/`hours_per_year`/
   `vacation_hours_per_year` per `SalaryMode` into `LaborData`.
7. **`NONE` path** (existing, unchanged) — no-op.
8. **Downstream** (new, outside `workcontent`'s own generation but modeled here for completeness)
   — a `PlannedShiftCreator` step converts each generator's `WorkContent` output into
   `PlannedShift`s (replacing the current placeholder `"PLANNED:{marker}"` string markers in
   `BasicPlannedShiftCreator`), recording `PlannedShiftWorkContent` links back to the originating
   `WorkContent`.

---

## 4. Design Conventions

- **ID strategy** — every new entity gets a newtype id via the existing `id_type!` macro
  (`common/id_type.rs`), uuid v4. Java's mixed `int`/`UUID` primary keys are intentionally
  normalized to uuid everywhere in this design.
- **Reference by id, not by value** — related entities are linked via their id type and looked up
  from the owning collection, never embedded by value. This is a deliberate fix, not a
  continuation: the current `JobShiftDefinition` (embeds a full `JobShift`) and `SalariedStandard`
  (embeds a full `JobShift`) patterns are design smells to correct, not propagate, when this design
  is implemented.
- **Enums replace Java constant-holders/interfaces** — e.g. `PlannerSettings` becomes one resolved
  struct produced by a resolution function, rather than a Java interface with three implementing
  classes; `FilterableSpreadStandard` becomes a Rust enum (`SpreadValue::Fixed`/`Dynamic`) rather
  than a trait object.
- **Derives** — baseline `Debug, Clone, PartialEq` on every new struct; add `Eq, Hash, Copy` where
  all fields allow it (matches current usage, e.g. `LaborData`, id types).
- **No `serde`/`sqlx` yet** — matches the current module, which has zero dependencies on either
  (confirmed via `Cargo.toml` and a full-tree grep). This design stays a pure in-memory domain
  model; DB/JSON boundary mapping is an explicit non-goal here and would live in a separate layer.

---

## 5. Current Implementation Status

| Area | Status | Notes |
|---|---|---|
| `PlannerModel`, `Job`, `JobShift`, `SalariedStandard`, `StandardSet`, `Location`, `BusinessDriver` | Implemented (partial) | Present but narrower than this design; `Location`→`Property`, `Job`/`JobShift` need the `Assignment` tree and id-referencing fixes described in §2.1/§2.6. |
| `WorkContent`, `PlannedShift`, `LaborData` domain structs | Implemented (partial) | Present but missing several fields (§2.9, §2.11) and have type bugs (below). |
| `WorkGenerator` trait, `work_generators::create` factory, `WorkResults` | Implemented | Correct shape for this design; only generator bodies need to grow. |
| `SALARIED` generator | Implemented | Produces real `LaborData`; no changes needed beyond the `LaborData`→`JobLaborData` field widening in §2.9. |
| `NONE` generator | Implemented | Correct no-op behavior, no changes needed. |
| `BASIC` generator | Stubbed | `BasicStandardsProcessorImpl` emits placeholder `"BASIC:{date}:SHIFT:{id}"` strings instead of real `WorkContent`; `BasicPlannedShiftCreator` similarly emits `"PLANNED:{marker}"` placeholders. `basic_work_generator.rs` is an empty, dead file. |
| `ADVANCED` generator | Not started | Currently returns an empty shifts vec; none of §2.3–§2.10 (Environment, KBI, FlowPattern/FlowPlan, ShiftRelatedStandard, SpreadStandard, RecurringTaskStandard, WorkContentLog) exist in Rust yet. |
| `Assignment` hierarchy | Not started | Rust only has flat `Job`; no `Property`, `LaborLevel`, or `Assignment` tree. |
| Audit trail (`WorkContentLog` and children) | Not started | No equivalent to `WorkContentLogger.java` exists. |
| Downstream scheduling (`Employee`, `EmployeeShift`, `ShareWithSchedule`, weighting calculators) | Not started | Included in this design for completeness (§2.11) but out of `workcontent`'s own scope to implement. |

**Known bugs/smells in current code to fix as part of implementing this design:**
- `Job::planner_settings()` getter always returns `PlannerSettings::default()` instead of
  `self.planner_settings` — silently ignores configured settings.
- `JobShiftDefinition.start_time`/`end_time` are typed `LocalDate` (should be `LocalTime`).
- `JobShiftDefinition` embeds a full `JobShift` by value instead of referencing it by id
  (self-referential nesting).
- `PlannedShift.id` is `i32` even though the uuid-based `PlannedShiftId` exists and is imported but
  unused; `PlannedShift.assignment_id` is typed `Option<JobId>` instead of a distinct id.
- `BusinessDriverValues` (`domain/business_driver_values.rs`) is unused/superseded by
  `PlannerModel.business_driver_values: HashMap<BusinessDriverId, u32>`.
- `generators/basic/basic_work_generator.rs` is an empty file (dead code).

---

## 6. Appendix: Java → Rust Entity Mapping

Complete reference for every class under `planner/java/engine/entities/`, `WorkContentLogger`, and
the `WorkContentProcessor` orchestration types, mapped to its proposed Rust home. "§" references
point back into this document; "n/a" marks Java types intentionally not carried forward (transient
DTOs, marker interfaces with no data, or concepts superseded by a Rust equivalent).

| Java class | Rust type | Section |
|---|---|---|
| `Assignment` | `Assignment` | §2.1 |
| `AssignmentEffectiveDate` | `AssignmentEffectiveDate` | §2.6 |
| `AssignmentMinMaxCoverage` | `AssignmentMinMaxCoverage` | §2.6 |
| `AssignmentPlannerSettings` | resolution source for `PlannerSettings` | §2.2 |
| `AssignmentShift` | `AssignmentShift` (supersedes `JobShift`) | §2.6 |
| `AssignmentShiftDetail` | `AssignmentShiftDetail` (supersedes `JobShiftDefinition`) | §2.6 |
| `CalendarPlan` | `CalendarPlan` | §2.3 |
| `CalendarPlanDate` | `CalendarPlanDate` | §2.3 |
| `CoverCountType` | `CoverCountType` enum | §2.7 |
| `DistributionMethod` | `DistributionMethod` enum | §2.7 |
| `DurationType` | `DurationType` enum | §2.7 |
| `DynamicSpreadStandard` | `DynamicSpreadStandard` (variant of `SpreadValue`) | §2.7 |
| `DynamicSpreadValue` | `DynamicSpreadValue` | §2.4 |
| `Employee` | `Employee` | §2.11 |
| `EmployeeShift` | `EmployeeShift` | §2.11 |
| `EmployeeShiftRequest` | `EmployeeShiftRequest` | §2.11 |
| `Environment` | `Environment` | §2.3 |
| `Environments` | `Environments` (lookup helper, not persisted) | §2.3 |
| `EnvStat` | `EnvStat` | §2.3 |
| `FilterableSpreadStandard` | `SpreadValue` enum (replaces interface) | §2.7 |
| `FlowPattern` | `FlowPattern` | §2.5 |
| `FlowPatternActual` | `FlowPatternActual` | §2.5 |
| `FlowPatternPeriod` | `FlowPatternPeriod` | §2.5 |
| `FlowPlan` | `FlowPlan` | §2.5 |
| `FrequencyType` | `FrequencyType` enum | §2.7 |
| `GeneratorMode` | `GeneratorMode` enum | §2.2 |
| `InheritedPlannerSettings` | resolution output of `PlannerSettings` | §2.2 |
| `JobLaborData` | folded into `LaborData` | §2.9 |
| `KBI` | `Kbi` | §2.4 |
| `KbiId` | `KbiId` | §2.4 |
| `KBIConfig` | `KbiConfig` | §2.4 |
| `KBIConfigEnvFlowPlan` | `KbiConfigEnvFlowPlan` | §2.4 |
| `KBILaborConfig` | `KbiLaborConfig` | §2.4 |
| `KBISet` | `KbiSet` | §2.4 |
| `KBIStat` | `KbiStat` | §2.4 |
| `KBIType` | `KbiType` enum | §2.4 |
| `LaborLevel` | `LaborLevel` | §2.1 |
| `MealBreak` | `MealBreak` (unchanged) | §2.2 |
| `MonthlyIntervalType` | `MonthlyIntervalType` enum | §2.7 |
| `NonFlowedDistributionMethod` | `NonFlowedDistributionMethod` (unchanged) | §2.2 |
| `NonMealBreak` | `NonMealBreak` (unchanged) | §2.2 |
| `OccurrenceType` | `OccurrenceType` enum | §2.7 |
| `PeriodLength` | `PeriodLength` enum | §2.2 |
| `PlannedShift` | `PlannedShift` (fixes id/assignment typing) | §2.11 |
| `PlannedShiftAudit` | n/a — empty marker interface, no data | §2.11 |
| `PlannedShiftWeightingCalculator` | `PlannedShiftWeightingCalculator` | §2.11 |
| `PlannedShiftWeightingCalculatorCategory` | enum | §2.11 |
| `PlannedShiftWeightingCalculatorType` | enum | §2.11 |
| `PlannedShiftWorkContent` | `PlannedShiftWorkContent` | §2.11 |
| `PlannerArraySizeConstants` | n/a — Java period-array sizing constants, superseded by `Vec` sizing at runtime | — |
| `PlannerMode` | `PlannerMode` enum | §2.2 |
| `PlannerSettings` | `PlannerSettings` (resolved struct, unchanged shape) | §2.2 |
| `PlanType` | `PlanType` (unchanged) | §2.6 |
| `Property` | `Property` (supersedes `Location`) | §2.1 |
| `PropertyDataKey` | `PropertyDataKey` enum | §2.2 |
| `PropertyPlannerSettings` | resolution source for `PlannerSettings` | §2.2 |
| `RecurringTaskStandard` | `RecurringTaskStandard` | §2.7 |
| `RecurringTaskStandardCalculationResult` | transient `RecurringTaskCalcResult` value | §2.7 |
| `RecurringVariableWork` | `RecurringVariableWork` | §2.7 |
| `RevenueCenter` | `RevenueCenter` | §2.8 |
| `RevenueCenterPeriod` | `RevenueCenterPeriod` | §2.8 |
| `RevenueCenterPeriodDay` | `RevenueCenterPeriodDay` | §2.8 |
| `RevenueCenterPeriodStandardSet` | `RevenueCenterPeriodStandardSet` | §2.8 |
| `SalariedStandard` | `SalariedStandard` (unchanged shape) | §2.7 |
| `SalaryMode` | `SalaryMode` (unchanged) | §2.7 |
| `Season` | `Season` | §2.3 |
| `SeasonPeriod` | `SeasonPeriod` | §2.3 |
| `ShareWithSchedule` | `ShareWithSchedule` | §2.11 |
| `ShiftCategory` | `ShiftCategory` | §2.6 |
| `ShiftRelatedRange` | `ShiftRelatedRange` | §2.7 |
| `ShiftRelatedStandard` | `ShiftRelatedStandard` | §2.7 |
| `ShiftRelatedStandardValue` | `ShiftRelatedStandardValue` | §2.7 |
| `ShiftSource` | `ShiftSource` (unchanged) | §2.6 |
| `ShiftType` | `ShiftType` enum | §2.6 |
| `SpreadStandard` | `SpreadStandard` | §2.7 |
| `SpreadStandardRange` | `SpreadStandardRange` | §2.7 |
| `SpreadStandardType` | `SpreadStandardType` enum | §2.7 |
| `SpreadStandardValue` | `SpreadStandardValue` (variant of `SpreadValue`) | §2.7 |
| `StandardSet` | `StandardSet` (extended) | §2.2 |
| `StandardType` | `StandardType` (unchanged) | §1 |
| `TaskStandard` | `TaskStandard` | §2.7 |
| `TaskStandardDetail` | `TaskStandardDetail` | §2.7 |
| `TaskStandardFrequency` | `TaskStandardFrequency` | §2.7 |
| `TaskStandardRange` | `TaskStandardRange` | §2.7 |
| `TaskStandardStaffingLevels` | `TaskStandardStaffingLevels` | §2.7 |
| `TaskType` | `TaskType` enum | §2.7 |
| `TempFlowPattern` | `TempFlowPattern` | §2.4 |
| `Unit` | `Unit` | §2.4 |
| `Units` | `Units` enum | §2.4 |
| `ValueType` | `ValueType` enum | §2.4 |
| `WorkContent` | `WorkContent` (extended) | §2.9 |
| `WorkContentDetail` | `WorkContentDetail` | §2.9 |
| `WorkContentLog` | `WorkContentLog` | §2.10 |
| `WorkContentLogArray` | `WorkContentLogArray` | §2.10 |
| `WorkContentLogArrayType` | enum | §2.10 |
| `WorkContentLogDetail` | `WorkContentLogDetail` | §2.10 |
| `WorkContentLogDetailArray` | `WorkContentLogDetailArray` | §2.10 |
| `WorkContentLogDetailArrayType` | enum | §2.10 |
| `WorkContentType` | `WorkContentType` enum | §2.9 |
| `WorkType` | `WorkType` enum | §2.7 |
| `WorkContentLogger` (audit/) | folded into the audit-recording step of §3 (orchestration), not a data type | §3 |
