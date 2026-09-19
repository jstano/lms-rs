//! Regular hours distribution. `RuleType::RegularHoursDistribution` — 3
//! rules.
//!
//! Ground truth:
//! `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/regularhoursdistribution/`.
//!
//! Puts a shift's own net hours onto its regular distribution — one
//! distribution per shift, or two if the shift spans a configured boundary.
//! Distinct from [`hoursdistribution`](super::hoursdistribution), which moves
//! *already-distributed* hours between regular, overtime and double-time
//! buckets over a whole work week; this family runs first, per shift, and puts
//! the hours there in the first place.
//!
//! # The interface is shift-scoped, not card-scoped
//!
//! `RegularHoursDistributionRule.execute(EmployeeShift, RuleItem)` — no
//! `TimeCard`, no work week. Each rule reads and writes exactly the one shift
//! it is given. The `TimeCard`-level orchestration —
//! deciding which shifts are open for editing, clearing and re-running closed
//! ones, falling back to a default rule — lives in
//! [`RegularHoursDistributionRunner`](crate::rules::runner::regular_hours_distribution::RegularHoursDistributionRunner),
//! not here.
//!
//! # A clean 1:1 match
//!
//! Three `*RuleImpl` classes, three catalogue entries — unlike
//! `hoursdistribution`'s 19-vs-20 mismatch, there is no deferred oddball here.
//!
//! [`reg_hours_on_shift_date`], [`reg_hours_by_day`] and
//! [`reg_hours_by_work_week`].

pub mod config;
pub mod reg_hours_by_day;
pub mod reg_hours_by_work_week;
pub mod reg_hours_on_shift_date;

use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;

/// One shift's regular-hours rule. `RegularHoursDistributionRule`.
pub trait RegularHoursDistributionRule {
    /// Distribute this shift's net hours onto its regular bucket.
    fn execute(&self, shift: &mut EmployeeShift, rule_item: &RuleItem);
}
