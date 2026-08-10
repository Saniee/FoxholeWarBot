use sqlx::{postgres::PgPoolOptions, FromRow, PgPool};

/// A Foxhole live shard. `shard` in the database stores [`Shard::api_url`],
/// `shard_name` stores [`Shard::as_str`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shard {
    Able,
    Baker,
    Charlie,
}

impl Shard {
    pub fn as_str(&self) -> &'static str {
        match self {
            Shard::Able => "Able",
            Shard::Baker => "Baker",
            Shard::Charlie => "Charlie",
        }
    }

    pub fn api_url(&self) -> &'static str {
        match self {
            Shard::Able => "https://war-service-live.foxholeservices.com/api",
            Shard::Baker => "https://war-service-live-2.foxholeservices.com/api",
            Shard::Charlie => "https://war-service-live-3.foxholeservices.com/api",
        }
    }

    /// Unknown values fall back to Able, matching the pre-rewrite behavior.
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "baker" => Shard::Baker,
            "charlie" => Shard::Charlie,
            _ => Shard::Able,
        }
    }

    pub fn list_all() -> [Shard; 3] {
        [Shard::Able, Shard::Baker, Shard::Charlie]
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct GuildData {
    pub id: i64,
    pub guild_id: i64,
    pub shard: String,
    pub shard_name: String,
    pub show_command_output: bool,
    pub full_map_faction_tint: bool,
    /// Draw the contested boundary between the factions. Unlike the tint, this
    /// applies to `/get-map` as well: it answers a sub-region question, and the
    /// hex a player cares about most is the contested one.
    pub frontline: bool,
    /// This server's default timezone, as an IANA name. Every new schedule
    /// starts from it; `/schedule-report` can override it for one report.
    pub timezone: String,
    /// May this guild put a full-map report on a timer? Rendering one on demand
    /// never consults this. See `specs/premium-full-map.md`.
    pub full_map_approved: bool,
}

impl GuildData {
    pub fn shard(&self) -> Shard {
        Shard::from_str(&self.shard_name)
    }
}

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct JobData {
    pub id: i64,
    /// `guilds.id` (the surrogate row id), *not* the Discord snowflake.
    pub guild: i64,
    pub job_name: String,
    /// A string the scheduler accepts. Generated from the user's choices now;
    /// older rows hold whatever phrase was typed, and both still parse.
    pub schedule: String,
    /// The cadence in words, for embeds and listings. `None` for rows created
    /// before `migrations/0005_schedule_input.sql`.
    pub schedule_label: Option<String>,
    /// IANA name, resolved when the schedule was created and stored here so a
    /// later change to the guild's default can't silently move this report.
    pub timezone: String,
    pub webhook_url: String,
    /// The region this job renders, or `None` for a full-map job. See
    /// `migrations/0003_full_map_gate.sql` for why the absence *is* the marker.
    pub map_name: Option<String>,
    pub draw_text: bool,
    pub job_id: Option<String>,
    /// Whether this job's channel has already been told the schedule went
    /// dormant. Only ever true for a full-map job, since nothing else is gated.
    pub dormant_notified: bool,
}

/// One `cronjobs` row plus the Discord id of the guild that owns it. Columns are
/// aliased in the query because both tables have an `id`; flat rather than
/// nested because sqlx's `FromRow` maps columns, not sub-structs.
///
/// Only the id is carried over from `guilds` — each tick re-reads the guild's
/// settings anyway, so a shard change takes effect without a restart.
#[derive(Debug, Clone, FromRow)]
pub struct JobWithGuild {
    pub job_row_id: i64,
    pub guild_row_id: i64,
    pub job_name: String,
    pub schedule: String,
    pub schedule_label: Option<String>,
    pub timezone: String,
    pub webhook_url: String,
    pub map_name: Option<String>,
    pub draw_text: bool,
    pub job_id: Option<String>,
    pub guild_id: i64,
}

/// Everything needed to persist a schedule. The scheduler UUID is included
/// because the job is registered with the scheduler *before* the row is
/// written (QA B-3: no orphan rows when scheduling fails).
#[derive(Debug, Clone)]
pub struct NewJob {
    pub guild: i64,
    pub job_name: String,
    pub schedule: String,
    pub schedule_label: Option<String>,
    pub timezone: String,
    pub webhook_url: String,
    pub map_name: Option<String>,
    pub draw_text: bool,
    pub job_id: String,
}

/// Where a full-map schedule request stands. Mirrors the `status` CHECK in
/// `migrations/0003_full_map_gate.sql`; the two have to be changed together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestStatus {
    Pending,
    Approved,
    Denied,
    Withdrawn,
}

