//! Port of `com.unifocus.watson.common.enums.ScheduleMode`.
//!
//! Whether a property builds its schedules a week at a time or a month at a
//! time. Read by [`contract`](crate::common::contract) together with the pay
//! period type, and by nothing else in the rules tree.

use crate::coded_enum;

coded_enum! {
    /// The period a property schedules over. `ScheduleMode`.
    ScheduleMode {
        Monthly => "M",
        Weekly => "W",
    }
}
