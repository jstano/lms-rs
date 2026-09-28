# `forecaster` Data Model — Phase 0

> Ground truth is `taps/forecaster/src/main/java/com/unifocus/watson/server/labor/forecaster/engine/`
> (68 files), read directly — never a local copy. See `PLAN_FORECASTER.md` (repo root) for the full
> phased plan, and `lms-rs-java-port-conventions` / `scheduler-uses-plain-i32-ids-not-id-type`
> memories for prior-art id-strategy decisions this document follows or diverges from.

## 1. Id strategy

Confirmed by reading `KBI.java` directly: every id here is a legacy Hibernate/DB `int` primary key
(`KBI.id`, `KBI.propertyID`, `KBI.unitID`, `KBI.kbiConfigId`), matching `scheduler`'s precedent, not
`planner`'s `id_type!` uuid macro.

**Decision: thin non-uuid newtypes, not bare `i32`.** This diverges from `scheduler`
([[scheduler-uses-plain-i32-ids-not-id-type]]), which settled on bare `i32`. The reason: `scheduler`
mostly threads a single id kind (job/employee id) through any one function. `forecaster` routinely
holds several different id kinds side by side in the same call — `KBICode.getKBIID(int propertyID,
String kbiCode)` takes a `PropertyId` and returns a `KbiId`; `KBI` itself carries `id`, `propertyID`,
`unitID`, and `kbiConfigId` as four different sibling `int` fields. A bare-`i32` port makes exactly
the kind of transposition bug that class of code invites, and the formula engine (Phase 1) will add
a fifth kind (`StandardSetId`) resolved from a completely different lookup path
(`getKbiSetIdForStandardSetId`). Newtypes cost one `#[derive]` line each and buy real safety here.

```rust
pub struct KbiId(pub i32);
pub struct PropertyId(pub i32);
pub struct UnitId(pub i32);
pub struct KbiConfigId(pub i32);
pub struct StandardSetId(pub i32);
```

All `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`, each a single-field tuple struct wrapping
`i32`, no `uuid`, no `id_type!` macro (that macro is uuid-only — see `planner::common::id_type`).

## 2. Enum translations

Straight 1:1 mappings, all pure data with a code/lookup method — translate as Rust enums with a
`from_code`/`code` pair, dropping the Java `ICodeNamePair` ceremony:

| Java | Rust | Notes |
|---|---|---|
| `KBIType` (`INPUT`/`CALCULATED`/`STATISTICAL`/`PERCENT_OF_BASE`, codes `I`/`C`/`S`/`P`) | `KbiType` | drives `KBI.createKBI()`'s dispatch → becomes the `Kbi` enum's variant selector (§4 of the plan) |
| `KBIMode` (`BUDGET`/`LABOR`, codes `B`/`L`) | `KbiMode` | selects which SQL column (`BudgetKBITypeCode` vs `LaborKBITypeCode`) a `KBI` loads from — stays relevant at the I/O boundary (Phase 3), not just a display enum |
| `ForecastMode` | `ForecastMode` | `ROOMS`/`REVENUE`/`UPDATE_SYS`/`UPDATE_FST`/`UPDATE_ACT`/`ACTUAL` — orchestrator dispatch (Phase 4); confirm exact variant set when reading `ForecastMode.java` in Phase 4 |
| `KBIStatType` | `KbiStatType` | value-kind selector (`computeValue(date, statType)`) — confirm variants when reading `KBIStatType.java` in Phase 2 |
| `StatOpType` | `StatOpType` | regression/past-average/TAES dispatch key (Phase 2) |
| `PropertyStandardSetType` | `PropertyStandardSetType` | confirm variants in Phase 3 |

`KBIType::getKBIType(code)` throws `IllegalArgumentException` on an unknown code; the Rust
`from_code` returns `Option<KbiType>` (or a `ForecasterError` variant at call sites that need to
propagate a failure) rather than panicking — same spirit as `planner`/`scheduler`'s "no panics in
the pure core" rule.

## 3. `KBICode` — resolution, not a domain entity

