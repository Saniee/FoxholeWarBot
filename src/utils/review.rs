//! The review surface for full-map schedule requests: the embed a reviewer sees,
//! the Approve/Deny buttons, and who is allowed to press them.
//!
//! Both review paths land here. The channel post is the primary surface and
//! `/full-map-requests` is the fallback, but they drive the same
//! `review_full_map_request` call and render the same embed, so they cannot
//! diverge (`specs/active/premium-full-map.md`).

use std::collections::HashSet;
use std::sync::OnceLock;

use poise::serenity_prelude as serenity;

use super::db::{Database, FullMapRequest, RequestStatus, Revoked};

/// Prefix on every button this module owns, so the interaction handler can tell
/// its own components from anything else the bot ever adds.
const CUSTOM_ID_PREFIX: &str = "fullmap-request";

/// The support-server channel where requests are posted for review. Optional:
/// unset means command-only review, which is a degraded surface, never a
/// dropped request.
fn requests_channel() -> Option<serenity::ChannelId> {
    static CHANNEL: OnceLock<Option<serenity::ChannelId>> = OnceLock::new();

    *CHANNEL.get_or_init(|| match dotenv::var("REQUESTS_CHANNEL_ID") {
        Ok(raw) => match raw.trim().parse::<u64>() {
            Ok(id) => Some(serenity::ChannelId::new(id)),
            Err(_) => {
                log::warn!("REQUESTS_CHANNEL_ID is not an integer, falling back to command review");
                None
            }
        },
        Err(_) => None,
    })
}

/// Everyone allowed to decide requests. **The complete list — there is no
/// implicit owner.**
///
/// `REVIEWER_IDS`, in `.env`. One id on its own, or several separated by
/// commas; whitespace around them is ignored:
///
/// ```text
/// REVIEWER_IDS=123456789012345678
/// REVIEWER_IDS=123456789012345678,987654321098765432
/// ```
///
/// The application's owner is **not** added automatically, and that is the
/// point. This bot is open source and self-hosted: "the owner can always
/// approve" reads clearly when you wrote it and misleadingly when you are
/// deploying somebody else's code, because it invites a guess about whose
/// account that is — the Discord application's owner, which on a team
/// application may be nobody who runs the deployment. One env var that lists
/// every reviewer by id cannot be misread. Set it, put your own id in it.
///
/// In `.env` rather than the database on purpose: reviewing means reading other
/// servers' free-text answers and granting recurring load on the host, so the
/// list belongs to whoever runs the host — not to anything editable from inside
/// Discord, where a compromised account could add itself. It also means the bot
/// stores no record of who its reviewers are, which keeps `docs/privacy.md`'s
/// stored-data list honest without a word of change.
///
/// Parsed once. A malformed entry is logged and skipped rather than taken as an
/// empty list — one typo must not quietly lock everyone out.
pub fn reviewers() -> &'static HashSet<u64> {
    static REVIEWERS: OnceLock<HashSet<u64>> = OnceLock::new();

    REVIEWERS.get_or_init(|| {
        let raw = dotenv::var("REVIEWER_IDS").unwrap_or_default();

        let reviewers: HashSet<u64> = raw
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .filter_map(|entry| match entry.parse::<u64>() {
                Ok(id) => Some(id),
                Err(_) => {
                    log::warn!("ignoring '{entry}' in REVIEWER_IDS — not a Discord user id");
                    None
                }
            })
            .collect();

        // Warned about at startup, not on the first refusal, so a misconfigured
        // deployment is noticed before someone has already filed a request that
        // nobody can act on.
        if reviewers.is_empty() {
            log::warn!(
                "REVIEWER_IDS is empty — nobody can approve full-map schedule requests. \
                 Set it to your own Discord user id (see .env.example); the application owner \
                 is not added automatically."
            );
        } else {
            log::info!("{} full-map reviewer(s) configured", reviewers.len());
        }

        reviewers
    })
}

/// May this user decide requests?
///
/// Membership of [`reviewers`], and nothing else. Not the application owner
/// unless they listed themselves, and specifically **not** "an administrator of
/// the server this was used in" — a reviewer sees other servers' free-text
/// answers and can grant recurring load on the host, so the gate is who runs the
/// bot, not who runs a server that happens to have added it.
pub fn is_reviewer(user: serenity::UserId) -> bool {
    reviewers().contains(&user.get())
}

