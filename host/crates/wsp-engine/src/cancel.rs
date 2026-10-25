// SPDX-License-Identifier: AGPL-3.0-only
//! The caller's cancel: node's `AbortSignal` with its `reason`. No TS file of its own. Spec: spec/README.md, "A cancel
//! and its words".
use std::sync::{Arc, OnceLock};

use tokio_util::sync::CancellationToken;

/// What `AbortSignal` answers as its reason when `abort()` was called with none.
pub const DEFAULT_REASON: &str = "This operation was aborted";

/// A clonable handle: every clone cancels and observes the same token. The reason is the first cancel's, set before the
/// token fires, so whoever the cancel wakes reads it.
#[derive(Clone, Debug, Default)]
pub struct Cancel {
    token: CancellationToken,
    reason: Arc<OnceLock<Option<String>>>,
}

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancels with `reason`, or with none. Only the first call counts: a later call keeps the first one's reason, and
    /// a first call with none leaves the default for good.
    pub fn cancel(&self, reason: Option<&str>) {
        let _ = self.reason.set(reason.map(str::to_owned));
        self.token.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Resolves once any clone has cancelled; at once where one already has.
    pub async fn cancelled(&self) {
        self.token.cancelled().await
    }

    /// The first cancel's reason, `DEFAULT_REASON` where it passed none, and `DEFAULT_REASON` before any cancel.
    pub fn reason(&self) -> String {
        match self.reason.get() {
            Some(Some(r)) => r.clone(),
            _ => DEFAULT_REASON.to_owned(),
        }
    }

    /// The reason as given: `None` where the cancel passed none or has not happened.
    pub fn given_reason(&self) -> Option<String> {
        self.reason.get().cloned().flatten()
    }

    /// The token under the handle, for a caller that races it against another future.
    pub fn token(&self) -> &CancellationToken {
        &self.token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_cancel_is_not_cancelled_and_reads_the_default_reason() {
        let c = Cancel::new();
        assert!(!c.is_cancelled());
        assert_eq!(c.reason(), "This operation was aborted");
        assert_eq!(c.given_reason(), None);
    }

    #[test]
    fn a_cancel_with_no_reason_reads_the_default() {
        let c = Cancel::new();
        c.cancel(None);
        assert!(c.is_cancelled());
        assert_eq!(c.reason(), "This operation was aborted");
        assert_eq!(c.given_reason(), None);
    }

    #[test]
    fn a_cancel_with_a_reason_reads_it() {
        let c = Cancel::new();
        c.cancel(Some("the user pressed stop"));
        assert!(c.is_cancelled());
        assert_eq!(c.reason(), "the user pressed stop");
        assert_eq!(c.given_reason().as_deref(), Some("the user pressed stop"));
    }

    #[test]
    fn an_empty_reason_is_a_reason() {
        let c = Cancel::new();
        c.cancel(Some(""));
        assert_eq!(c.reason(), "");
        assert_eq!(c.given_reason().as_deref(), Some(""));
    }

    #[test]
    fn the_first_cancel_sets_the_reason_and_a_later_one_never_changes_it() {
        let c = Cancel::new();
        c.cancel(Some("first"));
        c.cancel(Some("second"));
        c.cancel(None);
        assert_eq!(c.reason(), "first");
    }

    #[test]
    fn a_first_cancel_with_none_keeps_the_default_against_a_later_reason() {
        let c = Cancel::new();
        c.cancel(None);
        c.cancel(Some("late"));
        assert_eq!(c.reason(), "This operation was aborted");
        assert_eq!(c.given_reason(), None);
    }

    #[test]
    fn a_clone_shares_the_cancel_and_the_reason_both_ways() {
        let a = Cancel::new();
        let b = a.clone();
        b.cancel(Some("from the clone"));
        assert!(a.is_cancelled());
        assert_eq!(a.reason(), "from the clone");
        a.cancel(Some("ignored"));
        assert_eq!(b.reason(), "from the clone");
    }

    #[test]
    fn separate_cancels_do_not_share_state() {
        let a = Cancel::new();
        let b = Cancel::new();
        a.cancel(Some("a"));
        assert!(!b.is_cancelled());
        assert_eq!(b.reason(), "This operation was aborted");
    }

    #[tokio::test]
    async fn cancelled_resolves_at_once_when_already_cancelled() {
        let c = Cancel::new();
        c.cancel(None);
        tokio::time::timeout(std::time::Duration::from_secs(1), c.cancelled()).await.expect("cancelled never resolved");
    }

    #[tokio::test]
    async fn a_waiter_woken_by_the_cancel_reads_its_reason() {
        let c = Cancel::new();
        let waiter = {
            let c = c.clone();
            tokio::spawn(async move {
                c.cancelled().await;
                c.reason()
            })
        };
        tokio::task::yield_now().await;
        c.cancel(Some("stop waiting"));
        assert_eq!(waiter.await.unwrap(), "stop waiting");
    }

    #[tokio::test]
    async fn cancelled_stays_pending_until_a_cancel() {
        let c = Cancel::new();
        let waited = tokio::time::timeout(std::time::Duration::from_millis(20), c.cancelled()).await;
        assert!(waited.is_err());
    }

    #[test]
    fn the_token_under_the_handle_fires_with_it() {
        let c = Cancel::new();
        let token = c.token().clone();
        assert!(!token.is_cancelled());
        c.cancel(Some("x"));
        assert!(token.is_cancelled());
    }
}
