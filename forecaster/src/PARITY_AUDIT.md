# `forecaster` Parity Audit

> Updated continuously as each wave lands — not retroactively. Ground truth is
> `taps/forecaster/src/main/java/com/unifocus/watson/server/labor/forecaster/engine/`. See
> `PLAN_FORECASTER.md` (repo root) and `forecaster/src/DATA_MODEL.md` for the phased plan and data
> model this audit tracks against.

## Phase 0 — Data model & scaffolding

- Confirmed via direct read of `KBI.java`: no cross-module Java dependency, legacy `int` PKs
  throughout, module-name is not a trap (the `engine` package is the real algorithm).
- Id strategy: thin non-uuid newtypes (`KbiId`, `PropertyId`, `UnitId`, `KbiConfigId`,
  `StandardSetId`), diverging from `scheduler`'s bare-`i32` precedent — rationale in
  `DATA_MODEL.md` §1.
- No Java test suite exists (`src/test/java`, `src/test/groovy` both empty) — deviation from the
  `java_parity_tests` convention, called out explicitly in `DATA_MODEL.md` §6.

## Phase 1 — Formula engine

Ported `FormulaReader`, `Operation`/`AddOperation`/.../`ModulusOperation`, `KBICode`, `Function`
(all ~15 dispatch targets), and `KBIFormula.parse()`/`calculateFast()` into
`formula/{reader,operation,kbi_code,function,token,guard,context}.rs`. 26 unit tests, all green;
`cargo build --workspace` and `cargo clippy -p forecaster --all-targets` both clean (one intentional
warning: `FormulaReader::next` is named to match `FormulaReader.next()` in Java, not
`Iterator::next`).

Findings / deliberate deviations, in order of how much they matter:

- **`READKBI`/`READLABORKBI`/`READBUDGETKBI`'s "optional" trailing arguments are unreachable in
  Java.** `ReadLaborKBIFunction`'s javadoc claims the data-slot argument defaults to `ADJ` when
  omitted, and the constructor does set that default — but then *unconditionally* calls
  `parseDayOffset`/`parseDataSlot` next regardless, which resume scanning from the reader's current
  position (already past the call's closing paren, into whatever formula text follows) and
  unconditionally overwrite the default. The 1-/2-argument shortcut therefore either throws
  `NumberFormatException` or silently misparses trailing formula text — a latent bug, not a
  documented feature. The Rust port (`function.rs::parse_read_labor_kbi`/`parse_read_budget_kbi`)
  requires the full argument form and does not special-case the shortcut, matching what the Java
  engine actually does at runtime rather than what its javadoc claims.
- **KBI-id resolution moved from parse time to evaluate time.** Java's `KBICode` prefers
  `KBIList.getKBI(code)` and falls back to a raw SQL query; `ReadLaborKBIFunction`/
  `ReadBudgetKBIFunction`/`SameDayLastMonthFunction`/`PastAverageFunction` instead *always* run that
  SQL query, and do so eagerly from inside their constructors (i.e., during formula *parsing*).
  Since Phase 1 must stay DB-free, all five now resolve their referenced KBI id through
  `FormulaContext::kbi_id_for_code` at evaluation time instead — the same trait method, called at a
  different point in the pipeline. No behavioral difference once Phase 3 backs the trait, since
  Java's own `KBIList`-first path already amounts to "resolve on demand."
- **`DAYSINPERIOD` is a Java no-op, preserved as one.** `DaysInPeriodFunction.calculate()` always
  returns `0.0` regardless of its parsed period-offset argument. `FunctionCall::DaysInPeriod`
  carries no data and `evaluate_function_call` returns `0.0` unconditionally — not a simplification,
  a literal translation of dead-looking-but-real behavior.
- **`@STAT` / `StatisticalFunction` is not ported.** It is never reachable from
  `Function.createFunction`'s dispatch chain in the Java source — confirmed dead code.
- **Three DB-backed read paths collapsed into one `FormulaContext::read_kbi_stat_value` method.**
  `KBICode.calculate`, `ReadLaborKBIFunction.calculate`, and `SameDayLastMonthFunction.calculate`
  each independently implement "read the override value, else the per-stat-type value, defaulting
  to 0.0" — `ReadLaborKBIFunction`/`SameDayLastMonthFunction` additionally branch on the concrete
  `KBIReaderWriter` subclass, a branch that is always taken for `LaborKBIReaderWriter` (the only
  concrete reader/writer this engine ever runs with, per `PLAN_FORECASTER.md`). One trait method
  models all three call sites; see `context.rs`'s doc comment.
- **`DaysInPeriodFunction`'s period-offset bound check (`numPeriods`, from the property's
  financial-year-type) is not enforced.** That check requires a config lookup Phase 1 doesn't have;
  deferred to whichever phase wires up `Property`/financial-year config, and moot for computed
  values regardless since `calculate()` ignores the offset entirely.
