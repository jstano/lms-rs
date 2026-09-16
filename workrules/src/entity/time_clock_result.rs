//! Port of `com.unifocus.watson.timeclock.common.{TimeClockServerResultDTO,
//! UFTCResultStatus}`.
//!
//! What the server tells a time clock about a punch it offered. The
//! punch-validation family produces these and nothing else — it is the one
//! family that *returns* a value rather than mutating an entity.
//!
//! # Messages are resource keys
//!
//! Java composes each message with `ResourceMgr.lookup("res_someKey")`, falling
//! back to it when the rule has no message configured. The engine does not
//! display anything and the i18n bundles are not ported, so a default message
//! is represented **by its resource key**. A message the rule set configures is
//! carried literally, exactly as Java does.
//!
//! This keeps the Groovy assertions checkable: they read
//! `messages.contains(ResourceMgr.lookup("res_jobLockoutMessage"))`, which
//! becomes `messages.contains("res_jobLockoutMessage")` here — the same
//! question about the same branch, minus the bundle.

/// Whether the clock should accept the punch. `UFTCResultStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UftcResultStatus {
    Success,
    Failure,
}

/// The server's answer to a punch. `TimeClockServerResultDTO`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeClockServerResult {
    status: UftcResultStatus,
    overridable: bool,
    manager_overridable: bool,
    reject_resource_key: Option<String>,
    messages: Vec<String>,
}

impl TimeClockServerResult {
    /// An accepted punch. The Java no-arg constructor: success, no result, not
    /// overridable, no messages.
    pub fn success() -> Self {
        Self {
            status: UftcResultStatus::Success,
            overridable: false,
            manager_overridable: false,
            reject_resource_key: None,
            messages: Vec::new(),
        }
    }

    /// A rejected punch, carrying one message and the key naming the reason.
    ///
    /// The four-and-five-argument `TimeClockServerResultDTO` constructors. The
    /// `result` argument is `null` at every call site in this family, so it
    /// does not come across.
    pub fn failure(
        overridable: bool,
        message: impl Into<String>,
        reject_resource_key: impl Into<String>,
    ) -> Self {
        Self {
            status: UftcResultStatus::Failure,
            overridable,
            manager_overridable: false,
            reject_resource_key: Some(reject_resource_key.into()),
            messages: vec![message.into()],
        }
    }

    /// Mark the rejection as one a manager may override. `setManagerOverridable`.
    ///
    /// Distinct from [`overridable`](Self::overridable), which the employee at
    /// the clock can act on — several rules set one without the other.
    #[must_use]
    pub fn manager_overridable(mut self) -> Self {
        self.manager_overridable = true;
        self
    }

    /// Add a second message. `getMessages().add(...)`.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.messages.push(message.into());
        self
    }

    /// `getStatus()`.
    pub fn status(&self) -> UftcResultStatus {
        self.status
    }

    /// Was the punch accepted?
    pub fn is_success(&self) -> bool {
        self.status == UftcResultStatus::Success
    }

    /// May the employee override the rejection at the clock? `isOverridable()`.
    pub fn overridable(&self) -> bool {
        self.overridable
    }

    /// May a manager override it? `isManagerOverridable()`.
    pub fn is_manager_overridable(&self) -> bool {
        self.manager_overridable
    }

    /// The key naming why the punch was rejected. `getRejectResourceKey()`.
    pub fn reject_resource_key(&self) -> Option<&str> {
        self.reject_resource_key.as_deref()
    }

    /// The messages to show at the clock. `getMessages()`.
    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    /// Does the result carry this message?
    pub fn has_message(&self, message: &str) -> bool {
        self.messages.iter().any(|held| held == message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_successful_result_carries_nothing_else() {
        let result = TimeClockServerResult::success();

        assert!(result.is_success());
        assert_eq!(result.status(), UftcResultStatus::Success);
        assert!(!result.overridable());
        assert!(!result.is_manager_overridable());
        assert_eq!(result.reject_resource_key(), None);
        assert!(result.messages().is_empty());
    }

    #[test]
    fn a_failure_carries_its_reason_and_message() {
        let result =
            TimeClockServerResult::failure(false, "res_jobLockoutMessage", "res_invalidJobCode");

        assert!(!result.is_success());
        assert_eq!(result.reject_resource_key(), Some("res_invalidJobCode"));
        assert!(result.has_message("res_jobLockoutMessage"));
        assert!(!result.overridable());
    }

    #[test]
    fn overridable_and_manager_overridable_are_independent() {
        // Several rules set one and not the other.
        let employee_only = TimeClockServerResult::failure(true, "m", "k");
        assert!(employee_only.overridable());
        assert!(!employee_only.is_manager_overridable());

        let manager_only = TimeClockServerResult::failure(false, "m", "k").manager_overridable();
        assert!(!manager_only.overridable());
        assert!(manager_only.is_manager_overridable());
    }

    #[test]
    fn a_result_can_carry_a_second_message() {
        let result = TimeClockServerResult::failure(true, "first", "k").with_message("second");

        assert_eq!(result.messages(), ["first", "second"]);
        assert!(result.has_message("second"));
    }
}
