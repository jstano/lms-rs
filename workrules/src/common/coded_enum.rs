//! The shape shared by every enum in `com.unifocus.watson.common.enums`.
//!
//! Each of those is a "coded enum": a fixed set of constants, each carrying a
//! short `code` that is what actually lives in the database and in rule
//! parameters, plus a `fromCode` lookup that throws on anything unrecognised.
//! Most also carry a `resourceKey` for an i18n display string, which does not
//! come across — the engine never displays anything.
//!
//! Java throws `IllegalArgumentException` from `fromCode`. Here `from_code`
//! returns an `Option`, because these codes arrive from stored data rather than
//! from the program: an unknown code is a data problem for the caller to
//! handle, not a bug to abort on.

/// Define a coded enum with its `code`/`from_code` round trip.
///
/// ```ignore
/// coded_enum! {
///     /// Doc comment for the type.
///     PunchSource {
///         /// Doc comment for the variant.
///         Auto => "A",
///         Clock => "C",
///         Manual => "M",
///     }
/// }
/// ```
#[macro_export]
macro_rules! coded_enum {
    (
        $(#[$type_meta:meta])*
        $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident => $code:literal
            ),+ $(,)?
        }
    ) => {
        $(#[$type_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )+
        }

        impl $name {
            /// Every variant, in the order Java declares them.
            ///
            /// Java gets this from `values()`; several rules iterate it, and
            /// the order is load-bearing wherever they take the first match.
            pub const VALUES: &'static [Self] = &[$(Self::$variant,)+];

            /// This variant's position in the declaration, from zero.
            ///
            /// Java's `Enum.ordinal()`. Usually an implementation detail, but
            /// `PunchTimeComparator` sorts on it, so for some of these enums the
            /// declaration order is behaviour.
            pub fn ordinal(&self) -> usize {
                Self::VALUES
                    .iter()
                    .position(|value| value == self)
                    .expect("every variant is in VALUES")
            }

            /// The persisted code. `getCode()`.
            pub fn code(&self) -> &'static str {
                match self {
                    $(Self::$variant => $code,)+
                }
            }

            /// Look up by persisted code. `fromCode(String)`.
            ///
            /// `None` where Java throws `IllegalArgumentException`.
            pub fn from_code(code: &str) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    crate::coded_enum! {
        /// A stand-in used to test the macro itself.
        Sample {
            First => "F",
            Second => "S",
        }
    }

    #[test]
    fn codes_round_trip() {
        for value in Sample::VALUES {
            assert_eq!(Sample::from_code(value.code()), Some(*value));
        }
    }

    #[test]
    fn an_unknown_code_is_none_rather_than_a_panic() {
        assert_eq!(Sample::from_code("Z"), None);
        assert_eq!(Sample::from_code(""), None);
    }

    #[test]
    fn ordinals_follow_the_declaration() {
        assert_eq!(Sample::First.ordinal(), 0);
        assert_eq!(Sample::Second.ordinal(), 1);
    }

    #[test]
    fn values_keeps_declaration_order() {
        assert_eq!(Sample::VALUES, &[Sample::First, Sample::Second]);
    }
}
