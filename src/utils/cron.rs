use reqwest::StatusCode;
use serenity::all::{Context, CreateAttachment, CreateEmbed, CreateEmbedFooter, ExecuteWebhook, Webhook};
use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};
use uuid::Uuid;
use crate::commands::schedule_report::ReportJob;

use super::{api_definitions::foxhole::{DynamicMapData, StaticMapData}, cache::{load_map_cache, save_map_cache, save_maps_cache}, db::{Database, GuildData, JobData}, request_processing::place_image_info};

#[derive(Debug, Clone)]
pub struct Report {
    uuid: Uuid,
    name: String,
}

#[derive(Clone)]
pub struct CronHandler {
    scheduler: JobScheduler,
    uuid_list: Vec<Report>
}

impl CronHandler {
    pub async fn new() -> Result<Self, JobSchedulerError> {
        let sched = JobScheduler::new().await?;
        sched.start().await?;
        
        Ok(
            CronHandler { scheduler: sched, uuid_list: Vec::new() }
        )
    }

    pub fn list_all_jobs(&mut self) -> Vec<Report> {
        println!("Current active Reports: {:?}", self.uuid_list.len());
        self.uuid_list.clone()
    }

    pub async fn start_map_update_job(&self) {
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
        let jobs_f = sqlx::query_as("SELECT * FROM cronjobs").fetch_all(&db.conn).await;
        let jobs: Vec<JobData> = match jobs_f {
            Ok(jobs) => jobs,
            Err(_) => return
        };

        if jobs.is_empty() {
            println!("Report Jobs Database is Empty.");
            return;
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
            self.add_report_job(ctx.clone(), report_job).await;
        }
    }

    pub async fn add_report_job(&mut self, ctx: Context,  report_job: ReportJob) -> Option<Vec<Report>> {
        let job = report_job.clone();

        let success = report_job.db.add_job_entry(report_job.guild_id, report_job.clone()).await;

        match success {
            Some(_) => {},
            None => return None,
        }

        let uuid = self.scheduler.add(Job::new_async(job.schedule, move |uuid, mut l| {
            Box::pin(
                {
                    {
                    let http = ctx.clone();
                    let url = job.webhook.url().unwrap().clone();
                    let db = job.db.clone();
                    let map_name = job.map_name.clone();
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

                        let mut e = CreateEmbed::new().color((0, 255, 0)).attachment("attachment://render.png").image("attachment://render.png").footer(CreateEmbedFooter::new("Requested at")).timestamp(chrono::Local::now());

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
        }).unwrap()).await.unwrap();
        
        /* let job_data = report_job.db.get_job_entry(&report_job.schedule_name).await;
        match job_data {
            Some(_) => {},
            None => {
                
            }
        } */

        self.uuid_list.push(Report { uuid, name: report_job.clone().schedule_name });
        Some(self.list_all_jobs())
    }
}