impl RequestStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RequestStatus::Pending => "pending",
            RequestStatus::Approved => "approved",
            RequestStatus::Denied => "denied",
            RequestStatus::Withdrawn => "withdrawn",
        }
    }
}

/// One row of the application queue, with the applying guild's Discord snowflake
/// joined on — a reviewer sitting in the support server has no other way to tell
/// which guild `guild = 4` is.
///
/// Timestamps come back as epoch seconds rather than as a date type: it keeps
/// `sqlx` off a datetime feature, and Discord's own `<t:…:R>` markup wants
/// exactly this, so the reviewer's list renders "2 hours ago" in their locale
/// without the bot formatting anything.
#[derive(Debug, Clone, FromRow)]
pub struct FullMapRequest {
    pub id: i64,
    /// `guilds.id` (the surrogate row id), like `cronjobs.guild`.
    pub guild: i64,
    /// The applying guild's Discord snowflake, joined from `guilds`.
    pub guild_id: i64,
    pub requested_by: i64,
    /// Snapshot taken when the form was filed. Review context only — nothing
    /// reads it to decide anything (`specs/premium-full-map.md`).
    pub member_count: Option<i32>,
    pub cadence: String,
    pub channel_id: i64,
    pub use_case: Option<String>,
    pub audience: Option<String>,
    pub contact: Option<String>,
    pub status: String,
    pub reviewed_by: Option<i64>,
    /// The review post in `REQUESTS_CHANNEL_ID`, once it has been made. `None`
    /// means there is nothing to keep in step — no channel configured, the post
    /// failed, or the request predates the column.
    pub message_id: Option<i64>,
    pub created_at: i64,
}

impl FullMapRequest {
    pub fn is_pending(&self) -> bool {
        self.status == RequestStatus::Pending.as_str()
    }

    /// True only while this request *is* the guild's standing approval. Goes
    /// false the moment it's withdrawn, which is what stops a revoked post from
    /// still offering a Revoke button.
    pub fn is_approved(&self) -> bool {
        self.status == RequestStatus::Approved.as_str()
    }
}

/// The outcome of withdrawing a guild's approval.
///
/// Two facts, not one: whether anything changed, and whether there is still a
/// request on file to correct. An `Option<FullMapRequest>` would collapse them —
/// a guild whose request has been purged would look identical to a guild that
/// was never approved, and the caller would report "nothing to withdraw" having
/// just withdrawn it.
pub enum Revoked {
    /// It wasn't approved. Nothing changed.
    NotApproved,
    /// Approval withdrawn. Carries the closed request when there is one, so its
    /// review post can be brought back into line.
    Withdrawn(Box<Option<FullMapRequest>>),
}

/// The answers from the application modal, plus what the bot fills in itself.
#[derive(Debug, Clone)]
pub struct NewFullMapRequest {
    /// `guilds.id`, not the Discord snowflake.
    pub guild: i64,
    pub requested_by: i64,
    pub member_count: Option<i32>,
    pub cadence: String,
    pub channel_id: i64,
    pub use_case: Option<String>,
    pub audience: Option<String>,
    pub contact: Option<String>,
}

/// Every `full_map_requests` read goes through this, so the joined guild
/// snowflake and the epoch conversion are written once. `{where}` is a literal
/// in every caller — nothing user-supplied is ever formatted in here.
const REQUEST_SELECT: &str = "SELECT r.id, r.guild, g.guild_id, r.requested_by, r.member_count, \
                                     r.cadence, r.channel_id, r.use_case, r.audience, r.contact, \
                                     r.status, r.reviewed_by, r.message_id, \
                                     EXTRACT(EPOCH FROM r.created_at)::BIGINT AS created_at \
                              FROM full_map_requests r \
                              JOIN guilds g ON g.id = r.guild";

