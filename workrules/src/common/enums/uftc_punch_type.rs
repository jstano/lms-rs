//! Port of `com.unifocus.watson.timeclock.common.UFTCPunchType`.
//!
//! The time clock's own copy of [`PunchType`](super::punch_type::PunchType) —
//! same fifteen constants, same codes, same `isTimePunchType` predicate. Java
//! keeps two enums because one ships to the clock hardware and the other lives
//! on the server, with a standing comment on `PunchType` warning that the two
//! must stay compatible.
//!
//! Kept as a separate type here for the same reason: the punch-validation rules
//! are the server side of the clock protocol and take this one, and collapsing
//! them would quietly couple the wire format to the domain model.

use crate::coded_enum;

coded_enum! {
    /// What a punch from the time clock records. `UFTCPunchType`.
    UFTCPunchType {
        In => "IN",
        Out => "OUT",
        Break => "BREAK",
        Back => "BACK",
        Tips => "TIPS",
        Gross => "GRS",
        Pieces => "PCS",
        ChargeTips => "GTPS",
        Memo1 => "MEMO1",
        Memo2 => "MEMO2",
        Memo3 => "MEMO3",
        Memo4 => "MEMO4",
        OnSite => "ON_SITE",
        OffSite => "OFF_SITE",
        Meal => "MEAL",
    }
}

impl UFTCPunchType {
    /// Does this punch record a time? `isTimePunchType()`.
    pub fn is_time_punch_type(&self) -> bool {
        matches!(self, Self::In | Self::Out | Self::Break | Self::Back)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::punch_type::PunchType;

    #[test]
    fn every_code_round_trips() {
        for value in UFTCPunchType::VALUES {
            assert_eq!(UFTCPunchType::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn it_carries_the_same_constants_as_the_server_side_punch_type() {
        // The compatibility Java's comment insists on, asserted rather than
        // assumed — if either enum gains a constant, this fails.
        let mut clock: Vec<_> = UFTCPunchType::VALUES.iter().map(|p| p.code()).collect();
        let mut server: Vec<_> = PunchType::VALUES.iter().map(|p| p.code()).collect();
        clock.sort_unstable();
        server.sort_unstable();
        assert_eq!(clock, server);
    }

    #[test]
    fn but_it_declares_them_in_a_different_order() {
        // The clock reads IN, OUT, BREAK, BACK; the server IN, BREAK, BACK,
        // OUT. That is not cosmetic: `PunchTimeComparator` tiebreaks on the
        // server enum's ordinal, so the two orders are not interchangeable and
        // neither enum's position may be used to index the other.
        assert_eq!(UFTCPunchType::VALUES[1], UFTCPunchType::Out);
        assert_eq!(PunchType::VALUES[1], PunchType::Break);

        assert_eq!(UFTCPunchType::Out.ordinal(), 1);
        assert_eq!(PunchType::Out.ordinal(), 3);
    }

    #[test]
    fn the_four_time_punches() {
        assert!(UFTCPunchType::In.is_time_punch_type());
        assert!(UFTCPunchType::Out.is_time_punch_type());
        assert!(UFTCPunchType::Break.is_time_punch_type());
        assert!(UFTCPunchType::Back.is_time_punch_type());
        assert!(!UFTCPunchType::Tips.is_time_punch_type());
    }
}