`KBICode` (formula-engine helper, not a `KBI` subtype) parses `#code[offset]` syntax and resolves a
`kbiCode: String` to a `KbiId` via `KBIList.getKBI(code)` first, falling back to a raw SQL lookup
(`select ID from KBI where PropertyID = ? and Code = ?`) only when the in-memory list misses it.
Port as a plain struct: `{ kbi_code: String, kbi_id: KbiId, day_offset: i32 }`, with the id
resolution modeled as a function taking a `KbiListLookup`-shaped capability (either the already-
loaded `KbiList` in Phase 1/2, or a port trait method in Phase 3) rather than embedding SQL — this
is the same "narrow port trait, typed parameters" treatment the plan calls out for the I/O boundary
generally.

## 4. `ForecasterError`

Following `planner`'s `GenerationError` precedent
(`planner/src/workcontent/generators/error.rs`): one crate-wide enum, `Display` + `std::error::Error`
impls, no `thiserror` dependency added. Replaces `KBIComputeException` (carries a KBI code + message
— e.g. `KBICode`'s "unable to find the KBI in the database" and "unable to locate the day offset
bracket" sites), `InvalidDateException`, and `DataAccessException`.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecasterError {
    /// `KBIComputeException` — carries the KBI code of the formula being evaluated when the
    /// failure occurred, plus a message (Java attaches `formula.kbi.getCode()` at every throw site).
    KbiCompute { kbi_code: String, message: String },

    /// `InvalidDateException`.
    InvalidDate { message: String },

    /// `DataAccessException` — surfaced through the I/O port traits (Phase 3); kept as a variant
    /// here rather than a raw `Box<dyn Error>` so the pure phases (1-2) stay dependency-free.
    DataAccess { message: String },
}
```

Exact shape (especially whether `DataAccess` needs a source-error field) will firm up once Phase 3
enumerates real port-trait error returns — noted here as the starting point, not a final signature.

## 5. Module layout

```
forecaster/src/
  lib.rs           re-exports; crate-level docs
  DATA_MODEL.md     (this file)
  PARITY_AUDIT.md
  formula/          Phase 1 — FormulaReader, FormulaToken AST, Operation, Function dispatch,
                     KBICode, KBIFormula
  kbi/              Phase 2 — KbiType/KbiMode/StatOpType/KbiStatType/ForecastMode enums, the
                     Kbi enum (Input/Calculated/Statistical/PercentOfBase)
  analyzers/         Phase 2 — RStats, RegressionAnalyzer, TAESCalculator/TAESAnalyzer/TAESResult,
                     PastAverageAnalyzer/PastAverageFunction, SameDayLastMonthFunction,
                     PercentOfBaseKBIAnalyzer, StatisticalKBIAnalyzer
  io/               Phase 3 — ports.rs (KbiLoaderPort, KbiStatReaderPort, KbiStatWriterPort,
                     FinancialYearPeriodPort, RevenueCenterConfigPort, MarketSegmentConfigPort,
                     CalendarPlanPort)
  engine/           Phase 4 — ForecasterError, run_forecast orchestrator, checkMarketSegments/
                     checkRevenueCenters
```

`KbiId`/`PropertyId`/`UnitId`/`KbiConfigId`/`StandardSetId` (§1) live in `lib.rs` or a small
top-level `ids.rs` — shared across every module above, not owned by any one phase.

## 6. Deviation: no Java test suite to transcribe

`src/test/java` and `src/test/groovy` under `taps/forecaster` are configured (Groovy/Spock,
`framework-test`) but contain **zero files** — confirmed by directory listing. Every other crate
ported so far (`planner`, `scheduler`, `workrules`) followed a `java_parity_tests` convention:
transcribe the existing Java/Groovy test suite 1:1 as the Rust suite's starting point. That
convention does not apply here; there is nothing to transcribe.

Instead, tests must be hand-derived directly from reading the algorithm:
- **Phase 1** (formula parser/evaluator): construct formula strings by hand (`#code[0] + @AVG(...)`
  etc.), stub KBI values, and hand-compute the expected result.
- **Phase 2** (regression/TAES): synthetic time series with manually-computed expected regression
  coefficients / TAES-smoothed values (the highest-value place to invest test effort, per the plan,
  since these are the least mechanical translations — `RStats`' fixed-size matrix bounds and
  `TAESCalculator`'s alpha/beta grid search are both easy to get subtly wrong without a reference
  suite to catch it).

This is a real gap relative to prior crates, not a shortcut — call it out again in PR/review context
whenever `forecaster` test coverage is discussed.
