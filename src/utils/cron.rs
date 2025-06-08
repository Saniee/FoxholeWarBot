use std::str::FromStr;

use reqwest::StatusCode;
use serenity::all::{Context, CreateAttachment, CreateEmbed, CreateEmbedFooter, ExecuteWebhook, Webhook};
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};
use uuid::Uuid;
use crate::commands::schedule_report::ReportJob;

use super::{api_definitions::foxhole::{DynamicMapData, StaticMapData}, cache::{load_map_cache, save_map_cache, save_maps_cache}, db::{Database, GuildData, JobData}, request_processing::place_image_info};

#[derive(Clone)]
pub struct CronHandler {
    scheduler: JobScheduler
}

impl CronHandler {
    pub async fn new() -> Result<Self, JobSchedulerError> {
        let sched = JobScheduler::new().await?;
        sched.start().await?;

        Ok(
            CronHandler { scheduler: sched}
        )
    }

    pub async fn start_map_update_job(&self) {
        println!("Starting map updating job!");
        self.scheduler.add(Job::new_async("0 0 0 * * *", |_uuid, mut _l| {
            Box::pin({
                async move {
                    save_maps_cache().await;
                    println!("Map List Updated!");
                }
            })
        }).unwrap()).await.unwrap();
    }

    pub async fn restart_report_jobs(&mut self, ctx: &Context, db: Database) {
        println!("Restarting scheduled report jobs from the database!");
        let jobs_f = sqlx::query_as("SELECT * FROM cronjobs").fetch_all(&db.conn).await;
        let jobs: Vec<JobData> = match jobs_f {
            Ok(jobs) => jobs,
            Err(_) => return println!("Error getting jobs from database!")
        };

        if jobs.is_empty() {
            return println!("Report Jobs Database is Empty.");
        }

        let guild: GuildData = sqlx::query_as("SELECT * FROM guilds WHERE id == ?1").bind(jobs[0].guild).fetch_one(&db.conn).await.unwrap();
        for job in jobs {
            let report_job = ReportJob {
                schedule_name: job.job_name,
                schedule: job.schedule,
                webhook: serenity::all::Webhook::from_url(&ctx, &job.webhook_url.to_owned()).await.unwrap(),
                guild_id: guild.guild_id,
                map_name: job.map_name,
                draw_text: job.draw_text,
                db: db.clone(),
            };
            self.add_report_job(ctx.clone(), db.clone(), report_job, true).await;
        }
    }

