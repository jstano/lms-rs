# Complete the BASIC (SIMPLE_NON_FLOWED) Work-Content Generator — TDD, Full Parity, Functional Core

## Context

The Rust `BASIC` generator (`planner/src/workcontent/generators/basic/`, porting Java's
`SIMPLE_NON_FLOWED` standard type) is currently a stub — it emits placeholder strings instead
of real `WorkContent`/`PlannedShift`. The user wants **full parity**, including the
KBI-driven `totalWorkMinutes` resolution chain, built **test-first** (port the *existing* Java
tests to Rust as failing tests, then implement to green — not invented test cases), and
architected as a **functional core with I/O pushed to the edges**: read → input → calculate →
output → save, where "calculate" is pure — no I/O, and (per explicit direction) no panics
either, using `Result`/`Option` end-to-end instead of mirroring Java's crash-on-misconfiguration
behavior.

The existing tests aren't in this repo — they and the real production source (this repo's
`planner/java/engine/` is a renamed/trimmed reference copy) live in a sibling checkout:

- **Production source root** (`JPR`): `/Users/jstano/workspace/unifocus/taps/src/java/com/unifocus/watson/server/labor/planner/engine/`
- **Spock test root** (`JTR`): `/Users/jstano/workspace/unifocus/taps/src/junit/com/unifocus/watson/server/labor/planner/engine/`

A full read-through confirmed the reference copy in `lms-rs/planner/java/engine/` is
**behaviorally identical** to production for every file on this path (only package names/
Spring annotations/Java-version syntax differ) — so it's safe to keep using as the primary
reading copy, but **test cases and exact external-utility semantics come from `JTR`/`JPR`**,
not guessed.

## Pipeline shape (functional core / imperative shell)

```
read ──▶ input ──▶ calculate ──▶ output ──▶ save
```
- **read**: loading `PlannerModel`/`Job`/etc. from a data source. **Out of scope** — no
  persistence layer exists in this Rust project at all yet (by design, per `DATA_MODEL.md`).
  Today these are constructed in-memory (tests, `main.rs`).
- **input**: the plain data handed to the calculation — `PlannerModel`, `Job`, `JobShift`,
  `PlannerSettings`, a `shift_date` — already-loaded, immutable references. No I/O.
- **calculate**: everything in Phases 1, 3, 6–11 below. **Pure functions**: same inputs always
  produce the same outputs, no I/O, no shared mutable state, no panics — genuine error
  conditions (misconfigured environment, un-loaded KBI, invalid `Units` variant) are `Result::Err`
  values, not process aborts. This is the core the user asked to keep functional.
- **output**: `WorkContent`/`PlannedShift` values returned from `calculate` (Phases 10–13) —
  still pure, just the shape of the answer.
- **save**: persisting `WorkContent`/`PlannedShift` somewhere. **Out of scope**, same reason as
  "read" — but the seam is exact: `WorkResults` (returned from `WorkGenerator::generate_work`)
  is precisely the value a future save step would consume. Nothing in this plan blocks adding
  it later; nothing here does it prematurely.

## Error handling: `Result`/`Option` everywhere, no panics in the calculation core

Java throws for two genuine misconfiguration cases on this path — `PlannerModel.getKbiValue`
(NPE on a wholly-missing KBI) and `Environments.getEnvironmentForDate` (`IllegalStateException`
on an unconfigured day-of-week environment). Per explicit direction, Rust does **not** mirror
these as panics. New shared error type:

```rust
// planner/src/workcontent/generators/error.rs
pub enum GenerationError {
    EnvironmentNotConfigured(DayOfWeek),
    KbiNotLoaded(KbiId),
    InvalidUnits(Units), // Units::UnitsPerPerson has no defined formula
}
```
Functions that can hit these return `Result<T, GenerationError>`; functions with a legitimate
"no match" case that *isn't* an error (e.g. "no `ShiftRelatedStandardValue` for this volume")
return `Result<Option<T>, GenerationError>` — the `Option` is real domain data, the `Result`
is only for the two hard-failure cases above. This ripples through:

