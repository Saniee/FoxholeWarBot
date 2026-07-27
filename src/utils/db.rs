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
    pub schedule: String,
    pub webhook_url: String,
    pub map_name: String,
    pub draw_text: bool,
    pub job_id: Option<String>,
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
    pub webhook_url: String,
    pub map_name: String,
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
    pub webhook_url: String,
    pub map_name: String,
    pub draw_text: bool,
    pub job_id: String,
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
    pub async fn upsert_guild(
        &self,
        guild_id: i64,
        shard: Shard,
        show: bool,
    ) -> Result<GuildData, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO guilds (guild_id, shard, shard_name, show_command_output) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (guild_id) DO UPDATE \
             SET shard = EXCLUDED.shard, \
                 shard_name = EXCLUDED.shard_name, \
                 show_command_output = EXCLUDED.show_command_output \
             RETURNING *",
        )
        .bind(guild_id)
        .bind(shard.api_url())
        .bind(shard.as_str())
        .bind(show)
        .fetch_one(&self.conn)
        .await
    }

    /// Cascades to the guild's `cronjobs` rows via the FK.
    pub async fn delete_guild(&self, guild_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM guilds WHERE guild_id = $1")
            .bind(guild_id)
            .execute(&self.conn)
            .await?;
        Ok(())
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
            "INSERT INTO cronjobs (guild, job_name, schedule, webhook_url, map_name, draw_text, job_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *",
        )
        .bind(job.guild)
        .bind(&job.job_name)
        .bind(&job.schedule)
        .bind(&job.webhook_url)
        .bind(&job.map_name)
        .bind(job.draw_text)
        .bind(&job.job_id)
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
}
