//! Who may put a full-map report on a timer.
//!
//! See `specs/premium-full-map.md`. Today the answer is one boolean on
//! the guild row, and this module exists purely so that stays true of *one*
//! place. The rule is asked at two points that must never drift apart —
//! `/schedule-report` when a schedule is created, and every tick of an existing
//! one — and the second is the one that matters: an approval that can be granted
//! but not revoked isn't a gate.
//!
//! The trait is a seam, not an abstraction with two implementations. If a future
//! entitlement source ever replaces the flag (a sponsor role, a donation tier —
//! neither of which ships, see the spec's ToS findings), it is written here and
//! the command bodies don't move. It is `async` for the same reason: a source
//! that has to ask Discord or a payment provider shouldn't force every call site
//! to change shape on the day it arrives.

use super::db::GuildData;

pub trait FullMapScheduling {
    /// May this guild run a *scheduled* full-map report right now?
    ///
    /// Nothing about guild size enters into this. Rendering the full map on
    /// demand never asks at all — that path is free for everyone.
    ///
    /// Spelled `-> impl Future + Send` rather than as an `async fn`, and the
    /// `Send` is not decoration. A bare `async fn` in a trait produces a future
    /// with no `Send` bound, and both callers need one: poise boxes every
    /// command body as `Pin<Box<dyn Future + Send>>`, and the scheduler does the
    /// same to every tick. Written the short way, this compiles here and fails
    /// at both call sites with an error that points at them and not at this
    /// line.
    fn is_allowed(&self, guild: &GuildData) -> impl std::future::Future<Output = bool> + Send;
}

/// The shipped rule: the owner approved this guild's request, and hasn't taken
/// it back.
pub struct ApprovalFlag;

impl FullMapScheduling for ApprovalFlag {
    async fn is_allowed(&self, guild: &GuildData) -> bool {
        guild.full_map_approved
    }
}

/// The rule in force. Every caller goes through this rather than naming an
/// implementation, so swapping the source is a one-line change.
pub fn scheduling() -> impl FullMapScheduling {
    ApprovalFlag
}