- Phase 5 (`Environments::environment_for_date`) → `Result<&Environment, GenerationError>`.
- Phase 6 (`PlannerModel::kbi_value`) → `Result<i32, GenerationError>` (`Err` only when the KBI
  itself was never loaded; `Ok(0)` for a missing date on an already-present KBI, matching
  Java's ternary exactly).
- Phase 8 (`assignment_shift_detail`, `shift_related_standard_value`) →
  `Result<Option<T>, GenerationError>`.
- Phase 9 (`calculate_work_minutes_per_unit`, `total_work_minutes`) → `Result<f64, GenerationError>` /
  `Result<i32, GenerationError>`, propagating via `?`.
- Phase 12 (`basic_standards_processor::process`) → `Result<WorkResults, GenerationError>`,
  propagating any per-date/shift/standard error immediately (a misconfigured environment or an
  un-loaded KBI is a run-wide problem, not something to silently skip past).
- Phase 14: the shared `WorkGenerator` trait's `generate_work` signature changes from
  `-> WorkResults` to `-> Result<WorkResults, GenerationError>` so the error can actually reach
  a caller. This is the one change that touches generators outside BASIC — `NoneWorkGenerator`/
  `SalariedWorkGenerator`/`AdvancedWorkGenerator` are trivially infallible today, so their
  change is mechanical (`WorkResults` → `Ok(WorkResults)`), not a rewrite.

Phases 10–11 (`SimpleNonFlowedCalculator`/`WorkContentCalculator`/`WorkContentCreatorUtility`
ports) have no genuine error case in the Java source (pure arithmetic and data copying) and
stay infallible — no `Result` wrapping where nothing can fail.

## Resolved semantics (confirmed from real source)

- **Rounding** (`com.unifocus.tbx.core.TDouble`, `legacy-tbx-core`):
  - `round(double) -> int`: round-half-away-from-zero (custom manual implementation, not
    `Math.round`).
  - `round(double, n) -> double`: rounds to `n` decimals using an epsilon-nudge-then-snap
    trick (`Math.rint(value*10^n + sign(value)*0.01/10^n)/10^n`, then snaps near-zero results
    to exactly `0.0`).
  - `roundHours(v) = round(v, 2)`; `roundRawHours(v) = round(v, 4)` — the 2-vs-4-decimal
    distinction is load-bearing, preserve exactly.
  - `truncate(double) -> int`: raw `(int)` cast — truncation toward zero.
  - `roundLong(double) -> long`: same half-away-from-zero algorithm, `long` result.
- **`TimeUtil.durationInFractionalHours(start, end)`**: if `start > end` (millis-of-day),
  `24.0 - (start-end)/millisPerHour` (overnight); else `(end-start)/millisPerHour`.
- **`AssignmentShiftDetail.getShiftLength()`**: `start.equals(end) ? 24.0 : durationInFractionalHours(start,end)`.
- **`AssignmentShiftDetail.toDateTimeRange(date)`**: if `end.compareTo(start) <= 0` (covers
  both overnight *and* the equal-times case), end date rolls to `date.plusDays(1)`.
- **`DateUtil.correctEndOfFebruary(month,day,year)`**: only bumps Feb 28→29 in a leap year
  when `day==28` exactly (and demotes 29→28 in a non-leap year); used by `EffectiveDateFilter`
  for the annual month/day-only recurring-window matching (confirmed by
  `EffectiveDateFilterTest`, which is **out of scope** here per the existing Scope section
  below — Rust keeps using `PlannerSettings::dates()`).
- **Missing-KBI lookup / missing day-of-week environment**: both confirmed present
  *identically* in production Java (not artifacts of a stale copy) — see "Error handling"
  above for how Rust represents them instead of crashing.
- **Remainder shift-length in `SimpleNonFlowedCalculator`**: confirmed via
  `SimpleNonFlowedCalculatorTest`'s `"If shift length is less than Min shift..."` case (shift
  length 3.0 < minShift 4 → remainder stays 3.0, not raised to 4) — it reads the **raw
  `AssignmentShiftDetail.getShiftLength()`**, not the max-shift-clamped value.
- **`NonFlowedDistributionMethod`**: confirmed via full read of all 16 core files — never
  referenced anywhere on this path. `DATA_MODEL.md`'s claim that BASIC uses it is inaccurate
  for this code path; not implemented here.
- **Full-shift placement pairing**: `SimpleNonFlowedWorkContentCreatorTest` confirms the
  pairing loop (half the full shifts anchored to shift-start, half to shift-end, odd leftover
  → one more start block) always runs structurally — when `shiftLength <= maxShift` the
  start-anchored and end-anchored placement functions simply both return the *same* unclipped
  window, so every block looks identical (test 1); when `shiftLength > maxShift` they diverge
  and produce a real start/end split (test 2). One algorithm, not a conditional branch.

## Scope (unchanged from prior analysis, still applies)

**In scope**: the live call chain from `SimpleNonFlowedStandardsProcessor` down through
`SimpleNonFlowedCalculator`/`SimpleNonFlowedWorkContentCreator`/`PlannedShiftCreator`,
including KBI value lookup, environment-aware detail/value resolution, and the
`ShiftRelatedStandard`/`Units` minutes-per-unit formula.

**Out of scope** (irrelevant to this call chain, or requires infrastructure nothing in this
project has yet): `KBIConfig`/`KBIStat`/`EnvStat` (DB/config-row shapes — no persistence layer
exists — this is the "read"/"save" stages, deliberately deferred), `LaborStructure`'s
map-of-maps indirection (Rust's `Job` owns its standards directly, like `salaried_standards`
already does), dynamic-standard-set mode, `EffectiveDateFilter`'s annual-recurring-window logic
(existing `PlannerSettings::dates()` stays as the date source), `WorkContentLog` audit trail,
`ShiftCategory` on `PlannedShift`.

---

## Phase 0 — Unblock the build

**Model:** Sonnet 5

`planner/Cargo.toml` depends directly on `joda_rs = "0.2.0"`, while `date_range_rs = "0.1.0"`
is permanently pinned to `joda_rs 0.1.0`, causing a dependency-graph conflict and compile
failure in `planner_settings.rs`. Change `joda_rs = "0.2.0"` → `"0.1.0"` — every symbol this
codebase uses exists identically in 0.1.0. **Verify:** `cargo check -p planner` succeeds.

---

## Phase 1 — Rounding utilities (`TDouble` port) + shared error type

**Model:** Sonnet 5

New files: `planner/src/workcontent/common/rounding.rs`, `planner/src/workcontent/generators/error.rs`.

```rust
pub fn round(value: f64) -> i32          // half-away-from-zero
pub fn round_long(value: f64) -> i64     // half-away-from-zero, i64
pub fn round_to_decimals(value: f64, decimals: u32) -> f64  // epsilon-nudge + snap-to-zero
pub fn round_hours(value: f64) -> f64    // round_to_decimals(value, 2)
pub fn round_raw_hours(value: f64) -> f64 // round_to_decimals(value, 4)
pub fn truncate(value: f64) -> i32       // value as i32 (truncation toward zero)
```
```rust
pub enum GenerationError {
    EnvironmentNotConfigured(DayOfWeek),
    KbiNotLoaded(KbiId),
    InvalidUnits(Units),
}
```
Port each rounding formula exactly per "Resolved semantics" above (do not reach for
`f64::round`, which rounds differently than this manual algorithm).