- **AVG/SUM/MAX/MIN's sub-expressions are parsed once, not per-call.** Java constructs a fresh
  `KBIFormula` for every sub-expression on every `calculate()` invocation (re-parsing its text from
  scratch each time); the Rust port parses them into a `Vec<FormulaToken>` once, at the outer
  formula's parse time, per `DATA_MODEL.md`'s "parse once, no lazy caching" decision. Pure
  performance difference, no behavioral one.
- **The `computing` recursion guard is defined (`formula/guard.rs::FormulaGuard`) but not yet
  wired into `evaluate()`.** Tracing the actual Java re-entrancy path shows the guard only matters
  at `CalculatedKBI.computeValue()` calling back into its own cached `KBIFormula.calculate()`
  through a `#code` reference resolved by `KBIList.getKBI` — nothing in the pure formula
  AST/evaluator itself re-enters a KBI's own computation (`KBICode`/`Function` only read already-
  materialized values through `FormulaContext`). `FormulaGuard` is ready (tested: direct recursion,
  release-then-reenter, unrelated KBIs don't collide) for Phase 2 to thread through
  `Kbi::compute_value` once that method exists, rather than wiring it in prematurely here.

## Phase 2 — KBI domain + statistical analyzers

Ported the `KbiType`/`KbiMode`/`StatOpType` enums (`kbi/{kbi_type,kbi_mode,stat_op_type}.rs`), the
`StatKbiType`/`StatRelatedKbi`/`StatKbi` trio (`kbi/stat_kbi.rs`), the `Kbi` domain enum
(`kbi/domain.rs`), and the full statistical-analyzer stack (`analyzers/{row_data,stats,rstats,taes,
past_average,percent_of_base,statistical_kbi_analyzer,regression}.rs`). 74 unit tests total (up
from Phase 1's 26), all green; `cargo build --workspace` clean.

Findings / deliberate deviations, roughly in order of how much they matter:

- **`RegressionAnalyzer`/`RowData` are far more I/O-saturated than `PLAN_FORECASTER.md`'s Phase 2
  description assumed.** The plan grouped `RegressionAnalyzer` with `RStats`/`TAESCalculator` under
  "pure numeric algorithms." Reading `RegressionAnalyzer.java`/`RowData.java` directly shows nearly
  every branch reads through `KBIReaderWriter`/`LaborKBIReaderWriter` (season/environment lookups,
  four different KBI-value read paths, `readKBIStatData`). It is ported in full regardless —
  `analyzers/regression.rs`'s `RegressionLookupPort` trait has one method per DB access surface
  (`DATA_MODEL.md` §3's "narrow port trait" treatment), and `RegressionAnalyzer::calculate` mirrors
  the Java control flow line-for-line, including the compress-loop/outlier-rejection/
  Durbin-Watson-first-difference structure — only the actual DB reads are deferred to Phase 3. Same
  finding applies to a lesser extent to `TAESAnalyzer`/`PercentOfBaseKBIAnalyzer`.
- **`PLAN_FORECASTER.md`'s `FNYPeriod` reference for `PercentOfBaseKBIAnalyzer` was wrong.**
  Confirmed by reading `PercentOfBaseKBIAnalyzer.java` directly and grepping the package for
  `FNYPeriod`: it only appears in `KBICode.java`/`KBIReaderWriter.java` (Phase 3 territory), never
  in `PercentOfBaseKBIAnalyzer`. That class's only I/O is `readKBIOverrideValue`/`readKBIValue` (or,
  in the dead budget-mode branch, a recursive `computeValue` call) — modeled as
  `PercentOfBaseLookupPort` in `analyzers/percent_of_base.rs`.
- **`LaborKBIReaderWriter` confirmed (by grep) to be the only concrete `KBIReaderWriter` subtype**
  in this Gradle module. Every `instanceof LaborKBIReaderWriter` / non-Labor "else" branch across
  `RowData`, `RegressionAnalyzer`, `PastAverageAnalyzer`, and `PercentOfBaseKBIAnalyzer` is dead
  code and was not ported — the same precedent `formula::context` already established in Phase 1.
- **`INDEPENDENT_VALUE_OVERRIDE_QUIRK` (`analyzers/regression.rs`).** In
  `RegressionAnalyzer.calculate`'s independent-value-lookup block, the non-actual-mode branch for a
  related KBI that is *not* the analyzed KBI still calls `kbiReaderWriter.readKBIOverrideValue(kbi,
  date)` — the analyzed KBI's own override, not the related KBI's. Looks like a copy-paste bug
  (every sibling branch is keyed off whichever KBI is actually in scope) but it's what production
  runs today; ported verbatim, flagged with a doc comment at the call site rather than "fixed."
- **`RStats.gaussJordan`'s row-swap bug ported verbatim, not fixed.** Java's row-swap loop reads
  `invary[row * numCol + 1]` — a literal `1` instead of the loop variable `l` — so every swapped
  value beyond the first two columns is wrong. `RStats::swap_rows_if_needed` in `rstats.rs`
  reproduces this exactly (documented inline); fixing it would silently change every forecast that
  flows through a regression with 3+ independents. The Phase 2 regression test suite only exercises
  1-independent regressions (`numCol == 2`, where the bug happens not to bite), so it doesn't mask
  this — flagging here for anyone tempted to "clean up" `RStats` later.
- **`TAESCalculator`'s mutating-lambda alpha/beta grid search rewritten as plain nested loops**, per
  the plan's explicit Phase 2 guidance (`analyzers/taes.rs::find_alpha_and_beta`) — behaviorally
  identical, just not expressed as `IntStream.rangeClosed(...).forEach` closures mutating `this`.
- **`TAESAnalyzer.adjustResultForFutureDates`'s day-count arithmetic assumes
  `ArbitraryDateRange.getNumberOfDaysInRange()` counts both endpoints inclusively** (`end - start +
  1`), which cancels against Java's own `- 1` to equal a plain day-count difference —
  `joda_rs`'s `(LocalDate - LocalDate) -> Duration` / `Duration::to_days()` gives that difference
  directly, so `adjust_result_for_future_dates` uses it with no extra `-1`. This is inferred from
  the arithmetic, not confirmed against `date_range_rs`'s actual semantics — worth double-checking
  against `taps/`'s `ArbitraryDateRange`/`date_range_rs`'s `DateRange::len()` before Phase 3 wires a
  real `TaesLookupPort`.
- **`StatKBI.numberDataPoints` is dead state, kept for documentation only.** `StatKBI` has a
  getter/setter for it (implying a `PAST_AVERAGE` stat mode), but `StatKBIType` only has
  `TAES`/`REGRESSION` variants, and `StatisticalKBI.load()` never sets it. `kbi::StatKbi` carries
  the field with a doc comment explaining why it's unused rather than dropping it silently.
- **`Kbi::compute_value` is not implemented.** Every `KBI` subtype's `computeValue()` override in
  Java does nothing but delegate to a `KBIReaderWriter` method (`computeInputKBI`/
  `computeCalculatedKBI`/etc.) — that's the Phase 3/4 I/O boundary, not domain logic. What's pure —
  `isEditableKBI`/`isDanglingKBI`/`isOpen` — is ported in `kbi/domain.rs`; `compute_value` itself
  waits for Phase 4's orchestrator to have real ports to dispatch through.
- **`ForecastMode` is still not ported**, per `DATA_MODEL.md` §2's original Phase 4 scoping.
  `RegressionAnalyzer`'s one dependency on it (`getMode() == ForecastMode.ACTUAL`) is modeled as a
  single `RegressionLookupPort::is_actual_mode() -> bool` instead of pulling the whole enum forward.
- **`PercentOfBaseKbiAnalyzer` drops Java's `computing` recursion guard.** That flag only guards the
  budget-mode branch's recursive `baseKBI.computeValue(...)` call, which collapsed into
  `PercentOfBaseLookupPort::read_base_kbi_value` (per the `LaborKBIReaderWriter`-only
  simplification) — there's no recursion left at this layer to guard.
- **`StatisticalKBIAnalyzer`'s only DB dependency is `KBIReaderWriter.getDates()` → `TDatePeriod
  .getAbsoluteDayIndex`**, collapsed into one `StatisticalDispatch::absolute_day_index` port method
  (`analyzers/statistical_kbi_analyzer.rs`). The dispatch decision itself (TAES vs. regression,
  which related-KBI list to hand off) is a plain match, ported and tested directly — no fake needed
  beyond the index lookup.

Deferred to Phase 3 (needs the real port implementations to exercise beyond a mock):
`RegressionLookupPort`, `PastAverageLookupPort`, `PercentOfBaseLookupPort`, `TaesLookupPort`,
`StatisticalDispatch` all need real `LaborKBIReaderWriter`-backed implementations.

## Phase 3 — Reader/Writer I/O boundary

Ported `KBI.load()`'s SQL-issuing steps and `KBIReaderWriter`/`LaborKBIReaderWriter`'s
mode-dependent value read/write selection logic on top of four narrow port traits
(`io/ports.rs`), plus supporting types `KbiStatData` (`io/kbi_stat_data.rs`), `ForecastMode`
(`io/forecast_mode.rs`), and `FnyPeriod` (`io/financial_year_period.rs`). 25 new unit tests (74 →
99 total), all green; `cargo build --workspace` clean.

Findings / deliberate deviations:

- **`RevenueCenterConfigPort`/`MarketSegmentConfigPort` collapsed into one `KbiLoaderPort`.**
  `PLAN_FORECASTER.md`'s Phase 3 checklist named these as two separate port traits (mirroring the
  plan's original guess at `LaborKBIReaderWriter`'s query surfaces). Reading `KBI.java` directly
  shows they're only ever called together, from `KBI.load()`, to populate one KBI's rooms/
  departures/revenue-center/days-open flags in sequence (`setRoomsFlags` → `setRevenueFlags` →
  `loadDaysOpen`) — modeled as `KbiLoaderPort`'s three methods plus `load_kbi_flags` in
  `io/loader.rs`, which reproduces the exact call sequence (`setRevenueFlags` only runs
  `if (!isRoomsKBI)`, and `loadDaysOpen` only runs if a `RevenueCenterPeriod` row was found).
- **`CalendarPlanPort` stays deferred to Phase 4.** It backs `ForecastThread.checkRevenueCenters`
  only (a 3-year calendar-plan-date rebase), which is Phase 4's orchestrator territory — nothing in
  `KBIReaderWriter`/`LaborKBIReaderWriter`'s own methods touches it, so defining it now would be
  speculative ahead of its one call site.
- **`ForecastMode` ported in this phase, not Phase 4, despite `DATA_MODEL.md` §2's original
  scoping.** `KBIReaderWriter.readKBIValue`/`writeKBIValue` — this phase's actual subject — switch
  on every `ForecastMode` variant directly (6-way branches in both directions), so there was no way
  to port "the mode-dependent read/write selection logic" (Phase 3's own checklist item) without
  the enum. `io/forecast_mode.rs` notes the deviation from `DATA_MODEL.md`.
- **`Kbi::compute_value` still isn't implemented** (Phase 4, since every Java `KBI` subtype's
  `computeValue()` override just delegates to a formula/analyzer that Phase 4's orchestrator
  assembles). `KbiReaderWriter::read_kbi_value` takes the `kbi.computeValue(date, statType)` call
  as a `compute` closure parameter instead of calling into `Kbi` directly — same shape as Phase 2's
  analyzer ports taking DB reads as trait methods rather than blocking on a concrete
  implementation.
- **The "KBI stat value was zeroed out" sanity check in `writeKBIValue` (Java lines ~296-323) is
  not ported.** It's a debug `System.out.println` block wrapped in `try { ... throw new
  RuntimeException(...) } catch (Exception e) { e.printStackTrace(); }` — the exception it throws
  is caught and swallowed one line later, so it has zero effect on control flow or persisted state.
  Not modeling dead diagnostic scaffolding matches the precedent from Phase 1's dead `@STAT`
  formula branch.
- **`recordExists`/insert-vs-update branching folded into `KbiStatWriterPort::write_kbi_stat_data`
  as one method**, not exposed as a separate port method. `LaborKBIReaderWriter.writeKBIStatData`
  treats "does a row exist" purely as an implementation detail of its own upsert (the exists-check
  query only ever gets called from inside that one method) — same "one port method per DB access
  surface, not per query" precedent `scheduler`'s save-style ports set.
- **`KBIReaderWriter.getKBIStatData`/`putKBIStatData` (the `kbiValueMap` cache) modeled as a plain
  `HashMap<(i32, LocalDate), KbiStatData>` field on the new `KbiReaderWriter` struct**, keyed by
  raw `kbiID` rather than `KbiId` (a tuple map key needs `Eq + Hash`, which `KbiId` already derives
  — using the raw `i32` was just simpler for the key tuple and costs nothing since `KbiId` is a
  thin newtype).
- **`day_of_week`/date-range-start passed as explicit parameters to `read_kbi_value`**, not
  computed from `LocalDate`/a `TDatePeriod` internally — same treatment `KbiRecord::is_open`/
  `StatisticalKbiData::is_taes_kbi` already established in Phase 2 (`TDate.dayOfWeek()`→`i32`
  conversion and the forecast date range are Phase 4 orchestrator concerns).
- **`find_period_for_date`'s "date has an `FNYPeriod` but no period within the first `numPeriods`
  matches" case returns `None`, matching Java's fall-through-with-`periodNo`-still-`0`, not an
  error.** Flagged in the function's doc comment since callers (Phase 4) must map `None` back to
  period `0`, not skip the KBI, to match `computePercentOfBaseKBI`'s behavior exactly (the Java
  loop never resets `periodNo` from its initial value).

Deferred to Phase 4: `CalendarPlanPort`, `Kbi::compute_value`, and wiring `KbiReaderWriter`'s
`compute` closures to real `Kbi`/formula/analyzer calls.

## Phase 4 — Orchestrator

Ported `ForecastThread.run()`/`processKBIs`/`processKBI`/`checkMarketSegments`/`checkRevenueCenters`/
`getKbiSetIdForStandardSetId` as plain functions (`engine::orchestrator::{run_forecast,
process_kbis, process_kbi, check_market_segments, check_revenue_centers}`), plus `Kbi::compute_value`
(`engine::compute::compute_kbi_value`, dispatching to the Phase 1 formula evaluator and Phase 2
analyzers — the piece every earlier phase deferred), `KBIList` (`engine::kbi_list::KbiList`), and
three new orchestrator-level port traits (`engine::ports::{KbiSetPort, MarketSegmentCheckPort,
RevenueCenterCheckPort}`, distinct from `io::ports`'s `KBIReaderWriter`-level surfaces). 8 new unit
tests (99 → 107 total, `analyzers`' 3 pre-existing `taes` tests plus Phase 4's `java_day_of_week`,
`KbiList`, and the four `check_market_segments`/`check_revenue_centers` tests), all green; `cargo
build --workspace` clean.

Findings / deliberate deviations, roughly in order of how much they matter:

- **`read_kbi_value`'s return type and `compute` closure changed from `f64` to `Option<f64>`,
  amending Phase 3's signature.** Reading `KBIReaderWriter.readKBIValue`/`KBI.computeValue` closely
  while wiring the real `compute` callback in shows `computeValue()` is a genuinely nullable
  `Double` in Java — `RegressionAnalyzer.calculate`/`TAESAnalyzer.calculate` can both return `null`
  (a mis-configured regression with zero independent variables; no recent actuals for a TAES KBI),
  and that `null` propagates all the way through `readKBIValue`'s own return value. Phase 3's
  `KbiReaderWriter::read_kbi_value` (`io/reader_writer.rs`) originally always wrapped `compute`'s
  result in `Some(...)`, which cannot represent this. Both the `compute` closure's return type and
  `read_kbi_value`'s own return type are now `Result<Option<f64>, ForecasterError>`; the four
  existing Phase 3 unit tests were updated to the new shape (`Ok(Some(7.0))` etc.) and one new test
  (`compute_returning_none_propagates_as_none`) covers the previously-unrepresentable case.
  `write_kbi_value`'s signature already took `value: Option<f64>` and needed no change.
- **`KbiSetPort::load_kbi_list` hands back fully-built `Kbi` domain objects, not raw rows.** There
  is no `TRecordset` equivalent in this crate (the "no persistence layer of its own" stance
  `PLAN_FORECASTER.md`'s Verification section sets, same as `scheduler`), so the multi-table load
  `KBIList.loadKBIs`/`KBI.createKBI`/each subtype's own `load()` override performs (`KBIConfig`/
  `KBIBudgetConfig`/`KBILaborConfig`/`KBIConfigFormula`/`KBIConfigStatistical`/`KBIConfigTAES`/
  `KBIConfigRegression`/`KBIConfigPercent`, plus `KBI.load()`'s flag loading) collapses into one
  port method returning `Vec<Kbi>` — the same "port returns the parsed domain type, not raw
  columns" precedent `KbiStatReaderPort::read_kbi_stat_data` already set in Phase 3.
  `KBIList.loadBasisForPercentOfBaseKBIs` needs no separate modeling: `PercentOfBaseKbiData
  ::base_kbi_id` (an id, not a `KBI` object reference) is all `PercentOfBaseLookupPort
  ::read_base_kbi_value` (Phase 2) ever needed.
- **`compute_kbi_value`'s Input-KBI branch reads through the port directly, bypassing
  `KbiReaderWriter`'s cache.** `computeInputKBI` calls `readKBIStatData(kbi, date)`, which in Java
  is itself cache-then-DB (`LaborKBIReaderWriter`'s `getKBIStatData`/DB-read/`putKBIStatData`
  sequence) — the same cache `read_kbi_value` already checked immediately before falling through to
  `compute`. The `compute` closure passed into `read_kbi_value` doesn't have mutable access to that
  cache (it's a private field on `&mut self`, and the closure only captures `&ComputeContext`), so
  `compute_kbi_value`'s Input branch calls `KbiStatReaderPort::read_kbi_stat_data` directly. This
  re-reads the same idempotent row a second time in the worst case rather than reusing a
  freshly-populated cache entry — a performance-only difference once a real cache-aware port
  implementation exists, not a behavioral one.
- **A formula-less `CalculatedKBI` (no `KBIConfigFormula` row, `formula_text: None`) computes to
  `0.0` rather than reproducing whatever NPE Java would hit** constructing a `KBIFormula` around a
  `null` string and then calling `.calculate()` on it. Not a real production configuration Java
  ever expects to run with, so a typed `0.0` result beats matching a crash.
- **`PercentOfBaseKBIAnalyzer.getPercents(periodNo)` indexes its percents list *positionally*, not
  by matching a `KBIPercent.periodNo` field value** — confirmed by reading the Java source
  directly (`percents.get(periodNo)`). `compute_percent_of_base` (`engine/compute.rs`) does the
  same (`percents.get(period_no)`); an out-of-range index is a typed `ForecasterError::DataAccess`
  here rather than Java's unchecked `IndexOutOfBoundsException`.
- **`checkRevenueCenters`'s `RevenueCenterPeriodDay` loop has a confirmed Java quirk, ported
  verbatim.** `break` inside the `for (TDate date = startDate; ...)` loop exits after writing the
  *first* date in range matching a closed `DayNo` — not every matching date. Looks like it should
  zero every occurrence of that weekday across the forecast range but doesn't; production runs this
  today, so it's ported as-is (`check_revenue_centers`, with a dedicated regression test:
  `revenue_centers_period_day_quirk_only_zeroes_the_first_matching_date`).
- **`checkMarketSegments`'s departures-KBI write is unconditional, not gated behind the
  `departs < 0.0` branch** — only the rooms/arrivals rewrites are inside that `if`; the departures
  write at the bottom of the Java method runs whenever a departures KBI exists, regardless of which
  branches fired above. Easy to misread on a first pass (an earlier draft of this port's own test
  suite got it backwards); `market_segments_caps_arrivals_at_rooms_without_touching_departures_when_nonnegative`'s
  doc comment calls this out explicitly.
- **`ForecastMode.ACTUAL && kbi.getKBIType() == CALCULATED && kbi.getKBIType() == CALCULATED`**
  (`processKBIs`'s last `else if`) is Java's literal, redundant-but-harmless condition (the same
  clause repeated by copy-paste); ported as the single check it actually reduces to
  (`record.kbi_type == KbiType::Calculated`).
- **`java_day_of_week`** (`engine/compute.rs`) is a new conversion helper, not a Java method port:
  `joda_rs::LocalDate::day_of_week()` is ISO-based (Monday = 1..Sunday = 7), but every Java call
  site this crate touches (`KBI.daysOpen`, `RevenueCenterPeriodDay`/`CalendarPlan`'s `Sun`-first
  column ordering, `PercentOfBaseKBIAnalyzer`'s `dayOfWeek() - 1` indexing) assumes `TDate
  .dayOfWeek()`'s Sunday = 1..Saturday = 7 convention. This is the first phase to derive a
  day-of-week from an actual `LocalDate` rather than taking one as a hardcoded test parameter, so
  the conversion had to be added here rather than reused from an earlier phase.
- **Interruption/progress (`isInterrupted()`, `getProgress().ping()`/`setMessage()`,
  `fireThreadFinished`) is not ported**, per `PLAN_FORECASTER.md`'s explicit "TBD hook" scoping —
  there is no cooperative-cancellation or progress-reporting infrastructure in this crate to hook
  into. `run_forecast` returns `Result<ForecastOutcome, ForecasterError>` in place of Java's
  `success` boolean field + `Progress.propagateException` combination.
- **`KBIReaderWriter.close()`'s empty base implementation (no override in `LaborKBIReaderWriter`
  either) is not ported** — there is nothing to close.
- **`CalendarPlanDateRange::contains_in_year` guards against invalid rebased dates** (e.g. a
  Feb 29 `CalendarPlanDate` rebased onto a non-leap year via the 3-year `years[]` array) rather than
  calling `joda_rs::LocalDate::of` directly, which panics on an invalid year/month/day combination.
  Java's own `new TDate(month, day, year)` behavior for this exact edge case wasn't independently
  verified against `taps/`; treating it as "doesn't match" (returns `false` from
  `contains_in_year`) rather than crashing was judged the safer default for a port with no test
  coverage of that specific Java behavior.

Remaining scope, explicitly out of bounds for this crate per `PLAN_FORECASTER.md`'s "no persistence
layer of its own" Verification note: every port trait across all four phases still needs a real
DB-backed implementation before this engine can run against actual data.
