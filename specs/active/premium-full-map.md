# Premium gating: scheduled full-map renders (ACTIVE)

Status: **proposed** — depends on a ToS check (below) before implementation.

## Gating model (locked)
The **full-map renderer** (all hex regions stitched together — see
`specs/active/rendering-placement.md` and its future full-map-renderer spec) is gated **only when
scheduled**:

| Path | Gated? | Rationale |
|---|---|---|
| On-demand command (a one-off "dry run" full-map render) | **Free** | The user explicitly triggers it and pays the render cost once. |
| Scheduled/recurring full-map report | **Premium** | The bot's infra pays the render cost repeatedly, indefinitely. |

Single-hex `/get-map`, `/war-report`, `/war-state`, and scheduling *hex* reports stay free and
unchanged. Only **scheduling a full-map** report requires an entitlement.

## Monetization approach (non-intrusive, no direct per-use payment)
- **Durable entitlement only.** Because a schedule is long-lived, the entitlement must be
  persistent — a **supporter/sponsor tier** (Ko-fi / GitHub Sponsors / Buy Me a Coffee) mapped to
  an entitlement, or an owner-granted per-guild "premium" flag.
- **Vote-walls are explicitly rejected here.** Top.gg-style votes expire (~12h) and would make a
  weeks-long schedule constantly lapse; and since on-demand is free, votes have no role in this
  feature.
- No watermarking of free renders; no Discord native App Subscriptions (that's a direct recurring
  payment, ruled out).

## Entitlement abstraction
Keep the *source* swappable so the mechanism can change without touching command logic:
```rust
#[async_trait]
trait Entitlements {
    /// Is this guild (or user) currently entitled to premium features?
    async fn is_premium(&self, guild_id: i64) -> bool;
}
```
Initial impl can be as simple as a `premium` boolean/`premium_until` timestamp on the `guilds`
row (Postgres), set by an owner-only admin command or a webhook from the supporter platform.
Later impls (Sponsors API, etc.) implement the same trait.

### Data
Add to the `guilds` table (folds into `specs/active/postgres-migration.md`):
```sql
premium        BOOLEAN NOT NULL DEFAULT FALSE,
premium_until  TIMESTAMPTZ                 -- NULL = not premium / no expiry tracking yet
```

## Where the check happens (both)
1. **At schedule creation** (`/schedule-report` when the target is the full map): if
   `!is_premium`, refuse with a friendly message explaining the perk and how to support. Free
   hex schedules skip the check entirely.
2. **At tick time**: each full-map scheduled tick re-checks entitlement. If it lapsed, the job
   goes **dormant** (skips rendering) and posts/logs a one-time heads-up rather than silently
   burning render cost forever. Re-granting entitlement reactivates it.

This ties into the scheduling overhaul (`specs/active/scheduling-overhaul.md`): the full-map job
type carries an `is_full_map` marker so the tick knows to apply the entitlement gate.

## Prerequisite / blocker — third-party ToS
The full map is built from **extracted Foxhole game assets** and Siege Camp's **War API**.
Monetizing anything derived from them — even indirectly (supporter revenue) — may be constrained
by their terms **regardless of how non-intrusive it is to users**. This is independent of the
mechanism and must be resolved **before** implementation:
- Confirm what the Foxhole War API terms / developer policy say about commercial or monetized use.
- Confirm redistribution/derivative terms for the map/icon assets.
- If terms disallow monetization: keep the full map entirely free (the cost-cadence caching tier
  from the earlier discussion can still limit infra cost without money changing hands).

## Open decisions
- Supporter platform (Ko-fi vs GitHub Sponsors vs manual grant) — pick once ToS is clear.
- On-demand full map: new command (e.g. `/full-map`) vs an option on `/get-map`.
- Whether entitlement is per-guild or per-user (per-guild recommended: schedules belong to guilds).

## Acceptance criteria (once unblocked)
- Scheduling a full-map report without entitlement is refused with a clear explanation; a
  one-off on-demand full-map render works for everyone.
- A full-map schedule created while entitled goes dormant (not deleted) if entitlement lapses,
  and resumes when re-granted.
- Free hex scheduling and all interactive commands are unaffected by the gate.
