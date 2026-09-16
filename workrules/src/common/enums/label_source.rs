//! Port of `com.unifocus.watson.common.enums.LabelSource`.

use crate::coded_enum;

coded_enum! {
    /// What applied a schedule label. `LabelSource`.
    LabelSource {
        Manual => "M",
        Rule => "R",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips() {
        for value in LabelSource::VALUES {
            assert_eq!(LabelSource::from_code(value.code()), Some(*value));
        }
    }
}
