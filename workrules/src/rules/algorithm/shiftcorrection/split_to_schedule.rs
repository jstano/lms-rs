//! Port of `SplitToScheduleCorrectionRuleImpl`.
//!
//! Ground truth: `taps/src/java/com/unifocus/watson/server/labor/rules/algorithm/shiftcorrection/SplitToScheduleCorrectionRuleImpl.java`.
//!
//! Cuts one long worked shift into pieces along a chain of back-to-back
//! schedules, so each piece carries the job and category of the schedule it
//! overlaps. The only rule in the engine that **creates shifts**: each cut is
//! persisted through [`EmployeeShiftSavePort`] and appended to the time card.
//!
//! # Gate
//!
//! The shift must have no derived errors, both times, exactly two punches,
//! be open for editing, and (unless `includeManagerEdits`) have no `MANUAL`
//! punch.
//!
//! # Which schedules count
//!
//! Schedules that *overlap* the shift (`ShiftUtil.isOverlappingShifts`, so
//! merely touching does not count), sorted by start, then only those that sit
//! **end-to-start against another overlapping schedule**. A lone schedule
//! never splits anything, and neither does a gap between two schedules
//! ("split schedule" in the spec: 12:00 end, 12:15 start).
//!
//! # The cut
//!
//! For each contiguous schedule, in order, if its start lies strictly inside
//! the *current* shift and its job or category differs from the current
//! shift's, the current shift's OUT moves back to the schedule start and a
//! copy of the schedule — made ACTUAL, id 0, planned shift cleared,
//! adjustments cleared — takes the old OUT. The copy becomes the current
//! shift, so a chain of three schedules cuts twice.
//!
//! Java's `buildContiguousScheduleList` compares schedules with `!=` and
//! `List.contains`; neither `EmployeeShift` nor its list overrides `equals`,
//! so both are identity. The port compares positions.

use crate::common::dates::is_between_exclusive;
use crate::common::enums::punch_source::PunchSource;
use crate::common::enums::punch_type::PunchType;
use crate::common::enums::shift_type::ShiftType;
use crate::entity::employee_shift::EmployeeShift;
use crate::entity::rule_item::RuleItem;
use crate::entity::time_card::TimeCard;
use crate::rules::algorithm::shiftcorrection::ShiftCorrectionRule;
use crate::rules::algorithm::shiftcorrection::config::{
    INCLUDE_MANAGER_EDITS_PROP, SplitToScheduleCorrectionRuleConfig,
};
use crate::rules::ports::EmployeeShiftSavePort;
use crate::rules::rule_config::RuleConfig;

/// `SplitToScheduleCorrectionRuleImpl`.
pub struct SplitToScheduleCorrectionRule<S: EmployeeShiftSavePort> {
    employee_shift: S,
}

impl<S: EmployeeShiftSavePort> SplitToScheduleCorrectionRule<S> {
    /// Build the rule over the port that persists the shifts it creates.
    pub fn new(employee_shift: S) -> Self {
        Self { employee_shift }
    }
}

