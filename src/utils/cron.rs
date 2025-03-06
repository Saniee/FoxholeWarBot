#![allow(dead_code, unused_variables)]

use sqlx::prelude::FromRow;
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};

use super::db::Database;

#[derive(Clone)]
pub struct CronHandler {
    scheduler: JobScheduler
}

#[derive(Debug, FromRow)]
struct JobData {
    pub id: i32,
    pub job_name: String,
    pub schedule: String,
    pub channel_id: i64,
    pub map_name: String,
    pub draw_text: i32
}

impl CronHandler {
    pub async fn new() -> Result<Self, JobSchedulerError> {
        let sched = JobScheduler::new().await?;

        sched.shutdown_on_ctrl_c();
        
        Ok(
            CronHandler { scheduler: sched }
        )
    }

    pub async fn restart_jobs(&self, db: Database) {
        let jobs: Vec<JobData> = sqlx::query_as("SELECT * FROM cronjobs").fetch_all(&db.conn).await.unwrap();

        for job in jobs  {
            self.add_job(&job.job_name, &job.schedule, &job.map_name, job.draw_text, db.clone()).await;
        }
    }

    pub async fn add_job(&self, job_name: &str, schedule: &str, map_name: &str, draw_text: i32, db: Database) {
        self.scheduler.add(Job::new_async(schedule, |_uuid, mut _l| {
            Box::pin(
                {
                    async move {

                    }
                }
            )
        }).unwrap()).await.unwrap();
    }
}