/// How the plan is being generated, which determines which plan types the
/// generated work is written against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerMode {
    Projected,
    Standard,
    ProjectedForecastOnly,
    ReProject,
}
