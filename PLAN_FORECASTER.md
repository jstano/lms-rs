# Plan: Port the Java `forecaster` engine to the Rust `forecaster` crate

## Context

`lms-rs` ports the UniFocus Java monorepo (`taps/`) crate by crate, following conventions
established by `planner` (workcontent generator) and `scheduler`/`workrules` (engine ports). The
`forecaster` crate exists today only as an untouched `cargo new` stub (`forecaster/src/lib.rs`
still has the default `add()` function, `Cargo.toml` has zero dependencies) — a workspace member
with nothing built yet.

The target is `taps/forecaster/src/main/java/com/unifocus/watson/server/labor/forecaster/engine/`
— the **KBI (Key Business Indicator) forecasting engine** for the hospitality/labor system: given a
property, date range, and standard set of KBIs, it computes forecast/actual values per day
(rooms, arrivals, departures, guests, revenue counts) that downstream labor scheduling
(`planner`/`scheduler`) consumes. Unlike `scheduler`, there is **no "module name is a trap" issue**
here — confirmed by reading `ForecastThread.java`/`KBI.java`/`KBIFormula.java` directly: the
`forecaster` Gradle module's `engine` package *is* the real algorithm, not a CRUD/API shell.

Scope: **`server/labor/forecaster/engine/` only** — 70 files, the entire KBI compute engine. The
sibling `common/`, `labor/data/`, `shared/` packages (29 files: enums, `Property`, `FNYPeriod`,
session/security types) are supporting cross-cutting types the engine depends on but does not own;
port only the slices the engine actually touches, as narrow local stubs (same treatment
`scheduler` gave `Assignment`/`Employee`/`Property`), not full ports of those types.

**No cross-module Java dependency** (verified via `build.gradle` and imports): only
`legacy-tbx-core`, `framework-jdbc`/`framework-joda`/`framework-spring-core`, and Joda-Time. No
imports of `workrules`, `scheduler`, `contract`, `certification`, or `systemsetup`. `forecaster`
will be **self-contained**, like `workrules`/`scheduler`/`planner` — its own newtypes, no
cross-crate dependency.

**No existing Java test suite** — `src/test/java` and `src/test/groovy` are configured (Groovy/
Spock, `framework-test`) but contain zero files. This is a deviation from the `java_parity_tests`
convention used for `planner`/`scheduler`/`workrules`: there is no Java suite to transcribe. Tests
must be hand-derived from reading the algorithm (especially the formula parser/evaluator and the
regression/TAES numeric methods) — call this out explicitly in `DATA_MODEL.md` rather than silently
skipping the convention.

## Id strategy

Every id here is a **legacy Hibernate/DB `int` primary key** — `KBI.id`, `KBI.propertyID`,
`KBI.unitID`, `KBI.kbiConfigId`, `standardSetId`, etc. (confirmed in `KBI.java`). This matches
`scheduler`'s precedent, not `planner`'s: **bare `i32`, no `id_type!` uuid macro.** `KBIId`,
`PropertyId`, `StandardSetId`, `UnitId` etc. can be plain `i32` (or thin non-uuid newtypes if
clarity is worth it — decide in Phase 0, not before).

## Architecture / data model

Four areas, in increasing order of I/O-boundary involvement — this also sets the phase order below.

**1. Formula engine (pure, self-contained, no DB) — the most "rule-engine"-shaped piece.**
`FormulaReader` (char-cursor tokenizer with mark/reset) → `KBIFormula.parse()` tokenizes formula
text (`#code`, `@Func(...)`, `(subformula)`, numeric constants, `+ - * / %`) into a flat
`List<Object>` → `calculateFast()` evaluates it with a two-stack (operands/operations) shunting-yard
algorithm honoring `Operation.getPriority()`. `Function` is a plugin dispatch table (`AVG`, `SUM`,
`MAX`, `MIN`, `ARRIVALSTTL`, `REVTTL`, `ROOMSTTL`, `SDLM`, `READKBI`, `READLABORKBI`,
`READBUDGETKBI`, `DAYSINPERIOD`, `PAVG`, `CFGROOMS`, ~15 total). Translation notes:
- Java's raw `List parseList`/`Object` items + `instanceof` chains → a typed Rust AST enum
  (`enum FormulaToken { KbiRef(KbiCode), Function(FunctionCall), Constant(f64), SubFormula(...),
  Operation(Op) }`) instead of `Object`.