/// What a reviewer needs to decide, and nothing they don't.
pub fn request_embed(request: &FullMapRequest, guild_name: Option<&str>) -> serenity::CreateEmbed {
    let title = match guild_name {
        Some(name) => format!("Full-map schedule request #{} — {name}", request.id),
        None => format!("Full-map schedule request #{}", request.id),
    };

    let color = match request.status.as_str() {
        "approved" => (60, 180, 75),
        "denied" => (200, 60, 60),
        // Withdrawn is neither a refusal nor still live — grey says "closed,
        // nobody judged it", which is exactly what happened.
        "withdrawn" => (130, 135, 140),
        _ => (250, 190, 60),
    };

    let mut embed = serenity::CreateEmbed::new()
        .title(title)
        .color(color)
        .field("Server", format!("`{}`", request.guild_id), true)
        .field(
            "Requested by",
            format!("<@{}>", request.requested_by),
            true,
        )
        // Context, not an input. Stated as such so a reviewer doesn't start
        // treating it as a threshold the bot is quietly applying.
        .field(
            "Members (context only)",
            request
                .member_count
                .map_or_else(|| "unknown".to_string(), |n| n.to_string()),
            true,
        )
        .field("Requested cadence", request.cadence.as_str(), false)
        .field("Posting to", format!("<#{}>", request.channel_id), true)
        .field("Status", request.status.as_str(), true)
        // Discord renders this in each reviewer's own timezone.
        .field("Filed", format!("<t:{}:R>", request.created_at), true);

    for (name, value) in [
        ("Use case", request.use_case.as_deref()),
        ("Expected audience", request.audience.as_deref()),
        ("Contact", request.contact.as_deref()),
    ] {
        if let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) {
            embed = embed.field(name, value, false);
        }
    }

    if let Some(reviewer) = request.reviewed_by {
        // A withdrawal isn't a judgement, and labelling the applicant who took
        // their own request back as having "decided" it reads as a refusal they
        // never received.
        let label = match request.status.as_str() {
            "withdrawn" => "Closed by",
            _ => "Decided by",
        };

        embed = embed.field(label, format!("<@{reviewer}>"), false);
    }

    embed
}

/// The buttons a request carries at its current status.
///
/// Pending offers Approve/Deny. Approved offers **Revoke**, because withdrawing
/// an approval belongs on the post that granted it — `/full-map-requests revoke`
/// wants a server id typed out by hand, which is a fine fallback and a poor
/// primary. Denied and withdrawn offer nothing: those posts are a record, and a
/// live button on one is how a settled decision gets overturned by a misclick.
pub fn review_buttons(request: &FullMapRequest) -> Vec<serenity::CreateActionRow> {
    let buttons = if request.is_pending() {
        vec![
            serenity::CreateButton::new(format!("{CUSTOM_ID_PREFIX}:approve:{}", request.id))
                .label("Approve")
                .style(serenity::ButtonStyle::Success),
            serenity::CreateButton::new(format!("{CUSTOM_ID_PREFIX}:deny:{}", request.id))
                .label("Deny")
                .style(serenity::ButtonStyle::Danger),
        ]
    } else if request.is_approved() {
        vec![
            serenity::CreateButton::new(format!("{CUSTOM_ID_PREFIX}:revoke:{}", request.id))
                .label("Revoke approval")
                .style(serenity::ButtonStyle::Danger),
        ]
    } else {
        return Vec::new();
    };

    vec![serenity::CreateActionRow::Buttons(buttons)]
}

/// The applicant's own way out of a request they no longer want.
///
/// Goes on the ephemeral reply they get when they run
/// `/request-full-map-schedule` with one already open — which is exactly where
/// somebody who filed the wrong thing ends up. Reviewers have Deny; without
/// this, an applicant who made a typo had no way to take it back, and the
/// one-pending-per-guild index meant they couldn't file a corrected one either.
pub fn withdraw_button(request: &FullMapRequest) -> Vec<serenity::CreateActionRow> {
    if !request.is_pending() {
        return Vec::new();
    }

    vec![serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(format!("{CUSTOM_ID_PREFIX}:withdraw:{}", request.id))
            .label("Withdraw request")
            .style(serenity::ButtonStyle::Secondary),
    ])]
}