impl<S: EmployeeShiftSavePort> ShiftCorrectionRule for SplitToScheduleCorrectionRule<S> {
    fn execute(
        &self,
        shift: &mut EmployeeShift,
        time_card: &mut dyn TimeCard,
        rule_item: &RuleItem,
    ) {
        let params = rule_item
            .params()
            .fixed(&SplitToScheduleCorrectionRuleConfig.default_values());
        let include_manager_edits = params.bool_at(INCLUDE_MANAGER_EDITS_PROP);

        if !shift_is_splittable(shift, time_card, include_manager_edits) {
            return;
        }

        let contiguous = contiguous_schedules(shift, time_card);

        // `None` is the shift the caller handed in; `Some(i)` is a split
        // already appended to the time card at `i`.
        let mut current_index: Option<usize> = None;

        for schedule in &contiguous {
            let mut split = {
                let current = current_shift(shift, time_card, current_index);
                if !should_split_to_schedule(current, schedule) {
                    continue;
                }
                let Some(current_out) = current.punch_index_of_type(PunchType::Out) else {
                    continue;
                };
                let mut split = schedule.clone();
                split.set_shift_type(ShiftType::Actual);
                split.set_id(0);
                split.set_planned_shift(None);
                split.clear_schedule_lunch_adjustment();
                let Some(split_out) = split.punch_index_of_type(PunchType::Out) else {
                    continue;
                };

                // The split takes over the current shift's OUT time...
                let old_out = current.punch(current_out).adj_time();
                {
                    let punch = split.punch_mut(split_out);
                    punch.set_adj_time(old_out);
                    punch.set_punch_time(old_out);
                }
                split.punch_cursor(split_out).set_rounded_time(old_out);

                // ...and the current shift now ends where the schedule starts.
                let schedule_start = schedule.start_date_time();
                current.punch_mut(current_out).set_adj_time(schedule_start);
                current
                    .punch_cursor(current_out)
                    .set_rounded_time(schedule_start);

                for index in 0..split.punch_count() {
                    split.punch_mut(index).set_id(0);
                }
                split.set_times_from_punches();
                current.set_times_from_punches();
                split
            };

            self.employee_shift.save(&mut split);
            time_card.shifts_mut().push(split);
            current_index = Some(time_card.shifts().len() - 1);
        }
    }
}

fn current_shift<'a>(
    shift: &'a mut EmployeeShift,
    time_card: &'a mut dyn TimeCard,
    index: Option<usize>,
) -> &'a mut EmployeeShift {
    match index {
        None => shift,
        Some(i) => &mut time_card.shifts_mut()[i],
    }
}

fn should_split_to_schedule(current: &EmployeeShift, schedule: &EmployeeShift) -> bool {
    let (Some(start), Some(cur_start), Some(cur_end)) = (
        schedule.start_date_time(),
        current.start_date_time(),
        current.end_date_time(),
    ) else {
        return false;
    };
    is_between_exclusive(start, cur_start, cur_end)
        && (current.job_id() != schedule.job_id()
            || current.shift_category_id() != schedule.shift_category_id())
}

fn shift_is_splittable(
    shift: &EmployeeShift,
    time_card: &dyn TimeCard,
    include_manager_edits: bool,
) -> bool {
    shift.derived_errors().is_empty()
        && shift.has_both_times()
        && shift.punch_count() == 2
        && time_card.is_open_for_editing_for_shift(shift)
        && (include_manager_edits || !has_manual_punches(shift))
}

fn has_manual_punches(shift: &EmployeeShift) -> bool {
    shift
        .punches()
        .iter()
        .any(|p| p.source() == PunchSource::Manual)
}

