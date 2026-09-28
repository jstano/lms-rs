//! `Operation.java` + `AddOperation`/`SubtractOperation`/`MultiplyOperation`/`DivideOperation`/
//! `ModulusOperation`. Priorities are ported literally from the Java source even though they look
//! surprising (subtract outranks add; multiply outranks divide/modulus) — `KBIFormula`'s
//! shunting-yard evaluator (`../mod.rs`) depends on these exact numbers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulus,
}

impl Op {
    /// `Operation.isOperation(char)`.
    pub fn is_operation_char(ch: char) -> bool {
        matches!(ch, '+' | '-' | '*' | '/' | '%')
    }

    /// `Operation.newOperation(char)`. Panics on a char `is_operation_char` did not accept, since
    /// every call site checks that first (same precondition the Java code relies on).
    pub fn from_char(operator: char) -> Self {
        match operator {
            '+' => Op::Add,
            '-' => Op::Subtract,
            '*' => Op::Multiply,
            '/' => Op::Divide,
            '%' => Op::Modulus,
            _ => panic!("'{operator}' is not a formula operator"),
        }
    }

    /// `Operation.getPriority()` on each concrete subclass.
    pub fn priority(self) -> i32 {
        match self {
            Op::Add => 1,
            Op::Subtract => 2,
            Op::Divide | Op::Modulus => 3,
            Op::Multiply => 4,
        }
    }

    /// `Operation.calculate(Double, Double)`.
    pub fn calculate(self, value1: f64, value2: f64) -> f64 {
        match self {
            Op::Add => value1 + value2,
            Op::Subtract => value1 - value2,
            Op::Multiply => value1 * value2,
            Op::Divide => {
                if value1 != 0.0 && value2 != 0.0 {
                    value1 / value2
                } else {
                    0.0
                }
            }
            Op::Modulus => {
                if value1 != 0.0 && value2 != 0.0 {
                    value1 % value2
                } else {
                    0.0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priorities_match_the_java_source_exactly() {
        assert_eq!(Op::Add.priority(), 1);
        assert_eq!(Op::Subtract.priority(), 2);
        assert_eq!(Op::Divide.priority(), 3);
        assert_eq!(Op::Modulus.priority(), 3);
        assert_eq!(Op::Multiply.priority(), 4);
    }

    #[test]
    fn divide_and_modulus_by_zero_return_zero_instead_of_dividing() {
        assert_eq!(Op::Divide.calculate(5.0, 0.0), 0.0);
        assert_eq!(Op::Divide.calculate(0.0, 5.0), 0.0);
        assert_eq!(Op::Modulus.calculate(5.0, 0.0), 0.0);
    }
}
