/// The kind of work a standard describes.
///
/// The non-flowed generators plan everything except [`WorkType::Staff`], which
/// is handled by the staffing generators instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkType {
    Daily,
    Weekly,
    Variable,
    Staff,
    RecurringTask,
    Task,
    ShareWith,
}