/// Posts a new request to the review channel.
///
/// Every failure here is logged and swallowed. The request is already committed
/// by the time this runs, and losing the notification is a reviewer checking
/// `/full-map-requests` instead — losing the request because a channel id was
/// wrong would be unforgivable.
pub async fn announce(
    http: &serenity::Http,
    db: &Database,
    request: &FullMapRequest,
    guild_name: Option<&str>,
) {
    let Some(channel) = requests_channel() else {
        log::info!(
            "REQUESTS_CHANNEL_ID is unset — request #{} is reviewable with /full-map-requests only",
            request.id
        );
        return;
    };

    let message = serenity::CreateMessage::new()
        .embed(request_embed(request, guild_name))
        .components(review_buttons(request));

    let posted = match channel.send_message(http, message).await {
        Ok(posted) => posted,
        Err(err) => {
            log::warn!(
                "could not post request #{} to the review channel: {err} \
                 — it is still pending and visible to /full-map-requests",
                request.id
            );
            return;
        }
    };

    log::info!("posted request #{} for review", request.id);

    // Recorded so a decision made anywhere else can come back and correct this
    // post. Failing to record it costs the post going stale later, never the
    // request itself, so it is logged rather than raised.
    if let Err(err) = db
        .set_request_message(request.id, posted.id.get() as i64)
        .await
    {
        log::warn!(
            "posted request #{} but could not remember the message: {err} \
             — the post won't update if it's decided elsewhere",
            request.id
        );
    }
}

/// Brings a request's review post back into line with the row.
///
/// Needed because a decision can be made somewhere other than the post itself:
/// `/full-map-requests`, or the applicant's own Withdraw button. Without this the
/// post keeps showing `pending` with live buttons — a record that lies, which is
/// the one thing a review queue can't afford.
///
/// Skipped silently when there is no post to correct, and every failure is
/// logged rather than raised: the decision is already committed, and an
/// uneditable message (deleted, or in a channel the bot lost access to) must not
/// turn a completed decision into an error.
pub async fn refresh_post(http: &serenity::Http, request: &FullMapRequest) {
    let (Some(channel), Some(message_id)) = (requests_channel(), request.message_id) else {
        return;
    };

    let edit = serenity::EditMessage::new()
        .embed(request_embed(request, None))
        .components(review_buttons(request));

    if let Err(err) = channel
        .edit_message(http, serenity::MessageId::new(message_id as u64), edit)
        .await
    {
        log::warn!("could not update the post for request #{}: {err}", request.id);
    }
}

/// What a button press asks for.
#[derive(Clone, Copy)]
enum Action {
    Decide(RequestStatus),
    Revoke,
    Withdraw,
}

/// Handles a review button press. `Ok(false)` means the component wasn't ours.
pub async fn handle_button(
    ctx: &serenity::Context,
    db: &Database,
    interaction: &serenity::ComponentInteraction,
) -> Result<bool, serenity::Error> {
    let Some(rest) = interaction.data.custom_id.strip_prefix(CUSTOM_ID_PREFIX) else {
        return Ok(false);
    };

    let mut parts = rest.trim_start_matches(':').split(':');

    let action = match parts.next() {
        Some("approve") => Action::Decide(RequestStatus::Approved),
        Some("deny") => Action::Decide(RequestStatus::Denied),
        Some("revoke") => Action::Revoke,
        Some("withdraw") => Action::Withdraw,
        _ => return Ok(false),
    };

    let Some(id) = parts.next().and_then(|id| id.parse::<i64>().ok()) else {
        return Ok(false);
    };

    let actor = interaction.user.id.get() as i64;

    // Withdrawing is the applicant's own button, not a reviewer's. It only ever
    // appears on the ephemeral reply to `/request-full-map-schedule`, which
    // already needs Manage Webhooks in that server, so the person pressing it is
    // the person who could have filed it — but the check below is what makes
    // that true rather than merely likely.
    if !matches!(action, Action::Withdraw) && !is_reviewer(interaction.user.id) {
        reply(ctx, interaction, "Only a reviewer can decide these requests.").await?;
        return Ok(true);
    }

    let outcome = match action {
        Action::Decide(status) => db.review_full_map_request(id, status, actor).await,
        Action::Revoke => revoke(db, id, actor).await,
        Action::Withdraw => match withdrawable(db, id, interaction).await {
            Ok(true) => db.withdraw_full_map_request(id, actor).await,
            Ok(false) => {
                reply(ctx, interaction, "That isn't your request to withdraw.").await?;
                return Ok(true);
            }
            Err(err) => Err(err),
        },
    };

    let decided = match outcome {
        Ok(decided) => decided,
        Err(err) => {
            log::warn!("could not record a decision on request #{id}: {err}");
            reply(ctx, interaction, "Couldn't record that. Nothing changed.").await?;
            return Ok(true);
        }
    };

    // Every statement above matches on the status its button assumed, so `None`
    // here is somebody else having got there first — not an error, and
    // specifically not a decision to overturn.
    let Some(request) = decided else {
        reply(
            ctx,
            interaction,
            "That request has already been decided by someone else.",
        )
        .await?;
        return Ok(true);
    };

    // Edit the message the button was on rather than replying under it: the
    // reviewer's post *is* the record of the request, and a stale "pending"
    // embed with dead buttons under it is how two reviewers end up disagreeing
    // about what happened. The applicant's ephemeral reply gets plain text —
    // they filed the thing, they don't need it read back at them.
    let updated = match action {
        Action::Withdraw => serenity::CreateInteractionResponseMessage::new()
            .content(format!(
                "Request **#{}** withdrawn. You can file a new one whenever you like.",
                request.id
            ))
            .components(Vec::new()),
        _ => serenity::CreateInteractionResponseMessage::new()
            .embed(request_embed(&request, None))
            .components(review_buttons(&request)),
    };

    interaction
        .create_response(ctx, serenity::CreateInteractionResponse::UpdateMessage(updated))
        .await?;

    // A withdrawal happens on the applicant's own ephemeral reply, so the
    // reviewers' post is somewhere else entirely and still reads "pending".
    // The other actions were pressed on that post, and the edit above *was* the
    // correction.
    if matches!(action, Action::Withdraw) {
        refresh_post(&ctx.http, &request).await;
    }

    // Only an answer to an open application gets announced. A withdrawal is the
    // applicant's own doing, and a revocation is already the dormancy notice's
    // job — it says the same thing, in the channel the reports actually go to,
    // and saying it twice from two places is how they end up disagreeing.
    if matches!(action, Action::Decide(_)) {
        notify_requester(ctx, &request).await;
    }

    Ok(true)
}