/// `buildContiguousScheduleList`: overlapping schedules, sorted by start, then
/// those whose start meets another's end — each at most once, in the order the
/// outer loop first reaches them.
fn contiguous_schedules(shift: &EmployeeShift, time_card: &dyn TimeCard) -> Vec<EmployeeShift> {
    let mut overlapping: Vec<&EmployeeShift> = time_card
        .schedules()
        .iter()
        .filter(|schedule| shift.is_overlapping(schedule))
        .collect();
    overlapping.sort_by_key(|s| s.start_time_for_ordering());

    let mut chosen: Vec<usize> = Vec::new();
    for (i, schedule) in overlapping.iter().enumerate() {
        for (j, schedule2) in overlapping.iter().enumerate() {
            let contiguous = i != j && schedule.end_date_time() == schedule2.start_date_time();
            if contiguous && !chosen.contains(&j) {
                chosen.push(j);
            }
        }
    }
    chosen.into_iter().map(|j| overlapping[j].clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::employee_shift_punch::EmployeeShiftPunch;
    use crate::entity::time_card::TimeCardData;
    use crate::rule_params;
    use crate::rules::rule_class::RuleClass;
    use joda_rs::{LocalDate, LocalDateTime};
    use std::cell::Cell;

    const FRONT_DESK: i32 = 1;
    const HOSTESS: i32 = 2;
    const DEF_CATEGORY: i32 = 1;
    const TRAINING: i32 = 2;

    #[derive(Default)]
    struct CountingSave(Cell<u32>);

    impl EmployeeShiftSavePort for &CountingSave {
        fn save(&self, _shift: &mut EmployeeShift) {
            self.0.set(self.0.get() + 1);
        }
    }

    fn time(hour: i32, minute: i32) -> LocalDateTime {
        LocalDateTime::of(2013, 4, 1, hour, minute, 0)
    }

    fn shift(
        shift_type: ShiftType,
        start: LocalDateTime,
        end: LocalDateTime,
        category: i32,
        job: i32,
    ) -> EmployeeShift {
        EmployeeShift::new(
            0,
            100,
            job,
            LocalDate::of(2013, 4, 1),
            shift_type,
            vec![
                EmployeeShiftPunch::new(1, PunchType::In, PunchSource::Manual, start),
                EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Manual, end),
            ],
        )
        .with_times(Some(start), Some(end))
        .with_shift_category_id(Some(category))
    }

    fn actual(s: LocalDateTime, e: LocalDateTime, category: i32, job: i32) -> EmployeeShift {
        shift(ShiftType::Actual, s, e, category, job)
    }

    fn sched(s: LocalDateTime, e: LocalDateTime, category: i32, job: i32) -> EmployeeShift {
        shift(ShiftType::Schedule, s, e, category, job)
    }

    fn sched1() -> EmployeeShift {
        sched(time(8, 0), time(12, 0), DEF_CATEGORY, FRONT_DESK)
    }

    fn sched2() -> EmployeeShift {
        sched(time(12, 0), time(16, 0), DEF_CATEGORY, HOSTESS)
    }

    /// `containsActuals`: same count, and every expected shift has a match on
    /// job, start, end and category.
    fn contains_actuals(card: &TimeCardData, expected: &[EmployeeShift]) -> bool {
        card.shifts().len() == expected.len()
            && expected.iter().all(|e| {
                card.shifts().iter().any(|a| {
                    a.job_id() == e.job_id()
                        && a.start_date_time() == e.start_date_time()
                        && a.end_date_time() == e.end_date_time()
                        && a.shift_category_id() == e.shift_category_id()
                })
            })
    }

    fn run(
        shift: EmployeeShift,
        schedules: Vec<EmployeeShift>,
        include_manager_edits: &str,
    ) -> (TimeCardData, u32) {
        let saver = CountingSave::default();
        let mut card = TimeCardData::new()
            .with_shifts(vec![shift.clone()])
            .with_schedules(schedules);
        let mut shift = shift;
        let item = RuleItem::new(
            1,
            1,
            "split",
            RuleClass::SplitToScheduleScr,
            rule_params! { INCLUDE_MANAGER_EDITS_PROP => include_manager_edits },
        );
        SplitToScheduleCorrectionRule::new(&saver).execute(&mut shift, &mut card, &item);
        // The runner writes the corrected shift back over the original.
        card.shifts_mut()[0] = shift;
        (card, saver.0.get())
    }

    // ---- java_parity_tests: SplitToScheduleCorrectionRuleImplTest.groovy ----

    #[test]
    fn one_schedule_does_not_split_actual() {
        let s = actual(time(11, 0), time(19, 0), DEF_CATEGORY, FRONT_DESK);
        let (card, saves) = run(s.clone(), vec![sched1()], "true");
        assert_eq!(saves, 0);
        assert!(contains_actuals(&card, &[s]));
    }

    #[test]
    fn two_matching_cause_split() {
        let s = actual(time(11, 0), time(19, 0), DEF_CATEGORY, FRONT_DESK);
        let (card, saves) = run(s, vec![sched1(), sched2()], "true");
        assert_eq!(saves, 1);
        assert!(contains_actuals(
            &card,
            &[
                actual(time(11, 0), time(12, 0), DEF_CATEGORY, FRONT_DESK),
                actual(time(12, 0), time(19, 0), DEF_CATEGORY, HOSTESS),
            ]
        ));
    }

    #[test]
    fn two_schedules_before_no_split() {
        let s = actual(time(3, 0), time(11, 0), DEF_CATEGORY, FRONT_DESK);
        let (card, saves) = run(s.clone(), vec![sched1(), sched2()], "true");
        assert_eq!(saves, 0);
        assert!(contains_actuals(&card, &[s]));
    }

    #[test]
    fn two_after_no_split() {
        let s = actual(time(13, 0), time(21, 0), DEF_CATEGORY, FRONT_DESK);
        let (card, saves) = run(s.clone(), vec![sched1(), sched2()], "true");
        assert_eq!(saves, 0);
        assert!(contains_actuals(&card, &[s]));
    }

    #[test]
    fn hit_the_post_end_time() {
        let s = actual(time(4, 0), time(12, 0), DEF_CATEGORY, FRONT_DESK);
        let (card, saves) = run(s.clone(), vec![sched1(), sched2()], "true");
        assert_eq!(saves, 0);
        assert!(contains_actuals(&card, &[s]));
    }

    #[test]
    fn hit_the_post_start_time() {
        let s = actual(time(12, 0), time(20, 0), DEF_CATEGORY, FRONT_DESK);
        let (card, saves) = run(s.clone(), vec![sched1(), sched2()], "true");
        assert_eq!(saves, 0);
        assert!(contains_actuals(&card, &[s]));
    }

    #[test]
    fn really_short_shift() {
        let s = actual(time(11, 57), time(16, 0), DEF_CATEGORY, FRONT_DESK);
        let (card, saves) = run(s, vec![sched1(), sched2()], "true");
        assert_eq!(saves, 1);
        assert!(contains_actuals(
            &card,
            &[
                actual(time(11, 57), time(12, 0), DEF_CATEGORY, FRONT_DESK),
                actual(time(12, 0), time(16, 0), DEF_CATEGORY, HOSTESS),
            ]
        ));
    }

    #[test]
    fn triple_schedule() {
        let s = actual(time(8, 0), time(16, 0), DEF_CATEGORY, FRONT_DESK);
        let schedules = vec![
            sched(time(8, 0), time(10, 0), DEF_CATEGORY, FRONT_DESK),
            sched(time(10, 0), time(12, 0), TRAINING, HOSTESS),
            sched(time(12, 0), time(16, 0), DEF_CATEGORY, HOSTESS),
        ];
        let (card, saves) = run(s, schedules, "true");
        assert_eq!(saves, 2);
        assert!(contains_actuals(
            &card,
            &[
                actual(time(8, 0), time(10, 0), DEF_CATEGORY, FRONT_DESK),
                actual(time(10, 0), time(12, 0), TRAINING, HOSTESS),
                actual(time(12, 0), time(16, 0), DEF_CATEGORY, HOSTESS),
            ]
        ));
    }

    #[test]
    fn split_schedule_with_a_gap_does_not_split() {
        let s = actual(time(8, 0), time(16, 15), DEF_CATEGORY, FRONT_DESK);
        let schedules = vec![
            sched1(),
            sched(time(12, 15), time(16, 15), DEF_CATEGORY, HOSTESS),
        ];
        let (card, saves) = run(s.clone(), schedules, "true");
        assert_eq!(saves, 0);
        assert!(contains_actuals(&card, &[s]));
    }

    #[test]
    fn split_category_only() {
        let s = actual(time(8, 0), time(16, 0), DEF_CATEGORY, FRONT_DESK);
        let schedules = vec![
            sched1(),
            sched(time(12, 0), time(16, 0), TRAINING, FRONT_DESK),
        ];
        let (card, saves) = run(s, schedules, "true");
        assert_eq!(saves, 1);
        assert!(contains_actuals(
            &card,
            &[
                actual(time(8, 0), time(12, 0), DEF_CATEGORY, FRONT_DESK),
                actual(time(12, 0), time(16, 0), TRAINING, FRONT_DESK),
            ]
        ));
    }

    /// The spec's fixtures are all `MANUAL`, so turning the flag off blocks.
    #[test]
    fn include_manual_edits_turned_off() {
        let s = actual(time(8, 0), time(16, 0), DEF_CATEGORY, FRONT_DESK);
        let (card, saves) = run(s.clone(), vec![sched1(), sched2()], "false");
        assert_eq!(saves, 0);
        assert!(contains_actuals(&card, &[s]));
    }

    /// `actualWithError`: the first punch's type is flipped to BACK, leaving no IN.
    #[test]
    fn shift_with_errors() {
        let s = EmployeeShift::new(
            0,
            100,
            FRONT_DESK,
            LocalDate::of(2013, 4, 1),
            ShiftType::Actual,
            vec![
                EmployeeShiftPunch::new(1, PunchType::Back, PunchSource::Manual, time(8, 0)),
                EmployeeShiftPunch::new(2, PunchType::Out, PunchSource::Manual, time(16, 0)),
            ],
        )
        .with_times(Some(time(8, 0)), Some(time(16, 0)))
        .with_shift_category_id(Some(DEF_CATEGORY));
        assert!(
            !s.derived_errors().is_empty(),
            "fixture must carry an error"
        );
        let (card, saves) = run(s.clone(), vec![sched1(), sched2()], "true");
        assert_eq!(saves, 0);
        assert!(contains_actuals(&card, &[s]));
    }

    /// `actualWithBreaks`: four punches, so the "exactly two" gate fails.
    #[test]
    fn shifts_with_multiple_punches() {
        let s = actual(time(8, 0), time(16, 0), DEF_CATEGORY, FRONT_DESK)
            .with_punch(EmployeeShiftPunch::new(
                3,
                PunchType::Break,
                PunchSource::Manual,
                time(11, 0),
            ))
            .with_punch(EmployeeShiftPunch::new(
                4,
                PunchType::Back,
                PunchSource::Manual,
                time(15, 0),
            ));
        let (card, saves) = run(s.clone(), vec![sched1(), sched2()], "true");
        assert_eq!(saves, 0);
        assert!(contains_actuals(&card, &[s]));
    }

    #[test]
    fn same_job_and_category_should_not_split() {
        let s = actual(time(11, 0), time(19, 0), DEF_CATEGORY, FRONT_DESK);
        let schedules = vec![
            sched1(),
            sched(time(12, 0), time(16, 0), DEF_CATEGORY, FRONT_DESK),
        ];
        let (card, saves) = run(s.clone(), schedules, "true");
        assert_eq!(saves, 0);
        assert!(contains_actuals(&card, &[s]));
    }

    // ---- behaviour tests from the Java ----

    #[test]
    fn a_split_is_a_new_actual_with_no_planned_shift_and_fresh_ids() {
        let s = actual(time(11, 0), time(19, 0), DEF_CATEGORY, FRONT_DESK);
        let sched_with_plan = sched2().with_planned_shift(Some(
            crate::entity::planned_shift::PlannedShift::new(time(12, 0), 4.0),
        ));
        let (card, _) = run(s, vec![sched1(), sched_with_plan], "true");
        let split = card
            .shifts()
            .iter()
            .find(|s| s.job_id() == HOSTESS)
            .expect("a split exists");
        assert_eq!(split.shift_type(), ShiftType::Actual);
        assert_eq!(split.id(), 0);
        assert!(split.planned_shift().is_none());
        assert!(split.punches().iter().all(|p| p.id() == 0));
    }

    #[test]
    fn the_cut_moves_the_original_out_punch_not_just_its_end_time() {
        let s = actual(time(11, 0), time(19, 0), DEF_CATEGORY, FRONT_DESK);
        let (card, _) = run(s, vec![sched1(), sched2()], "true");
        let original = &card.shifts()[0];
        let out = original.punch(original.punch_index_of_type(PunchType::Out).unwrap());
        assert_eq!(out.adj_time(), Some(time(12, 0)));
        assert_eq!(out.rounded_time(), Some(time(12, 0)));
    }
}