**Test (TDD, write first):** since no dedicated `TDoubleTest` was found, write direct tests
from the documented algorithm: `round` on `0.5/-0.5/1.4/1.6/-1.6`; `round_to_decimals` on a
value with float noise near a boundary (e.g. `1.005` at 2 decimals) and a near-zero result
that must snap to exactly `0.0`; `truncate` on positive/fractional values. This module's
correctness is additionally cross-checked transitively by every downstream phase's ported
test tables (Phase 3 onward).

---

## Phase 2 — `MealBreak`/`NonMealBreak` no-break semantics

**Model:** Sonnet 5

Files: `planner/src/workcontent/domain/meal_break.rs`, `non_meal_break.rs` (both currently
empty `impl` blocks). Add:
```rust
impl MealBreak {
    pub fn with_no_break() -> Self { Self { break_after: -1.0, break_length: -1.0 } }
    pub fn is_no_break(&self) -> bool { self.break_after < 0.0 || self.break_length < 0.0 }
}
```
(identical shape for `NonMealBreak` with `break_every`/`break_length`). Pure, infallible —
no error case here.

**Test:** `with_no_break().is_no_break() == true`; a normal break `is_no_break() == false`;
single-negative-field cases (`break_after < 0` xor `break_length < 0`) → still `true` (this
"single negative field ⇒ no-break" case is exercised repeatedly by Phase 3's table below, e.g.
`mb(5.0, -0.5)`).

---

## Phase 3 — `BreakLengthCalculator` port

**Model:** Sonnet 5

New file: `planner/src/workcontent/generators/basic/basic_break_calculator.rs`. Source:
`JPR/workcontent/utlities/BreakLengthCalculator.java` (confirmed identical in both copies).
Pure, infallible (no error case in the Java source):

```rust
pub fn break_in_fractional_hours(shift_length: f64, meal_break: Option<&MealBreak>, non_meal_break: Option<&NonMealBreak>) -> f64
pub fn productive_time_from_total_duration(total_duration: f64, meal_break: Option<&MealBreak>, non_meal_break: Option<&NonMealBreak>) -> f64
```
(Rust's `f64` can't be null the way Java's `Duration`/break params can, so the "null duration"
edge cases in the Java table don't apply — only the null-vs-present *break* params matter,
modeled as `Option`.)

- `break_in_fractional_hours`: if either break `None`/`is_no_break()` on both → `0.0`; if only
  meal missing → `calculate_break_given_only_non_meal_break`; if only non-meal missing →
  `calculate_break_given_only_meal_break`; else `calculate_break_given_both_break_types`.
- `productive_time_from_total_duration`: mirror structure, returning `total_duration`
  unchanged when both breaks are absent/no-break.
- Private helpers: `calculate_break_given_only_meal_break`, `calculate_break_given_only_non_meal_break`
  (uses `truncate`), `calculate_break_given_both_break_types` (uses `first_multiple_of_non_meal_break_after_meal_break`),
  `calculate_productive_time_given_meal_break_only`, `calculate_productive_time_given_non_meal_break_only`,
  `calculate_productive_time_given_both_break_types`, `first_multiple_of_non_meal_break_after_meal_break`
  (loop: `multiple = 0; while multiple <= meal_break.break_after { multiple += non_meal_break.break_every }`).

**Test (TDD, write first — port verbatim):** the full data table from
`JTR/workcontent/eventrelated/plannedshifts/BreakLengthCalculatorTest.groovy` (confirms it
tests `workcontent.utlities.BreakLengthCalculator`, the same class, despite living in a
different test package) — both `"...calculateBreakInFractionalHours"` (23 rows) and
`"...calculateProductiveTimeFromTotalDuration"` (27 rows) data tables, meal-only/non-meal-only/
both/negative-field-edge-case scenarios. Write these as one `#[rstest]` table (or two) per
method before writing any implementation; confirm they fail (no implementation yet), then
implement until green.

---

## Phase 4 — `Job`/`JobShiftDefinition` domain fixes

**Model:** Sonnet 5

Files: `planner/src/workcontent/domain/job.rs`, `job_shift.rs`. Pure, infallible.

- `Job::planner_settings()` (job.rs:37-39) currently always returns `PlannerSettings::default()`
  — fix to return `&self.planner_settings`. Add `Job::property_id() -> LocationId` accessor.