- `KBIFormula` is **not stateless**: it lazily caches `parseList` on first `calculate()` and holds
  a mutable `computing` flag as a recursion guard. Parse once into an immutable AST at construction
  (or on first use with a `OnceCell`), and thread the recursion guard as an explicit parameter/stack
  rather than a mutable field, since the Rust evaluator will be `&self`-based.
- No `BigDecimal` anywhere — all KBI values are `Double`; the Rust port uses `f64` throughout, with
  attention to null-vs-`0.0` handling (Java's `Double` boxing lets values be `null`; several call
  sites explicitly substitute `0.0` when a sub-result is `null` — preserve each site's behavior
  rather than normalizing).

**2. KBI domain hierarchy + statistical analyzers (pure numeric, no DB once inputs are loaded).**
`KBI` (abstract, `computeValue()`) with `InputKBI`/`CalculatedKBI`/`StatisticalKBI`/
`PercentOfBaseKBI` subclasses, dispatched via `KBI.createKBI()` on `KBIType`. Maps to a Rust enum
(`enum Kbi { Input(..), Calculated(..), Statistical(..), PercentOfBase(..) }`) with a
`compute_value` match rather than a class hierarchy. Numeric algorithms to port faithfully:
- `RegressionAnalyzer`/`RStats` (hand-rolled multiple linear regression, matrix build/invert, fixed
  `MAXSAM=13`/`MAXCOL=15` limits — port the fixed-size array bounds literally).
- `TAESCalculator`/`TAESAnalyzer`/`TAESResult` (trend-adjusted exponential smoothing, grid-searches
  alpha/beta in [0,1] step 0.01 minimizing MAD). Java does this via `IntStream.rangeClosed(...)`
  lambdas that **mutate enclosing instance fields** — this won't translate directly to Rust
  closures under borrow rules; rewrite as plain nested loops accumulating into local variables.
- `PastAverageAnalyzer`/`PastAverageFunction`, `SameDayLastMonthFunction`,
  `PercentOfBaseKBIAnalyzer` (ties into `FNYPeriod`/financial-year-period lookups — narrow-stub that
  lookup behind a port trait rather than porting `FNYPeriodsSqlImpl` wholesale).
- `StatisticalKBIAnalyzer` dispatches to Regression/PastAverage/TAES based on `StatOpType`/
  `KBIMode` — straightforward match/dispatch.

**3. Reader/Writer boundary — the I/O-heavy piece, treat as ports (scheduler convention).**
`KBIReaderWriter` (abstract) → `LaborKBIReaderWriter` (1124 lines, **all raw SQL** via legacy
`TQuery`/`TRecordset`/`Connection` — no Hibernate/JPA anywhere in this module). Also note: `KBI.load()`
itself runs SQL during "loading" (`setRoomsFlags`/`setRevenueFlags`/`loadDaysOpen` each issue their
own query) — this is not a simple field-population step, it's part of the I/O boundary too. Follow
`scheduler::engine::io::ports.rs`'s pattern exactly: one narrow port trait per DB access surface,
named after what it does (`KbiLoaderPort`, `KbiStatReaderPort`, `KbiStatWriterPort`,
`FinancialYearPeriodPort`, `RevenueCenterConfigPort`, `MarketSegmentConfigPort`, `CalendarPlanPort`
— derived from `LaborKBIReaderWriter`'s and `ForecastThread`'s actual query call sites, not
speculative), each method mirroring exactly one query/mutation the Java code performs, with a doc
comment citing the Java source method. Several interpolated-int SQL strings in the Java (e.g.
`"WHERE PropertyID = " + getProperty().getId()`) are a reminder to design the Rust port trait
signatures as typed parameters, not string-built SQL, even though no real implementation is written
in this crate yet.