/// May the presser withdraw this request?
///
/// The applicant, or anyone a reviewer would let act anyway. The guild check is
/// the one that matters: a component interaction can only reach a user Discord
/// sent it to, but tying the button to the request's own server means a stray
/// custom id can never take back somebody else's application.
async fn withdrawable(
    db: &Database,
    id: i64,
    interaction: &serenity::ComponentInteraction,
) -> Result<bool, sqlx::Error> {
    let Some(request) = db.get_full_map_request(id).await? else {
        return Ok(false);
    };

    if is_reviewer(interaction.user.id) {
        return Ok(true);
    }

    let same_guild = interaction
        .guild_id
        .is_some_and(|guild| guild.get() as i64 == request.guild_id);

    Ok(same_guild && request.requested_by == interaction.user.id.get() as i64)
}

/// Withdraws the approval a request granted.
///
/// Shaped like [`Database::review_full_map_request`] so the button handler can
/// treat the two identically: `Some` is the updated row, `None` is "that wasn't
/// the standing approval any more" — an unknown id, an already-withdrawn
/// request, or another reviewer pressing first.
async fn revoke(
    db: &Database,
    id: i64,
    reviewer: i64,
) -> Result<Option<FullMapRequest>, sqlx::Error> {
    let Some(request) = db.get_full_map_request(id).await? else {
        return Ok(None);
    };

    if !request.is_approved() {
        return Ok(None);
    }

    match db.revoke_full_map_approval(request.guild, reviewer).await? {
        // The row it closed *is* this one — an approval only ever comes from the
        // guild's single approved request — so read it back for the new embed.
        Revoked::Withdrawn(_) => db.get_full_map_request(id).await,
        Revoked::NotApproved => Ok(None),
    }
}

/// Tells the requesting server what was decided, in the channel they nominated
/// for the reports.
///
/// Not a DM. `docs/privacy.md` states the bot never sends one, and that promise
/// is worth more than the convenience — the channel they picked is somewhere
/// they are already expecting this bot to speak.
pub async fn notify_requester(ctx: &serenity::Context, request: &FullMapRequest) {
    let outcome = if request.status == RequestStatus::Approved.as_str() {
        "**approved** — set one up with `/schedule-report`."
    } else {
        "not approved this time. `/full-map` still renders the world map on demand, for free."
    };

    let body = format!(
        "<@{}> — your full-map schedule request was {outcome}",
        request.requested_by
    );

    let channel = serenity::ChannelId::new(request.channel_id as u64);

    if let Err(err) = channel
        .send_message(ctx, serenity::CreateMessage::new().content(body))
        .await
    {
        log::warn!(
            "decided request #{} but could not tell <#{}>: {err}",
            request.id,
            request.channel_id
        );
    }
}

async fn reply(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    content: &str,
) -> Result<(), serenity::Error> {
    interaction
        .create_response(
            ctx,
            serenity::CreateInteractionResponse::Message(
                serenity::CreateInteractionResponseMessage::new()
                    .content(content)
                    .ephemeral(true),
            ),
        )
        .await
}