- `JobShiftDefinition.start_time`/`end_time`: `LocalDate` → `LocalTime`. Empty `impl` block →
  add real accessors plus:
  - `shift_length_hours(&self) -> f64`: `if start==end { 24.0 } else { duration_in_fractional_hours(start,end) }`
    (port the `TimeUtil.durationInFractionalHours` overnight-wrap formula from "Resolved
    semantics" as a local helper).
  - `to_date_time_range(&self, date: LocalDate) -> (LocalDateTime, LocalDateTime)`: end date
    rolls to `date.plus_days(1)` when `end <= start` (covers overnight *and* equal-times).
  - `has_times()`, `minutes_before()`/`minutes_after()` (via Phase 1's `round`).
- Additive (don't touch existing `day_of_week`-based lookup used by SALARIED etc.): add
  `id: JobShiftDefinitionId` and `environment_id: Option<EnvironmentId>` fields, plus
  `JobShift::shift_detail_for_environment(environment_id) -> Option<&JobShiftDefinition>`.

**Test:** `shift_length_hours`/`to_date_time_range` for same-day (e.g. 7:00–15:00 → 8.0h),
overnight (20:00–2:00 → 8.0h, end date +1), and the equal-times 24h case — these three cases
are drawn directly from `WorkContentCalculatorTest`'s own fixture shift times, so they double
as regression coverage once Phase 11 is reached.

---

## Phase 5 — Environment family (new)

**Model:** Sonnet 5

New file: `planner/src/workcontent/domain/environment.rs`. Uuid ids via the existing
`id_type!` macro (project convention; Java's int ids aren't replicated).

```rust
id_type!(EnvironmentId, uuid_v4);
pub struct Environment { id: EnvironmentId, location_id: LocationId, name: String, day_of_week: DayOfWeek, ignore_dow: bool, system_env: bool, comment: String }
pub struct Environments { day_of_week_environments: Vec<Environment>, normal_environments: Vec<Environment> }
impl Environments {
    pub fn environment_for_date(&self, date: LocalDate) -> Result<&Environment, GenerationError>
    // Err(GenerationError::EnvironmentNotConfigured(day_of_week)) instead of Java's panic
}
pub struct EnvironmentData { entries: HashMap<KbiId, HashMap<LocalDate, EnvironmentId>> }
impl EnvironmentData {
    pub fn environment_for_kbi_and_date(&self, kbi_id: KbiId, date: LocalDate) -> Option<EnvironmentId>
    pub fn add(&mut self, kbi_id: KbiId, date: LocalDate, environment_id: EnvironmentId)
}
```
Extend `PlannerModel` with `environments: Environments, environment_data: EnvironmentData`.

**Test (TDD):** port `AssignmentShiftDetailServiceTest`'s 3 cases conceptually at this layer
first (day-of-week resolution; unconfigured → `Err`, not panic) — full end-to-end resolution
(including the shift-detail matching) is Phase 8's job; here just test
`Environments`/`EnvironmentData` in isolation (hit/miss/`Err`).

---

## Phase 6 — KBI value lookup (new)

**Model:** Sonnet 5

New file: `planner/src/workcontent/domain/kbi.rs`. Scoped to identity + resolved values only
(config/DB-row entities out of scope, per the Scope section).

```rust
id_type!(KbiId, uuid_v4);
pub struct Kbi { id: KbiId, location_id: LocationId, name: String, code: String }
```
Extend `PlannerModel` with `kbi_values: HashMap<KbiId, HashMap<LocalDate, i32>>` (consider
consolidating with the existing unused `BusinessDriver`/`BusinessDriverId`/orphaned
`BusinessDriverValues` stub — they look like an earlier, abandoned start at the same concept).
Add `PlannerModel::kbi_value(&self, kbi_id: KbiId, date: LocalDate) -> Result<i32, GenerationError>`:
`Err(GenerationError::KbiNotLoaded(kbi_id))` if `kbi_id` has no entry at all (mirrors Java's
NPE case, without panicking); `Ok(0)` for a missing date on an already-present KBI (matches
Java's ternary).

**Test:** hit → `Ok(value)`; date-miss → `Ok(0)`; kbi-entirely-missing → `Err(KbiNotLoaded(_))`.

---

## Phase 7 — `ShiftRelatedStandard` family (new)

**Model:** Sonnet 5

New file: `planner/src/workcontent/domain/shift_related_standard.rs`. Pure data + a pure
predicate — no error case here.

```rust
pub enum Units { HoursPerUnit, MinutesPerUnit, UnitsPerHour, UnitsPerMinute, Hours, Minutes, UnitsPerShift, UnitsPerPerson }
pub enum WorkType { Daily, Weekly, Variable, Staff, RecurringTask, Task, ShareWith }
pub struct ShiftRelatedStandardValue { id: ..., environment_id: EnvironmentId, value: f64 }
pub struct ShiftRelatedRange { id: ..., from_volume: i32, to_volume: i32, values: Vec<ShiftRelatedStandardValue> }
impl ShiftRelatedRange { pub fn contains_value(&self, value: i32) -> bool } // inclusive both ends
pub struct ShiftRelatedStandard { id: ..., job_id: JobId, standard_set_id: StandardSetId, job_shift_id: JobShiftId, kbi_id: Option<KbiId>, work_type: WorkType, units: Units, suppress_value: i32, ranges: Vec<ShiftRelatedRange>, name: String }
```
(Drops `distributionMethod`/`nonFlowedDistributionMethod`/`flowPlan`/`ignoreRetention` — unused
by this chain.) Extend `Job` with `shift_related_standards: Vec<ShiftRelatedStandard>` + a
lookup filtered by standard-set + shift + non-`Staff` `WorkType`, mirroring the existing
`salaried_standard_for_standard_set_and_shift` pattern (this replaces the need to port
`LaborStructure`, since Rust's `Job` owns its standards directly).

**Test (TDD):** port `ShiftRelatedStandardsFilterTest`'s `"filter"()` case exactly (3 standards:
matching standard-set+shift+non-staff passes; wrong-shift excluded; `Staff` work-type excluded
even on the matching shift → exactly 1 result) and `contains_value`'s inclusive-bounds cases.

---

## Phase 8 — Environment-aware resolution (`AssignmentShiftDetailService` + `ShiftRelatedStandardValueFilter`)

**Model:** Sonnet 5

New file: `planner/src/workcontent/generators/basic/basic_environment_resolution.rs`. Both
Java classes share one 2-tier pattern (per-KBI/date "operational" environment, falling back to
day-of-week) — extract it once. Both functions are `Result<Option<T>, GenerationError>`: the
`Option` is a legitimate "no match" domain outcome, the `Result`'s `Err` only fires when the
day-of-week fallback itself can't resolve (Phase 5's `environment_for_date`).

```rust
fn assignment_shift_detail(planner_model: &PlannerModel, shift: &JobShift, kbi_id: Option<KbiId>, date: LocalDate) -> Result<Option<&JobShiftDefinition>, GenerationError>
fn shift_related_standard_value(planner_model: &PlannerModel, standard: &ShiftRelatedStandard, kbi_value: i32, date: LocalDate) -> Result<Option<&ShiftRelatedStandardValue>, GenerationError>
```
`assignment_shift_detail`: if `kbi_id` present, try `environment_data.environment_for_kbi_and_date`
then `shift.shift_detail_for_environment(...)`; if that yields nothing (including when
`kbi_id` was `None`), fall back to `environments.environment_for_date(date)?` (propagates
`Err` here) then `shift_detail_for_environment(...)` again (Java resolves the *fallback* by
environment id too, not by day-of-week directly on the detail — the day-of-week only selects
which `Environment` applies).

`shift_related_standard_value`: find the `ShiftRelatedRange` containing `kbi_value`
(`contains_value`), then within it, try matching the operational-environment id, and **retry**
with the day-of-week environment id (`environment_for_date(date)?`) if the first attempt found
nothing (two independent attempts, not a single shared `resolve_environment` call — confirmed
by `ShiftRelatedStandardValueFilterTest`'s pattern).

**Test (TDD, port verbatim):**
- `AssignmentShiftDetailServiceTest`'s 3 cases exactly: no-KBI-override → day-of-week detail
  (id 500) wins; KBI-override environment resolved but no matching detail → falls back to
  day-of-week detail (id 500); KBI-override resolved *and* a matching detail exists → override
  detail (id 501) wins over the day-of-week default. Plus a new case (not in Java, required by
  the Result-based redesign): day-of-week environment itself unconfigured → `Err(EnvironmentNotConfigured)`.
- `ShiftRelatedStandardValueFilterTest`'s `testFilter()` 3-row table: `kbiValue` outside
  `[fromVolume,toVolume]` → `Ok(None)` regardless of environment; in-range + environment match
  → `Ok(Some(...))`.

---

## Phase 9 — `CalculateWorkMinutesPerUnit` + `TotalWorkMinutesForStandardsCalculator`

**Model:** Sonnet 5

New files: `planner/src/workcontent/generators/basic/basic_work_minutes_per_unit.rs`,
`basic_total_work_minutes.rs`.

```rust
fn calculate_work_minutes_per_unit(units: Units, standard_value: f64, kbi_value: i32, shift_length: f64) -> Result<f64, GenerationError>
fn total_work_minutes(planner_model: &PlannerModel, job: &Job, shift: &JobShift, shift_date: LocalDate) -> Result<i32, GenerationError>
```
`calculate_work_minutes_per_unit`: `standard_value == 0.0` → `Ok(0.0)`; else branch per `Units`
(`HoursPerUnit: standard_value*kbi_value*60.0`; `MinutesPerUnit: standard_value*kbi_value`;
`UnitsPerHour: kbi_value/standard_value*60.0`; `UnitsPerMinute: kbi_value/standard_value`;
`Hours: standard_value*60.0`; `Minutes: standard_value`; `UnitsPerShift: kbi_value/standard_value*shift_length*60.0`;
`UnitsPerPerson` → `Err(GenerationError::InvalidUnits(Units::UnitsPerPerson))` — no case in
Java's switch either, throws `IllegalArgumentException`), result passed through
`round_raw_hours` (Phase 1) per Java's `roundMinutes` wrapper.

`total_work_minutes`: for each non-staff `ShiftRelatedStandard` on `job`+`shift` (Phase 7),
resolve detail via Phase 8 (`?` to propagate a hard error; `continue` the loop on `Ok(None)` —
skip this standard, contributes 0, matching Java), `kbi_value = planner_model.kbi_value(...)?`
(0 if `kbi_id` is `None`, no lookup needed), resolve `ShiftRelatedStandardValue` via Phase 8
(same `?`/`continue` pattern), `suppressed_kbi_value = max(kbi_value - standard.suppress_value, 0)`,
`shift_length = min(planner_settings.max_shift_length, shift_definition.shift_length_hours())`,
call `calculate_work_minutes_per_unit(...)?`, truncate-accumulate into `i32` (matches Java's
implicit `double`→`int` narrowing on `+=`).

**Test (TDD, port verbatim):**
- `CalculateWorkMinutesPerUnitTest`'s 21-row table for `calculate_work_minutes_per_unit`
  (every `Units` variant, including the zero-standard-value early-out for all 8, and
  shift-length-sensitivity spot checks) plus an explicit `UnitsPerPerson` → `Err` case (not
  exercised with a non-zero value in the Java table — new coverage added here).
- `TotalWorkMinutesForStandardsCalculatorTest`'s `"test totalWorkMinutes"` 3-row table
  (standard-with-no-detail excluded from sum; multi-standard summation; null standard-value →
  contributes 0) and `"calculation uses suppress value"` 4-row table
  (`suppressedValue = max(kbiValue - suppressValue, 0)`, asserted via the 3rd argument passed
  into the per-unit calculation).

---

## Phase 10 — `SimpleNonFlowedCalculator` port

**Model:** Opus (trickiest numerical logic in this plan — the raw-vs-clamped shift-length
subtlety and threshold rounding are easy to get almost-right; worth the extra reasoning margin)

New file: `planner/src/workcontent/generators/basic/basic_calculator.rs`. Pure arithmetic on
already-known inputs — infallible, no `Result` (matches Java, which has no error path here).

```rust
pub struct BasicCalculationResult { job_id: JobId, job_shift_id: JobShiftId, shift_date: LocalDate, number_of_full_time_shifts: i32, remaining_work_hours: f64, work_hours_to_cover_breaks: f64 }
pub fn calculate(planner_settings: &PlannerSettings, shift_definition: &JobShiftDefinition, shift_date: LocalDate, total_work_minutes: i32) -> BasicCalculationResult
```
Port line-for-line: `shift_length = min(max_shift_length, template_length)` only if
`max_shift_length > 0 && max_shift_length < template_length`, else template length;
`productive_full_shift_hours` via Phase 3; `total_work_hours = round_hours(total_work_minutes/60.0)`;
`full_time_shifts = truncate(total_work_hours / productive_full_shift_hours)`;
`paid_breaks_for_full = full_time_shifts as f64 * (shift_length - productive_full_shift_hours)`;
remainder: `raw = round_hours(total_work_hours % productive_full_shift_hours)`; threshold-minutes
check against `truncate(period_length * (below_one_threshold if full_time_shifts<1 else above_one_threshold))`
→ discard to 0 if under threshold; else if `< min_shift_length`, raise to
**`min(shift_definition.shift_length_hours() /* raw, un-clamped */, min_shift_length)`**
(confirmed by Resolved-semantics above); round to period granularity (round up to one period
if `0 < minutes < period_length`, else round to nearest whole number of periods); add breaks
for remainder via Phase 3 if non-zero; final `remaining_work_hours` includes its own breaks.

**Test (TDD, port verbatim):** the full 17-row `testCalculations()` table from
`SimpleNonFlowedCalculatorTest.groovy` (covers no-break/meal-only/non-meal-only/both,
below/above-threshold discard, min-shift raise, period-length rounding at 15 vs 30) **plus**
the standalone `"If shift length is less than Min shift then shift length takes precedence"`
case (shiftLength 3.0 < minShift 4 → remainder stays 3.0) — this second case is the
confirmation test for the raw-vs-clamped ambiguity, make sure it's included and passing.

---

## Phase 11 — `WorkContentCalculator` + `WorkContentCreatorUtility` + `SimpleNonFlowedWorkContentCreator`

**Model:** Opus (the shared-start-time behavior between a start-anchored full-shift block and
a remainder block reads like a bug and isn't — a model eager to "clean up" placement logic
could silently break this)

New file: `planner/src/workcontent/generators/basic/basic_work_content_creator.rs`. Pure data
transformation — infallible.

Small local range type (no need for the full Java `DateTimeRange`/`Duration` machinery — out
of scope, unused elsewhere on this path): `struct WorkBlock { start: LocalDateTime, end: LocalDateTime, hours: f64 }`.

```rust
pub struct WorkContentCalculator<'a> { result: &'a BasicCalculationResult, shift_definition: &'a JobShiftDefinition, planner_settings: &'a PlannerSettings }
impl<'a> WorkContentCalculator<'a> {
    pub fn full_shift_block_for_start_of_shift(&self) -> WorkBlock
    pub fn full_shift_block_for_end_of_shift(&self) -> WorkBlock
    pub fn remaining_work_block(&self) -> WorkBlock
    pub fn number_of_full_shifts(&self) -> i32   // pass-through to result
    pub fn remaining_work_hours(&self) -> f64    // pass-through to result
}
```
- `full_shift_block_for_start_of_shift`: if `shift_definition.shift_length_hours() <= planner_settings.max_shift_length`,
  return the full `to_date_time_range(shift_date)` window unclipped; else clip to
  `max_shift_length` hours measured **from the window start**.
- `full_shift_block_for_end_of_shift`: same condition; else clip to `max_shift_length` hours
  measured **backward from the window end**.
- `remaining_work_block`: always anchored at the window **start**, spanning `remaining_work_hours`.

`create_work_content_record(plan_types: &[PlanType], job: &Job, shift_date, earliest_start, latest_start, latest_end, calculated_start, calculated_end, calculated_hours) -> Vec<WorkContent>`
— port of `WorkContentCreatorUtility.createWorkContentRecord`: one `WorkContent` per
`PlanType`, with `preferred_start_date_time = latest_start` (Java's parameter renaming,
confirmed by `WorkContentCreatorUtilityTest`), `calculated_hours = adjusted_hours = calculated_hours`.

`create_work_content(result, shift_definition, planner_settings, job, plan_types) -> Vec<WorkContent>`
— port of `SimpleNonFlowedWorkContentCreator.generateWorkContentRecords`: `half = full_shifts/2`;
for each pair push one start-anchored + one end-anchored block set; if `full_shifts` odd, one
more start-anchored set; if `remaining_work_hours != 0.0`, one remainder set. Each "set" calls
`create_work_content_record` with `earliest_start = preferred(latest_start) = calculated_start = block.start`,
`latest_end = calculated_end = block.end`, `calculated_hours = block.hours` (all four
timestamp params identical per block, confirmed by `validateCommonConditions` in
`SimpleNonFlowedWorkContentCreatorTest`).

**Test (TDD, port verbatim):**
- `WorkContentCalculatorTest`'s two 3-row tables (`calculateFullShiftWorkDateTimeRangeForStartOfShift`/
  `...ForEndOfShift`) — same-length-as-max (unclipped both ways), overnight (unclipped both
  ways, end date +1), longer-than-max (clipped from start vs. clipped from end, giving
  *different* results — 6:00-14:00 vs 8:00-16:00 for the same 6:00-16:00/8h-max fixture); the
  `calculateRemainingWorkDateTimeRange` case (7:00 + 1.25h → 8:15); `getNumberOfFullShifts`/
  `getRemainingWorkHours` pass-through checks.
- `WorkContentCreatorUtilityTest`'s single case: field-by-field mapping including the
  `latestStartDateTime → preferredStartDateTime` rename, `calculatedHours → adjustedHours` copy,
  one `WorkContent` per `PlanType` (2 in, 2 out).
- `SimpleNonFlowedWorkContentCreatorTest`'s 3 tests: shift-length ≤ max (all blocks identical,
  `fullShifts × numPlanTypes` records); shift-length > max (half start-anchored / half
  end-anchored, still all correct per-plan-type counts); remainder-only (2 records, anchored
  at shift start, ending at start+1.25h). Include the shared `validateCommonConditions` checks
  (job/property ids, `NON_EVENT_RELATED` type, `earliest==preferred==calculated` start,
  `calculated_hours == adjusted_hours == (end-start) fractional hours`, `!locked`).

---

## Phase 12 — Rewrite `basic_standards_processor.rs`

**Model:** Sonnet 5

File: `planner/src/workcontent/generators/basic/basic_standards_processor.rs`. Replace the
placeholder `process()` with the real orchestration from `SimpleNonFlowedStandardsProcessor`,
now `Result`-returning to propagate Phase 8/9's hard errors:

```
fn process(planner_model: &PlannerModel, job: &Job) -> Result<WorkResults, GenerationError> {
    let mut work_contents = Vec::new();
    for date in job.planner_settings().dates(planner_model) {
        for shift in job.shifts_for_standard_set(planner_model.standard_set_id()) {
            let Some(detail) = basic_environment_resolution::assignment_shift_detail(planner_model, shift, None, date)? else { continue };
            let total = basic_total_work_minutes::total_work_minutes(planner_model, job, shift, date)?;
            if total > 0 {
                let result = basic_calculator::calculate(job.planner_settings(), detail, date, total);
                work_contents.extend(basic_work_content_creator::create_work_content(&result, detail, job.planner_settings(), job, planner_model.plan_types()));
            }
        }
    }
    Ok(WorkResults::with_work_content(job.id(), work_contents, Vec::new() /* planned shifts wired in Phase 13/14 */))
}
```
(Initial per-date/shift detail resolution always passes `kbi_id = None`, matching Java — only
the *per-standard* resolution inside `total_work_minutes` is KBI-aware. A hard error from
either resolution call aborts the whole `process()` call via `?` — a misconfigured environment
or an un-loaded KBI is a run-wide problem, not something to silently skip past.)

Extend `PlannerModel` with `plan_types: Vec<PlanType>` (+ constructor param + getter, 2
call sites in `main.rs`). Extend `WorkResults` (`generators/work_generators.rs`) with
`work_content`/`planned_shifts` optional fields + `with_work_content(...)` constructor +
getters, additive alongside the existing `shifts`/`labor_data` variants.

**Test (TDD, port verbatim):**
- `SimpleNonFlowedStandardsProcessorTest`'s empty-effective-dates case (no dates → no results)
  adapted to `PlannerSettings::dates()`.
- `testProcessStandards()`'s shape: N dates × M shifts, each producing results → total =
  N×M×(records per call) — adapt the exact multiplication check to the real (non-mocked)
  Phases 3–11 pipeline via one concrete fixture reproducing `SimpleNonFlowedCalculatorTest`'s
  17.0h/8.0h/0.5h-break acceptance scenario end-to-end (2 full shifts + 1 remainder ×
  `plan_types.len()` records).
- `total <= 0` → no blocks; no matching standard/detail → no blocks (`Ok`, not `Err`).
- New: an unconfigured day-of-week environment anywhere in the date/shift sweep → `process()`
  returns `Err(EnvironmentNotConfigured(_))`.

---

## Phase 13 — Rewrite `basic_planned_shift_creator.rs`

**Model:** Sonnet 5

File: `planner/src/workcontent/generators/basic/basic_planned_shift_creator.rs`. Pure mapping
— infallible. Port `PlannedShiftCreator.createPlannedShiftsFromWorkContents`: one
`PlannedShift` per `WorkContent`, copying `job_id`, `shift_type`, `shift_date`,
`date_shift_generated_from = shift_date`, `start_date_time`/`end_date_time`/`duration` from the
calculated fields, `source = ShiftSource::Auto`, `assignment_id = Some(job_id)`. Skip
`shift_category` (no `ShiftCategory` type exists — out of scope). Delete the old
placeholder-string test.

**Test (TDD, port verbatim):**
- Empty input → empty output (`testCreatePlannedShiftsFromWorkContentsEmptyWorkContentList`).
- `testCreatePlannedShiftsFromWorkContents()`'s 2-WorkContent case (different jobs, one with
  an explicit `assignment_id` set and one without, both `FORECAST`/`ORIGINAL` shift types
  produce a planned shift — not filtered by plan type): assert `shift_date`,
  `date_shift_generated_from`, `start_date_time`, `end_date_time`, `duration` all copied
  correctly from each `WorkContent`.

---

## Phase 14 — Wire up `BasicWorkGenerator`, `WorkGenerator` trait becomes fallible, clean dead code

**Model:** Sonnet 5

- `basic.rs`: update `generate_work` to call the processor (`?`), then the planned-shift
  creator, returning `Ok(WorkResults)` with both populated.
- **Shared trait change**: `generators/work_generators.rs`'s `WorkGenerator::generate_work`
  signature changes from `fn generate_work(&self, ...) -> WorkResults` to
  `fn generate_work(&self, ...) -> Result<WorkResults, GenerationError>`. Mechanical, not a
  rewrite: `NoneWorkGenerator`/`SalariedWorkGenerator`/`AdvancedWorkGenerator` are infallible
  today, so each just wraps its existing return in `Ok(...)`. This is what makes the
  "no panics in calculate" principle actually reach a caller, rather than stopping at BASIC's
  own internal boundary.
- Delete `basic_work_generator.rs` (0-byte dead file) + its `mod` declaration.

**Test (TDD, port):** `SimpleNonFlowedGeneratorTest`'s two cases — empty work-content list →
zero downstream calls (no planned-shift creation), `Ok(WorkResults)` with empty content;
non-empty list → processor called once, then planned-shift creation, in that order (Rust
doesn't have Spock's mock-interaction counting, so express this as: call the real Phase 12/13
functions in sequence and assert the final `Ok(WorkResults)` contains exactly the expected
`work_content`/`planned_shifts`). Also update `work_generators.rs`'s existing
`create_should_return_the_correct_generator` test and the 3 other generators' call sites for
the new `Result` return type.

---

## Phase 15 — Docs

**Model:** Sonnet 5

Rewrite `planner/src/workcontent/generators/basic/README.md` to describe the real pipeline
(`basic.rs` → `basic_standards_processor.rs` → `basic_environment_resolution.rs`/
`basic_total_work_minutes.rs`/`basic_work_minutes_per_unit.rs` → `basic_calculator.rs` (+
`basic_break_calculator.rs`) → `basic_work_content_creator.rs` → `basic_planned_shift_creator.rs`),
replacing the old outer-loop-only sketch, and note the functional-core/`Result`-based error
handling shape described above.

---

## Critical files

- `planner/Cargo.toml` — Phase 0
- `planner/src/workcontent/common/rounding.rs`, `generators/error.rs` (new) — Phase 1
- `planner/src/workcontent/domain/{meal_break,non_meal_break,job,job_shift,environment,kbi,shift_related_standard,planner_model}.rs` — Phases 2, 4–7, 12
- `planner/src/workcontent/generators/basic/*.rs` — Phases 3, 8–14 (new: `basic_break_calculator.rs`, `basic_environment_resolution.rs`, `basic_work_minutes_per_unit.rs`, `basic_total_work_minutes.rs`, `basic_calculator.rs`, `basic_work_content_creator.rs`; rewritten: `basic_standards_processor.rs`, `basic_planned_shift_creator.rs`, `basic.rs`)
- `planner/src/workcontent/generators/work_generators.rs` — Phase 12 (`WorkResults` extension), Phase 14 (`WorkGenerator` trait signature)
- `planner/src/workcontent/generators/{none,salaried,advanced}/*.rs` — Phase 14 (mechanical `Ok(...)` wrap only)
- Java ground truth — production: `JPR/workcontent/{simplenonflowed,calculator,utlities}/*.java`; tests: `JTR/workcontent/{simplenonflowed,calculator,utlities}/*.groovy` and `JTR/workcontent/eventrelated/plannedshifts/BreakLengthCalculatorTest.groovy`

## Verification (TDD discipline, applies to every phase)

1. Write the ported Rust test(s) for the phase first; run them; **confirm they fail** (no
   implementation yet, or a stub that can't satisfy them).
2. Implement the phase's code.
3. Run `cargo test -p planner`; iterate until green.
4. `cargo check -p planner` (and full `cargo test -p planner`) must stay green after every
   phase — each phase is additive or a scoped rewrite of one file, never leaves the build
   broken for a later phase.
5. Phases 1–3, 6, 7, 9–11 are pure-function layers, testable without any `PlannerModel`/`Job`
   fixture — do these before the orchestration phases (12–14), which need a full fixture and
   are the end-to-end proof the chain works together, including error propagation.
6. **Review checkpoints and context resets**: don't run all 16 phases in one unbroken
   session. Stop for human review — and consider clearing conversation context before
   resuming — at these boundaries:
   - **Checkpoint A** (after Phase 3): build unblocked, rounding + break-length math in
     place, all green. Low risk, quick to review.
   - **Checkpoint B** (after Phase 9): full domain model (`Environment`/`Kbi`/`ShiftRelatedStandard`)
     + the environment-aware resolution chain done. Review before touching the two Opus
     phases.
   - **Checkpoint C** (after Phase 11): the core calculation engine (`SimpleNonFlowedCalculator`
     + placement logic) complete — the highest-value review point in this plan, since Phases
     10 and 11 carry the subtlest correctness risk (raw-vs-clamped shift length, the
     shared-start-time placement behavior). Worth a careful diff read here even if earlier
     checkpoints were skimmed.
   - **Checkpoint D** (after Phase 14): the generator is fully wired and functional
     end-to-end.
   - **Checkpoint E** (after Phase 15): done.
   Context resets are safe at these boundaries because each checkpoint's only durable output
   is code + passing tests, and this doc is written to be sufficient standalone context for a
   fresh session to resume from. Don't reset mid-checkpoint (e.g. between Phase 10 and 11) —
   they share test fixtures and debugging one often illuminates the other.