    pub async fn add_report_job(&mut self, ctx: Context, db: Database,  report_job: ReportJob, from_db: bool) -> Option<bool> {
        let job = report_job.clone();

        if !from_db {
            let success = report_job.db.add_job_entry(report_job.guild_id, report_job.clone()).await;

            match success {
                Some(_) => {},
                None => return None
            }
        }

        let uuid = self.scheduler.add(Job::new_async(job.schedule, move |uuid, mut l| {
            Box::pin(
                {
                    {
                    let http = ctx.clone();
                    let url = job.webhook.url().unwrap().clone();
                    let db = job.db.clone();
                    let map_name = job.map_name.clone();
                    let job_name = job.schedule_name.clone();
                    async move {
                        let next_tick_q = l.next_tick_for_job(uuid).await;
                        let next_tick = match next_tick_q {
                            Ok(Some(ts)) => Some(ts),
                            _ => None,
                        };

                        let webhook = Webhook::from_url(http.clone(), &url).await.unwrap();
                        let guild_data = db.get_guild(job.guild_id).await;
                        let guild = match guild_data {
                            Some(data) => data,
                            None => return,
                        };
                        let api_url = guild.shard;

                        let request_client = reqwest::Client::new();

                        let draw_text_i = job.draw_text;
                        let draw_text = draw_text_i == 1;

                        let cache_data = match load_map_cache(&map_name, &guild.shard_name).await {
                            Some(data) => Some(data),
                            None => {
                                println!("No cached data found... Creating...");
                                None
                            },
                        };

                        let dynamic_response;
                        let static_response;
                        let last_updated;

                        if cache_data.is_none() {
                            dynamic_response = request_client.get(format!("{api_url}/worldconquest/maps/{}/dynamic/public", map_name)).header("If-None-Match", "\"0\"").send().await.unwrap();
                            
                            static_response = request_client.get(format!("{api_url}/worldconquest/maps/{}/static", map_name)).header("If-None-Match", "\"0\"").send().await.unwrap();
                    
                            //println!("{} | {}", dynamic_response.status(), static_response.status());
                        } else {
                            dynamic_response = request_client.get(format!("{api_url}/worldconquest/maps/{}/dynamic/public", map_name)).header("If-None-Match", format!("\"{}\"", cache_data.clone().unwrap().0.version)).send().await.unwrap();
                            
                            static_response = request_client.get(format!("{api_url}/worldconquest/maps/{}/static", map_name)).header("If-None-Match", format!("\"{}\"", cache_data.clone().unwrap().1.version)).send().await.unwrap();
                        }

                        if dynamic_response.status() == StatusCode::INTERNAL_SERVER_ERROR && static_response.status() == StatusCode::INTERNAL_SERVER_ERROR {
                            return;
                        }

                        if dynamic_response.status() != StatusCode::NOT_MODIFIED && static_response.status() != StatusCode::NOT_MODIFIED {
                            let dynamic_data = dynamic_response.json::<DynamicMapData>().await.unwrap();
                            last_updated = dynamic_data.last_updated;
                    
                            let static_data = static_response.json::<StaticMapData>().await.unwrap();
                    
                            let img = match place_image_info(&dynamic_data, &static_data, draw_text, &format!("./assets/Maps/Map{}.TGA", map_name)) {
                                Some(i) => i,
                                None => {
                                    return;
                                }
                            };
                        
                            let _ = img.save("render.png");
                            save_map_cache(dynamic_data, static_data, &map_name, &guild.shard_name).await;
                        } else {
                            last_updated = cache_data.clone().unwrap().0.last_updated;
                            let img = match place_image_info(&cache_data.clone().unwrap().0, &cache_data.clone().unwrap().1, draw_text, &format!("./assets/Maps/Map{}.TGA", map_name)) {
                                Some(i) => i,
                                None => {
                                    return;
                                }
                            };
                        
                            let _ = img.save("render.png");
                        }

                        let mut e = CreateEmbed::new().title(format!("Scheduled Report: {}", job_name)).color((0, 255, 0)).attachment("attachment://render.png").image("attachment://render.png").footer(CreateEmbedFooter::new("Requested at")).timestamp(chrono::Local::now());

                        if next_tick.is_some() {
                            e = e.description(format!("Last API Update: {}\nNext Scheduled Update: {}", chrono::DateTime::from_timestamp_millis(last_updated).unwrap().format("%Y %m %d %H:%M:%S"), next_tick.unwrap().format("%Y %m %d %H:%M:%S")));
                        } else {
                            e = e.description(format!("Last API Update: {}", chrono::DateTime::from_timestamp_millis(last_updated).unwrap().format("%Y %m %d %H:%M:%S")));
                        }

                        let builder = ExecuteWebhook::new().add_file(CreateAttachment::path("render.png").await.unwrap()).embed(e);
                        
                        webhook.execute(http, false, builder).await.unwrap();
                    }
                    }
                }
            )
        }).unwrap()).await;

        match uuid {
            Ok(id) => {
                db.update_job_uuid(report_job.schedule_name, id.to_string()).await;
            },
            Err(err) => println!("Error adding job into list: {}", err)
        };
        Some(true)
    }

    pub async fn remove_report_job(&mut self, ctx: &Context, db: Database, job_name: String, guild_data: GuildData) {
        let job = db.get_job_entry(&job_name).await.unwrap();
        let jobs = db.get_jobs_for_guild(guild_data).await;
        if jobs.len() == 1 {
            let webhook = Webhook::from_url(&ctx, &job.webhook_url).await.unwrap();
            webhook.delete(ctx).await.unwrap();
        };
        let _ = self.scheduler.remove(&Uuid::from_str(&job.job_id).unwrap()).await;
        db.remove_job_entry(job_name).await;
    }
}
