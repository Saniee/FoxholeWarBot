#![allow(dead_code, unused_imports)]

use sqlx::{sqlite, Pool, Sqlite, FromRow};

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
        sqlx::query_as("SELECT * FROM foxholewarbot WHERE guild_id == ?1").bind(i64::try_from(guild_id).unwrap()).fetch_optional(&self.conn).await.unwrap()
    }

    pub async fn create_guild(&self, guild_id: u64, shard_name: Option<&str>) {
        let data = (i64::try_from(guild_id).unwrap(), Shard::from_str(shard_name.unwrap_or("Able")), 0);
        sqlx::query("INSERT INTO foxholewarbot (guild_id, shard, shard_name, show_command_output) VALUES (?1, ?2, ?3, ?4)").bind(data.0).bind(data.1.api_url()).bind(data.1.as_str()).bind(data.2).execute(&self.conn).await.unwrap();
    }

    pub async fn delete_guild(&self, guild_id: u64) {
        sqlx::query("DELETE FROM foxholewarbot WHERE guild_id == ?1").bind(i64::try_from(guild_id).unwrap()).execute(&self.conn).await.unwrap();
    }

    pub async fn update_guild(&self, id: i32, shard: &str, show: i32) {
        sqlx::query("UPDATE foxholewarbot SET shard = ?1, shard_name = ?2, show_command_output = ?3 WHERE id == ?4").bind(Shard::from_str(shard).api_url()).bind(Shard::from_str(shard).as_str()).bind(show).bind(id).execute(&self.conn).await.unwrap();
    }
}