/// One row of a usage summary: a bucket, and either one command's tally in it or
/// — when `command` is `None` — the bucket's own total.
///
/// The `None` row is not a convenience. `uses` can be summed across commands but
/// `servers` cannot: a server that ran three different commands on Tuesday is one
/// server, and adding the per-command distinct counts would report three. The
/// grouping set that produces this row is the only place that count is right.
///
/// `bucket` is text rather than a date for the same reason [`FullMapRequest`]'s
/// timestamps are epoch seconds: it keeps `sqlx` off a datetime feature. Here it
/// is also what gets printed, so formatting it in SQL means it is formatted once.
#[derive(Debug, Clone, FromRow)]
pub struct UsageBucket {
    /// `YYYY-MM-DD` — the day, or the Monday the week starts on.
    pub bucket: String,
    /// The command counted, or `None` for the bucket's total across all of them.
    pub command: Option<String>,
    pub uses: i64,
    /// Distinct servers, over exactly the rows this row summarises.
    pub servers: i64,
}

/// Whether a usage summary is bucketed by day or by week.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsagePeriod {
    Daily,
    Weekly,
}

/// The standing picture: what exists right now, as opposed to what happened.
///
/// Read live from `guilds` and `cronjobs` rather than counted into
/// `usage_daily`, because these are not events. "Nine schedules exist" is a fact
/// about the present that a tally of the past can only approximate.
#[derive(Debug, Clone, FromRow)]
pub struct UsageOverview {
    /// Servers that have run `/set-guild-settings`. Not the same as the servers
    /// the bot is in — a server that never set a shard has no row here.
    pub guilds: i64,
    pub approved_guilds: i64,
    pub schedules: i64,
    /// Of those, the ones rendering the whole world map (`map_name IS NULL`).
    pub full_map_schedules: i64,
    pub scheduling_guilds: i64,
}

#[derive(Clone)]
pub struct Database {
    pub conn: PgPool,
}

