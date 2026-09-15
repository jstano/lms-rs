#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanType {
    Forecast,
    Generated,
    Original,
    Standard
}
