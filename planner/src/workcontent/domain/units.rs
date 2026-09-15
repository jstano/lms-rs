/// The unit a standard's value is expressed in, which decides how the value and
/// the business driver volume combine into work minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Units {
    Hours,
    Minutes,
    HoursPerUnit,
    MinutesPerUnit,
    UnitsPerHour,
    UnitsPerMinute,
    UnitsPerShift,
    /// Only meaningful to dynamic spread standards, which compute their own
    /// work rather than going through the per-unit conversion. Java marks it
    /// deprecated — "to be removed along with dynamic spread standards" — and
    /// throws if it reaches `CalculateWorkMinutesPerUnit`, so the conversion
    /// here reports it as an error rather than inventing a formula.
    UnitsPerPerson,
}