impl Database {
    /// Connects to `DATABASE_URL` and applies the embedded migrations.
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let conn = PgPoolOptions::new().max_connections(5).connect(url).await?;
        let db = Database { conn };
        db.migrate().await?;
        Ok(db)
    }

    /// Migrations are embedded at compile time, so building needs no live database.
    async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::migrate!("./migrations")
            .run(&self.conn)
            .await
            .map_err(|e| sqlx::Error::Migrate(Box::new(e)))
    }

    // -- guilds ---------------------------------------------------------------

    pub async fn get_guild(&self, guild_id: i64) -> Result<Option<GuildData>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM guilds WHERE guild_id = $1")
            .bind(guild_id)
            .fetch_optional(&self.conn)
            .await
    }

    /// Lookup by the surrogate row id, which is what `cronjobs.guild` stores.
    pub async fn get_guild_by_row(&self, id: i64) -> Result<Option<GuildData>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM guilds WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.conn)
            .await
    }

    /// Idempotent create-or-update. Unlike the old `create_guild` this honors the
    /// caller's `show` flag on first insert instead of hardcoding it off (QA B-1),
    /// and relies on `guild_id UNIQUE` so a guild can never end up with two rows
    /// (QA C-10).
    ///
    /// `tint` is optional in the "leave it alone" sense, not the "default it"
    /// sense: `/set-guild-settings` requires the shard every time, so a guild
    /// changing shards would silently lose its tint setting if an absent option
    /// meant `FALSE`. `None` keeps whatever is already there, and only an insert
    /// falls back to off. `frontline` is optional in exactly the same sense.
    /// `timezone` is optional in the same "leave it alone" sense as `tint`, and
    /// for the same reason: a server changing shards must not have its clock
    /// quietly reset to UTC underneath its existing schedules.
    pub async fn upsert_guild(
        &self,
        guild_id: i64,
        shard: Shard,
        show: bool,
        tint: Option<bool>,
        frontline: Option<bool>,
        timezone: Option<&str>,
    ) -> Result<GuildData, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO guilds \
                 (guild_id, shard, shard_name, show_command_output, full_map_faction_tint, \
                  frontline, timezone) \
             VALUES ($1, $2, $3, $4, COALESCE($5::BOOLEAN, FALSE), \
                     COALESCE($6::BOOLEAN, FALSE), COALESCE($7::TEXT, 'UTC')) \
             ON CONFLICT (guild_id) DO UPDATE \
             SET shard = EXCLUDED.shard, \
                 shard_name = EXCLUDED.shard_name, \
                 show_command_output = EXCLUDED.show_command_output, \
                 full_map_faction_tint = COALESCE($5::BOOLEAN, guilds.full_map_faction_tint), \
                 frontline = COALESCE($6::BOOLEAN, guilds.frontline), \
                 timezone = COALESCE($7::TEXT, guilds.timezone) \
             RETURNING *",
        )
        .bind(guild_id)
        .bind(shard.api_url())
        .bind(shard.as_str())
        .bind(show)
        .bind(tint)
        .bind(frontline)
        .bind(timezone)
        .fetch_one(&self.conn)
        .await
    }

    /// Cascades to the guild's `cronjobs` rows via the FK.
    ///
    /// `usage_daily` is deleted by hand in the same transaction because it holds
    /// the Discord snowflake rather than a reference to `guilds.id`, so there is
    /// no FK to cascade down (see `migrations/0007_usage_stats.sql`). Together,
    /// or the promise in `docs/tos.md` — removing the bot deletes what that
    /// server left behind — would be true of the settings and false of the
    /// counts.
    pub async fn delete_guild(&self, guild_id: i64) -> Result<(), sqlx::Error> {
        let mut tx = self.conn.begin().await?;

        sqlx::query("DELETE FROM guilds WHERE guild_id = $1")
            .bind(guild_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM usage_daily WHERE guild_id = $1")
            .bind(guild_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await
    }

    // -- cronjobs -------------------------------------------------------------

    /// Job names are unique *per guild* now, so lookups must be scoped (QA B-2).
    pub async fn get_job_entry(
        &self,
        guild: i64,
        job_name: &str,
    ) -> Result<Option<JobData>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM cronjobs WHERE guild = $1 AND job_name = $2")
            .bind(guild)
            .bind(job_name)
            .fetch_optional(&self.conn)
            .await
    }

    pub async fn get_jobs_for_guild(&self, guild: i64) -> Result<Vec<JobData>, sqlx::Error> {
        sqlx::query_as("SELECT * FROM cronjobs WHERE guild = $1 ORDER BY job_name")
            .bind(guild)
            .fetch_all(&self.conn)
            .await
    }

    /// Every job paired with the guild that owns it. Startup restoration uses this
    /// so each job renders its *own* guild's shard instead of reusing the first
    /// row's guild for all of them (QA C-7).
    pub async fn all_jobs_with_guilds(&self) -> Result<Vec<JobWithGuild>, sqlx::Error> {
        sqlx::query_as(
            "SELECT c.id AS job_row_id, c.guild AS guild_row_id, c.job_name, c.schedule, \
                    c.schedule_label, c.timezone, \
                    c.webhook_url, c.map_name, c.draw_text, c.job_id, g.guild_id \
             FROM cronjobs c \
             JOIN guilds g ON g.id = c.guild",
        )
        .fetch_all(&self.conn)
        .await
    }

    /// Inserts a schedule that has already been accepted by the scheduler.
    /// A duplicate `(guild, job_name)` surfaces as a unique-violation error.
    pub async fn add_job_entry(&self, job: &NewJob) -> Result<JobData, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO cronjobs \
                 (guild, job_name, schedule, webhook_url, map_name, draw_text, job_id, \
                  schedule_label, timezone) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *",
        )
        .bind(job.guild)
        .bind(&job.job_name)
        .bind(&job.schedule)
        .bind(&job.webhook_url)
        .bind(&job.map_name)
        .bind(job.draw_text)
        .bind(&job.job_id)
        .bind(&job.schedule_label)
        .bind(&job.timezone)
        .fetch_one(&self.conn)
        .await
    }

    pub async fn remove_job_entry(&self, guild: i64, job_name: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM cronjobs WHERE guild = $1 AND job_name = $2")
            .bind(guild)
            .bind(job_name)
            .execute(&self.conn)
            .await?;
        Ok(())
    }

    pub async fn update_job_uuid(&self, id: i64, uuid: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE cronjobs SET job_id = $1 WHERE id = $2")
            .bind(uuid)
            .bind(id)
            .execute(&self.conn)
            .await?;
        Ok(())
    }

    /// Records that this job's channel has been told it went dormant, so the
    /// next tick doesn't say it again.
    pub async fn set_dormant_notified(
        &self,
        guild: i64,
        job_name: &str,
        notified: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE cronjobs SET dormant_notified = $3 WHERE guild = $1 AND job_name = $2")
            .bind(guild)
            .bind(job_name)
            .bind(notified)
            .execute(&self.conn)
            .await?;
        Ok(())
    }

    // -- full-map schedule requests -------------------------------------------

    /// Files an application. The partial unique index rejects a second pending
    /// row for the same guild as a unique violation — callers check first for a
    /// civil message, but the index is what actually holds the rule.
    pub async fn create_full_map_request(
        &self,
        req: &NewFullMapRequest,
    ) -> Result<FullMapRequest, sqlx::Error> {
        let id: (i64,) = sqlx::query_as(
            "INSERT INTO full_map_requests \
                 (guild, requested_by, member_count, cadence, channel_id, use_case, audience, contact) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
        )
        .bind(req.guild)
        .bind(req.requested_by)
        .bind(req.member_count)
        .bind(&req.cadence)
        .bind(req.channel_id)
        .bind(&req.use_case)
        .bind(&req.audience)
        .bind(&req.contact)
        .fetch_one(&self.conn)
        .await?;

        // Read back rather than RETURNING *: the row a reviewer sees carries the
        // guild's snowflake, which lives in the other table.
        self.get_full_map_request(id.0)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn get_full_map_request(
        &self,
        id: i64,
    ) -> Result<Option<FullMapRequest>, sqlx::Error> {
        sqlx::query_as(&format!("{REQUEST_SELECT} WHERE r.id = $1"))
            .bind(id)
            .fetch_optional(&self.conn)
            .await
    }

    /// The guild's open application, if it has one.
    pub async fn pending_request_for_guild(
        &self,
        guild: i64,
    ) -> Result<Option<FullMapRequest>, sqlx::Error> {
        sqlx::query_as(&format!(
            "{REQUEST_SELECT} WHERE r.guild = $1 AND r.status = 'pending'"
        ))
        .bind(guild)
        .fetch_optional(&self.conn)
        .await
    }

    /// The reviewer's queue, oldest first — whoever has been waiting longest is
    /// the one to answer next.
    pub async fn pending_full_map_requests(&self) -> Result<Vec<FullMapRequest>, sqlx::Error> {
        sqlx::query_as(&format!(
            "{REQUEST_SELECT} WHERE r.status = 'pending' ORDER BY r.created_at"
        ))
        .fetch_all(&self.conn)
        .await
    }

    /// Records a decision, and for an approval flips the guild's flag in the
    /// same transaction.
    ///
    /// `WHERE status = 'pending'` is the whole concurrency story: the channel
    /// post's buttons and `/full-map-requests` drive this one statement, so a
    /// second reviewer clicking Deny on an already-approved request updates no
    /// rows and gets `None` back rather than quietly overturning the first
    /// decision. An approval that can't reach the guild row leaves the request
    /// pending too — the flag and the row can't disagree.
    pub async fn review_full_map_request(
        &self,
        id: i64,
        status: RequestStatus,
        reviewed_by: i64,
    ) -> Result<Option<FullMapRequest>, sqlx::Error> {
        let mut tx = self.conn.begin().await?;

        let reviewed: Option<(i64,)> = sqlx::query_as(
            "UPDATE full_map_requests \
             SET status = $2, reviewed_by = $3, reviewed_at = now() \
             WHERE id = $1 AND status = 'pending' \
             RETURNING guild",
        )
        .bind(id)
        .bind(status.as_str())
        .bind(reviewed_by)
        .fetch_optional(&mut *tx)
        .await?;

        let Some((guild,)) = reviewed else {
            tx.rollback().await?;
            return Ok(None);
        };

        if status == RequestStatus::Approved {
            sqlx::query(
                "UPDATE guilds SET full_map_approved = TRUE, full_map_approved_at = now() \
                 WHERE id = $1",
            )
            .bind(guild)
            .execute(&mut *tx)
            .await?;

            // A guild that was revoked and is now approved again gets to hear
            // about it if it is ever revoked a second time.
            sqlx::query("UPDATE cronjobs SET dormant_notified = FALSE WHERE guild = $1")
                .bind(guild)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;

        self.get_full_map_request(id).await
    }

    /// Remembers which message is this request's review post, so a decision made
    /// anywhere else can go back and correct it.
    pub async fn set_request_message(&self, id: i64, message_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE full_map_requests SET message_id = $2 WHERE id = $1")
            .bind(id)
            .bind(message_id)
            .execute(&self.conn)
            .await?;

        Ok(())
    }

    /// Takes back a request that hasn't been decided yet. `None` means it was no
    /// longer pending — already decided, already withdrawn, or never existed.
    ///
    /// Separate from [`Self::revoke_full_map_approval`] because they undo
    /// different things: this one closes an application nobody has answered, and
    /// touches no guild flag, since a pending request never set one. Both land on
    /// `withdrawn`, which is right — neither is a refusal, and the reviewer's
    /// judgement shouldn't be recorded where none was given.
    pub async fn withdraw_full_map_request(
        &self,
        id: i64,
        withdrawn_by: i64,
    ) -> Result<Option<FullMapRequest>, sqlx::Error> {
        let updated: Option<(i64,)> = sqlx::query_as(
            "UPDATE full_map_requests \
             SET status = $3, reviewed_by = $2, reviewed_at = now() \
             WHERE id = $1 AND status = 'pending' \
             RETURNING id",
        )
        .bind(id)
        .bind(withdrawn_by)
        .bind(RequestStatus::Withdrawn.as_str())
        .fetch_optional(&self.conn)
        .await?;

        if updated.is_none() {
            return Ok(None);
        }

        self.get_full_map_request(id).await
    }

    /// Deletes requests that were turned down or taken back over 90 days ago.
    ///
    /// The window is stated in `docs/privacy.md`, so this is not housekeeping we
    /// can quietly skip — it is the retention policy, and the only thing that
    /// makes the sentence in the docs true. Approved requests are kept: they are
    /// the record of what the standing approval was granted for.
    pub async fn purge_stale_full_map_requests(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM full_map_requests \
             WHERE status IN ('denied', 'withdrawn') \
               AND COALESCE(reviewed_at, created_at) < now() - INTERVAL '90 days'",
        )
        .execute(&self.conn)
        .await?;

        Ok(result.rows_affected())
    }

    // There is deliberately no `set_full_map_approved(guild, bool)`. Granting and
    // revoking are not one operation with a flag: granting answers a pending
    // request, revoking closes an approved one, and each has to move a different
    // second row in the same transaction. A shared setter could only do the flag,
    // which is how the flag and the request start disagreeing.

    /// Withdraws a guild's approval and closes the request that granted it, in
    /// one transaction.
    ///
    /// The two facts move together on purpose. An approval lives on the guild
    /// row, but the *reason* for it is the request, and a guild whose flag is
    /// off while its request still reads `approved` is a reviewer looking at a
    /// post that lies. Marking it `withdrawn` also puts it back in reach of the
    /// 90-day purge, which is what `docs/privacy.md` says happens to a request
    /// that is no longer in force.
    ///
    /// `WHERE full_map_approved` makes a second press a no-op rather than a
    /// second revocation — both revoke surfaces need that, since a channel post
    /// stays clickable long after the decision.
    ///
    /// Schedules are left alone. The tick re-reads the flag and goes dormant, so
    /// re-approving resumes the job the guild already set up.
    pub async fn revoke_full_map_approval(
        &self,
        guild: i64,
        reviewed_by: i64,
    ) -> Result<Revoked, sqlx::Error> {
        let mut tx = self.conn.begin().await?;

        let revoked: Option<(i64,)> = sqlx::query_as(
            "UPDATE guilds SET full_map_approved = FALSE, full_map_approved_at = NULL \
             WHERE id = $1 AND full_map_approved \
             RETURNING id",
        )
        .bind(guild)
        .fetch_optional(&mut *tx)
        .await?;

        if revoked.is_none() {
            tx.rollback().await?;
            return Ok(Revoked::NotApproved);
        }

        // At most one row: the partial unique index allows one pending request
        // per guild, and an approval can only come from one that was pending.
        let closed: Option<(i64,)> = sqlx::query_as(
            "UPDATE full_map_requests \
             SET status = $3, reviewed_by = $2, reviewed_at = now() \
             WHERE guild = $1 AND status = 'approved' \
             RETURNING id",
        )
        .bind(guild)
        .bind(reviewed_by)
        .bind(RequestStatus::Withdrawn.as_str())
        .fetch_optional(&mut *tx)
        .await?;

        tx.commit().await?;

        // The flag is off either way. A guild whose request has since been
        // purged has nothing to read back, and that is not a failure to revoke —
        // which is exactly why this isn't an `Option<FullMapRequest>`, where
        // "nothing to correct" and "nothing happened" would look identical.
        let request = match closed {
            Some((id,)) => self.get_full_map_request(id).await?,
            None => None,
        };

        Ok(Revoked::Withdrawn(Box::new(request)))
    }

    // -- usage counters -------------------------------------------------------

    /// Adds one to today's tally for this command in this server.
    ///
    /// The date comes from `now() AT TIME ZONE 'utc'` rather than `CURRENT_DATE`
    /// on purpose: `CURRENT_DATE` is read in the database session's timezone, so
    /// the day a use lands in would depend on how the Postgres container happens
    /// to be configured — and would silently change if that ever moved.
    ///
    /// The upsert is the whole concurrency story. Two commands finishing in the
    /// same millisecond in the same server contend on one row and both
    /// increments land; there is no read-then-write for them to race in.
    pub async fn record_usage(&self, command: &str, guild_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO usage_daily (day, command, guild_id, uses) \
             VALUES ((now() AT TIME ZONE 'utc')::date, $1, $2, 1) \
             ON CONFLICT (day, command, guild_id) \
             DO UPDATE SET uses = usage_daily.uses + 1",
        )
        .bind(command)
        .bind(guild_id)
        .execute(&self.conn)
        .await?;

        Ok(())
    }

    /// The last `buckets` days or weeks, one row per command per bucket plus a
    /// total row per bucket (`command IS NULL`).
    ///
    /// Buckets with no usage at all are simply absent — nothing was written for
    /// them. The caller generates the labels it expects and fills the gaps with
    /// zeroes, because "no row" and "nobody used it" are the same fact here and
    /// a table that skips a quiet Sunday reads as if the data were lost.
    ///
    /// `GROUPING SETS` rather than two queries: the per-command tallies and the
    /// bucket's distinct-server count have to come from the same scan, or a use
    /// recorded between them would appear in one and not the other.
    pub async fn usage_buckets(
        &self,
        period: UsagePeriod,
        buckets: i64,
    ) -> Result<Vec<UsageBucket>, sqlx::Error> {
        // Both halves are literals chosen by the enum — nothing user-supplied is
        // ever formatted into this.
        let (bucket_expr, window) = match period {
            UsagePeriod::Daily => ("day", "day > (now() AT TIME ZONE 'utc')::date - $1::int"),
            UsagePeriod::Weekly => (
                "date_trunc('week', day)::date",
                "day >= date_trunc('week', (now() AT TIME ZONE 'utc')::date)::date \
                        - (($1::int - 1) * 7)",
            ),
        };

        sqlx::query_as(&format!(
            "SELECT to_char(b.bucket, 'YYYY-MM-DD') AS bucket, \
                    b.command AS command, \
                    SUM(b.uses)::BIGINT AS uses, \
                    COUNT(DISTINCT b.guild_id)::BIGINT AS servers \
             FROM (SELECT {bucket_expr} AS bucket, command, guild_id, uses \
                   FROM usage_daily WHERE {window}) b \
             GROUP BY GROUPING SETS ((b.bucket, b.command), (b.bucket)) \
             ORDER BY b.bucket DESC, b.command"
        ))
        .bind(buckets)
        .fetch_all(&self.conn)
        .await
    }

    /// What exists right now — servers, schedules, approvals.
    pub async fn usage_overview(&self) -> Result<UsageOverview, sqlx::Error> {
        sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM guilds)::BIGINT AS guilds, \
                    (SELECT COUNT(*) FROM guilds WHERE full_map_approved)::BIGINT \
                        AS approved_guilds, \
                    (SELECT COUNT(*) FROM cronjobs)::BIGINT AS schedules, \
                    (SELECT COUNT(*) FROM cronjobs WHERE map_name IS NULL)::BIGINT \
                        AS full_map_schedules, \
                    (SELECT COUNT(DISTINCT guild) FROM cronjobs)::BIGINT AS scheduling_guilds",
        )
        .fetch_one(&self.conn)
        .await
    }

    /// Enforces the retention window `docs/privacy.md` promises for usage
    /// counts, the same 90 days closed full-map requests get.
    pub async fn purge_stale_usage(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM usage_daily WHERE day < (now() AT TIME ZONE 'utc')::date - 90",
        )
        .execute(&self.conn)
        .await?;

        Ok(result.rows_affected())
    }
}