**4. Orchestrator — `ForecastThread` → a plain function/struct, no `Thread`/`Progress` machinery.**
`ForecastThread.run()` (read directly, ~480 lines) is the literal top-level driver:
1. Load KBIs for the standard set (`getKbiSetIdForStandardSetId`, `KBIList.loadKBIs`).
2. `processKBIs(startDate, endDate)`: for each date in range, for each KBI, skip based on
   `ForecastMode` (`ROOMS`/`REVENUE`/`UPDATE_SYS`/`UPDATE_FST`/`UPDATE_ACT`/`ACTUAL`→`UPDATE_ACT`)
   × KBI type/flags, else `readKBIValue` → `writeKBIValue`.
3. Mode-conditional post-processing: `checkMarketSegments` (ROOMS mode — reconciles
   rooms/arrivals/guests/departures per market segment, several inline business rules, e.g.
   "arrivals can't exceed rooms", "departures can't be negative") or `checkRevenueCenters` (REVENUE
   mode — zeroes out KBI values on days a revenue center's calendar plan / weekly open-day config
   says it's closed, including a 3-year calendar-plan-date rebase: `years[0]=periodYear-1,
   years[1]=periodYear, years[2]=periodYear+1`).
- Java's `Thread`/`EngineThread`/`Progress`/`IThreadListener` (cooperative-interrupt, progress
  callback) machinery is infrastructure, not algorithm — the Rust port models
  `run_forecast(params, ports) -> Result<ForecastOutcome, ForecasterError>` as a plain call, with
  interruption/progress left as a TBD hook (a trait or channel argument) rather than modeled now,
  same spirit as `scheduler` dropping Hibernate/session concerns at its I/O boundary.
- `KBIComputeException` (carries a KBI code)/`InvalidDateException`/`DataAccessException` map to a
  single `ForecasterError` enum (following `planner`'s `GenerationError` precedent) with variants
  preserving KBI-code/date context.

## Critical files (Java ground truth, `taps/forecaster/src/main/java/com/unifocus/watson/server/labor/forecaster/engine/`)
- `ForecastThread.java` — orchestrator (Phase 4)
- `KBIFormula.java`, `FormulaReader.java`, `Function.java`, `Operation.java`, `KBICode.java` —
  formula engine (Phase 1)
- `KBI.java`, `InputKBI.java`, `CalculatedKBI.java`, `StatisticalKBI.java`, `PercentOfBaseKBI.java`,
  `KBIType.java`, `KBIMode.java` — domain hierarchy (Phase 2)
- `RStats.java`, `RegressionAnalyzer.java`, `TAESCalculator.java`, `TAESAnalyzer.java`,
  `PastAverageAnalyzer.java`, `PercentOfBaseKBIAnalyzer.java`, `StatisticalKBIAnalyzer.java` —
  numeric algorithms (Phase 2)
- `KBIReaderWriter.java`, `LaborKBIReaderWriter.java` — I/O boundary (Phase 3)

Rust reference points: `scheduler/src/engine/io/ports.rs` (port-trait style), `PLAN_SCHEDULER.md`
and `scheduler/src/DATA_MODEL.md` (doc/phase structure), `planner/src/workcontent/generators/error.rs`
(single crate-wide error enum style).

## Verification

Since this crate has no persistence layer of its own (ports are stubs pending real DB access), same
as `scheduler`:
- `cargo test -p forecaster` passing per phase, with hand-derived unit tests (no Java suite exists
  to transcribe) — Phase 1's formula evaluator and Phase 2's regression/TAES math are the highest-
  value places to build confidence via synthetic inputs with manually-computed expected outputs.
- `cargo build --workspace` staying green throughout (additive crate, no cross-crate coupling to
  break).
- Periodic re-read of `ForecastThread.java` and the current phase's Java source under `taps/` to
  confirm no behavioral drift during translation — ground truth is always `taps/`, never a local
  copy.

## Progress

### Phase 0 — Data model & scaffolding
- [x] Write `forecaster/src/DATA_MODEL.md` (id strategy, enum translations, `ForecasterError`
      shape, explicit note on the "no Java test suite" deviation)
- [x] Create `forecaster/src/PARITY_AUDIT.md` (empty skeleton, updated continuously from here on)
- [x] Add `joda_rs`, `date_range_rs` to `forecaster/Cargo.toml` (no `uuid` needed)
- [x] Stand up module layout: `src/{formula, kbi, analyzers, io, engine}`
- [x] Decide bare-`i32` vs thin newtypes for `KBIId`/`PropertyId`/`StandardSetId`/`UnitId` —
      thin non-uuid newtypes (`KbiId`/`PropertyId`/`UnitId`/`KbiConfigId`/`StandardSetId` in
      `lib.rs`), diverging from `scheduler`'s bare-`i32`; rationale in `DATA_MODEL.md` §1

### Phase 1 — Formula engine (pure, no DB) — ✅ complete
- [x] Port `FormulaReader` (char-cursor tokenizer: mark/reset/next/previous)
- [x] Define the `FormulaToken` AST enum (replacing Java's raw `List<Object>` + `instanceof`)
- [x] Port `Operation` + `AddOperation`/`SubtractOperation`/`MultiplyOperation`/`DivideOperation`/
      `ModulusOperation` (priority-based)
- [x] Port `KBICode` (sibling KBI value resolution)
- [x] Port `Function` dispatch + concrete functions: `AVG`, `SUM`, `MAX`, `MIN`, `ARRIVALSTTL`,
      `DEPARTSTTL`, `GUESTSTTL`, `REVTTL`, `ROOMSTTL`, `CFGROOMS`, `SDLM`, `READKBI`,
      `READLABORKBI`, `READBUDGETKBI`, `DAYSINPERIOD`, `PAVG` — I/O-bound and KBI-lookup behavior
      modeled behind `formula::FormulaContext`; see `PARITY_AUDIT.md` Phase 1 for deviations
      (`READKBI`'s dead optional-args shortcut, `DAYSINPERIOD`'s Java no-op, dead `@STAT` skipped)
- [x] Port `KBIFormula.parse()` → build `FormulaToken` AST once (no lazy/mutable caching)
- [x] Port `KBIFormula.calculateFast()` → two-stack shunting-yard evaluator over the AST
- [x] Resolve the `computing` recursion-guard field into an explicit parameter/stack —
      `formula::FormulaGuard`, defined and tested but not yet wired in (real re-entrancy point is
      `Kbi::compute_value`, which doesn't exist until Phase 2 — see `PARITY_AUDIT.md`)
- [x] Unit tests: hand-constructed formula strings + stubbed KBI values — 26 tests, all green

### Phase 2 — KBI domain + statistical analyzers — ✅ complete
- [x] Port `KBIType`, `KBIMode`, `StatOpType`, `KBIStatType` enums (`ForecastMode` stays deferred to
      Phase 4 per `DATA_MODEL.md` §2 — `RegressionAnalyzer`'s one dependency on it is a single
      `is_actual_mode()` port method instead)
- [x] Define the `Kbi` enum (`Input`/`Calculated`/`Statistical`/`PercentOfBase`) replacing the
      `KBI`/`InputKBI`/`CalculatedKBI`/`StatisticalKBI`/`PercentOfBaseKBI` class hierarchy —
      `kbi/domain.rs`; `compute_value` itself deferred to Phase 4 (every Java override just
      delegates to `KBIReaderWriter`, i.e. it's I/O boundary, not domain logic)
- [x] Port `RStats` (fixed `MAXSAM=13`/`MAXCOL=15` matrix build/invert) — including one confirmed
      Java bug in `gaussJordan`'s row-swap, ported verbatim; see `PARITY_AUDIT.md`
- [x] Port `RegressionAnalyzer` — far more DB-coupled than this plan assumed (see
      `PARITY_AUDIT.md`); full control flow ported against a new `RegressionLookupPort` trait
- [x] Port `TAESResult`, `TAESCalculator` (rewrote mutating-lambda alpha/beta grid search as plain
      loops), `TAESAnalyzer` (pure `adjustResultForFutureDates` + `TaesLookupPort` for the DB reads)
- [x] Port `PastAverageAnalyzer` against `PastAverageLookupPort` (`PastAverageFunction`/
      `SameDayLastMonthFunction` were already parsed into the Phase 1 formula AST; their DB-backed
      bodies wait on Phase 3's `FormulaContext` implementation, not this phase)
- [x] Port `PercentOfBaseKBIAnalyzer` — the plan's `FNYPeriod` reference was wrong (confirmed by
      reading the file; that class never touches `FNYPeriod`), so no such stub was needed
- [x] Port `StatisticalKBIAnalyzer` dispatch
- [x] Unit tests: synthetic time series with hand-computed expected regression/TAES outputs — 74
      tests total (up from Phase 1's 26), including an end-to-end `RegressionAnalyzer::calculate`
      test against a fake port

### Phase 3 — Reader/Writer I/O boundary (ports) — ✅ complete
- [x] Enumerate every DB call site in `LaborKBIReaderWriter.java` and `KBI.load()`
      (`setRoomsFlags`/`setRevenueFlags`/`loadDaysOpen`)
- [x] Write `io/ports.rs`: one narrow trait per DB access surface — `KbiLoaderPort`,
      `KbiStatReaderPort`, `KbiStatWriterPort`, `FinancialYearPeriodPort`, each method
      doc-commented with its Java source method; `RevenueCenterConfigPort`/`MarketSegmentConfigPort`
      collapsed into `KbiLoaderPort` and `CalendarPlanPort` deferred to Phase 4 — see
      `PARITY_AUDIT.md`
- [x] Port `KBIReaderWriter`'s mode-dependent Forecast/Adjusted/Actual value read/write selection
      logic on top of the port traits — `io/reader_writer.rs::KbiReaderWriter`; also ported
      `KBI.load()`'s flag-loading sequence (`io/loader.rs::load_kbi_flags`) and
      `checkPeriodForDate`/`computePercentOfBaseKBI`'s `FNYPeriod` lookup
      (`io/loader.rs::check_period_for_date`/`find_period_for_date`); `ForecastMode` ported ahead
      of Phase 4 since this logic switches on it directly (see `PARITY_AUDIT.md`)
- [x] Unit tests: 25 new tests (74 → 99 total) against fake port implementations, all green

### Phase 4 — Orchestrator — ✅ complete
- [x] Define `ForecasterError` (replacing `KBIComputeException`/`InvalidDateException`/
      `DataAccessException`)
- [x] Port `getKbiSetIdForStandardSetId` + KBI-list loading — `engine::ports::KbiSetPort` +
      `engine::kbi_list::KbiList`; see `PARITY_AUDIT.md` for why `load_kbi_list` returns built `Kbi`
      objects rather than raw rows
- [x] Port `processKBIs` (date × KBI loop, `ForecastMode` × KBI-type/flag dispatch) —
      `engine::orchestrator::{process_kbis, process_kbi}`
- [x] Port `checkMarketSegments` (ROOMS mode reconciliation rules) —
      `engine::orchestrator::check_market_segments`
- [x] Port `checkRevenueCenters` (REVENUE mode calendar-plan/open-day zeroing, 3-year date rebase) —
      `engine::orchestrator::check_revenue_centers`
- [x] Assemble `run_forecast(params, ports) -> Result<ForecastOutcome, ForecasterError>` as the
      top-level entry point (no `Thread`/`Progress`/`IThreadListener` machinery)
- [x] Implement `Kbi::compute_value` (every earlier phase's deferred piece) —
      `engine::compute::compute_kbi_value`, dispatching to Phase 1's formula evaluator and Phase 2's
      analyzers; amended Phase 3's `read_kbi_value`/`compute` closure from `f64` to `Option<f64>` in
      the process (see `PARITY_AUDIT.md` — `KBI.computeValue()` is a genuinely nullable `Double` in
      Java)

### Ongoing
- [x] Keep `forecaster/src/PARITY_AUDIT.md` updated every wave (divergences, findings, "where the
      work stands"), not retroactively
- [x] Record a `forecaster-crate-targets-engine-not-trap` memory once confirmed in practice (this
      port did *not* hit the module-name trap `scheduler`/`workrules` did)
- [x] `cargo build --workspace` stays green after every wave

## Status: all four phases complete (107 tests green, `cargo build --workspace` clean). Every port
trait across all four phases is still a trait-only seam with no DB-backed implementation — this
crate cannot run against real data yet, matching `scheduler`'s own "ports are stubs" precedent per
the Verification section above. The only remaining plan item is the memory note above.
