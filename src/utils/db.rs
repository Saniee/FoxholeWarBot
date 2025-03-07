#![allow(dead_code, unused_imports)]

use sqlx::{sqlite, Pool, Sqlite, FromRow};

use crate::commands::schedule_report::ReportJob;

pub enum Shard {
    Able,
    Baker,
    Charlie
}

impl Shard {
    pub fn as_str(&self) -> String {
        match self {
            Shard::Able => "Able".to_string(),
            Shard::Baker => "Baker".to_string(),
            Shard::Charlie => "Charlie".to_string()
        }
    }

    pub fn api_url(&self) -> String {
        match self {
            Shard::Able => "https://war-service-live.foxholeservices.com/api".to_string(),
            Shard::Baker => "https://war-service-live-2.foxholeservices.com/api".to_string(),
            Shard::Charlie => "https://war-service-live-3.foxholeservices.com/api".to_string()
        }
    }

    pub fn from_str(str: &str) -> Self {
        match str.trim().to_lowercase().as_str() {
            "able" => Shard::Able,
            "baker" => Shard::Baker,
            "charlie" => Shard::Charlie,
            _ => Shard::Able
        }
    }

    pub fn list_all() -> Vec<Self> {
        vec![Shard::Able, Shard::Baker, Shard::Charlie]
    }
}

#[derive(Debug, FromRow)]
pub struct GuildData{
    pub id: i32,
    guild_id: i64,
    pub shard: String,
    pub shard_name: String,
    pub show_command_output: i32
}

#[derive(Debug, FromRow)]
pub struct JobData {
    pub id: i32,
    pub uuid: i64,
    pub job_name: String,
    pub schedule: String,
    pub webhook_url: String,
    pub map_name: String,
    pub draw_text: i32
}

#[derive(Clone)]
pub struct Database {
    pub conn: Pool<Sqlite>
}

impl Database {
    pub async fn connect() -> Self {
        let opt = sqlite::SqliteConnectOptions::new().filename("database.db").create_if_missing(true);

        let conn = sqlite::SqlitePool::connect_with(opt).await.unwrap();
        Database {
            conn
        }
    }

    pub async fn get_guild(&self, guild_id: u64) -> Option<GuildData> {
        sqlx::query_as("SELECT * FROM guilds WHERE guild_id == ?1").bind(i64::try_from(guild_id).unwrap()).fetch_optional(&self.conn).await.unwrap()
    }

    pub async fn create_guild(&self, guild_id: u64, shard_name: Option<&str>) {
        let data = (i64::try_from(guild_id).unwrap(), Shard::from_str(shard_name.unwrap_or("Able")), 0);
        sqlx::query("INSERT INTO guilds (guild_id, shard, shard_name, show_command_output) VALUES (?1, ?2, ?3, ?4)").bind(data.0).bind(data.1.api_url()).bind(data.1.as_str()).bind(data.2).execute(&self.conn).await.unwrap();
    }

    pub async fn delete_guild(&self, guild_id: u64) {
        sqlx::query("DELETE FROM guilds WHERE guild_id == ?1").bind(i64::try_from(guild_id).unwrap()).execute(&self.conn).await.unwrap();
    }

    pub async fn update_guild(&self, id: i32, shard: &str, show: i32) {
        sqlx::query("UPDATE guilds SET shard = ?1, shard_name = ?2, show_command_output = ?3 WHERE id == ?4")
        .bind(Shard::from_str(shard).api_url())
        .bind(Shard::from_str(shard).as_str())
        .bind(show)
        .bind(id)
        .execute(&self.conn).await.unwrap();
    }

    pub async fn get_job_entry(&self, job_name: &String) -> Option<JobData> {
        sqlx::query_as("SELECT * FROM cronjobs WHERE job_name == ?1").bind(job_name).fetch_optional(&self.conn).await.unwrap()
    }

    pub async fn add_job_entry(&self, uuid: u128, guild_id: u64, job: ReportJob) {
        let guild_data = self.get_guild(guild_id).await;
        let guild = match guild_data {
            Some(g) => g,
            None => {
                return;
            }
        };
        sqlx::query("INSERT INTO cronjobs (guild, uuid, job_name, schedule, webhook_url, map_name, draw_text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
        .bind(guild.id)
        .bind(i64::try_from(uuid).unwrap())
        .bind(job.schedule_name)
        .bind(job.schedule)
        .bind(job.webhook.url().unwrap())
        .bind(job.map_name)
        .bind(job.draw_text)
        .execute(&self.conn).await.unwrap();
    }

    #[allow(unused_variables)]
    pub async fn remove_job_entry(&self, report_job: ReportJob) {
        todo!()
    }

    pub async fn update_job_entry(&self, uuid: u128, job_name: String, job: ReportJob) {
        sqlx::query("UPDATE cronjobs SET uuid= ?1, job_name = ?2, schedule = ?3, webhook_url = ?4, map_name = ?5, draw_text = ?6 WHERE job_name == ?7")
        .bind(i64::try_from(uuid).unwrap())
        .bind(job.schedule_name)
        .bind(job.schedule)
        .bind(job.webhook.url().unwrap())
        .bind(job.map_name)
        .bind(job.draw_text)
        .bind(job_name)
        .execute(&self.conn).await.unwrap();
    }
}
