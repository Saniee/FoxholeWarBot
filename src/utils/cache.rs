use reqwest::StatusCode;

use super::api_definitions::foxhole::{DynamicMapData, Maps, StaticMapData, WarReport};
use super::db::Shard;

async fn create_cache_dirs() {
    let cache_dir = std::path::Path::new("./cache");
    let map_cache_dir = std::path::Path::new("./cache/map_choices");
    let static_cache_dir = std::path::Path::new("./cache/static");
    let dynamic_cache_dir = std::path::Path::new("./cache/dynamic");
    let war_report_dir = std::path::Path::new("./cache/war_reports");

    if !cache_dir.exists() {
        tokio::fs::create_dir(cache_dir).await.unwrap();
    }
    if !map_cache_dir.exists() {
        tokio::fs::create_dir(map_cache_dir).await.unwrap();
    }
    if !static_cache_dir.exists() {
        tokio::fs::create_dir(static_cache_dir).await.unwrap();
    }
    if !dynamic_cache_dir.exists() {
        tokio::fs::create_dir(dynamic_cache_dir).await.unwrap();
    }
    if !war_report_dir.exists() {
        tokio::fs::create_dir(war_report_dir).await.unwrap();
    }
}

pub async fn save_maps_cache() {
    create_cache_dirs().await;

    let client = reqwest::Client::new();
    let servers = Shard::list_all();

    for server in servers {
        let api_url = server.api_url();
        let shard = server.as_str();
        let resp = client.get(format!("{api_url}/worldconquest/maps")).send().await.unwrap();
        let maps = match resp.status() {
            StatusCode::OK => {
                resp.json::<Maps>().await.unwrap()
            }
            StatusCode::SERVICE_UNAVAILABLE => {
                continue;
            }
            _ => {
                println!("{:?}", resp.status());
                continue;
            }
        };

        let path_str = format!("./cache/map_choices/maps-{shard}.json");
        let path = std::path::Path::new(&path_str);

        tokio::fs::write(path, serde_json::to_string(&maps).unwrap()).await.unwrap();
    }
}

pub async fn load_maps(shard: Shard) -> Maps {
    let shard_name = shard.as_str();
    let path_str = format!("./cache/map_choices/maps-{shard_name}.json");
    let path = std::path::Path::new(&path_str);

    let data = tokio::fs::read_to_string(path).await.unwrap();

    serde_json::from_str(&data).unwrap()
}

pub async fn save_map_cache(dynamic_data: DynamicMapData, static_data: StaticMapData, map_name: &str, shard: &str) {
    create_cache_dirs().await;
    
    match tokio::fs::write(format!("./cache/dynamic/Dynamic_{}-{}.json", map_name, shard), serde_json::to_string(&dynamic_data).unwrap()).await {
        Ok(()) => {},
        Err(msg) => {
            println!("Error at writing dynamic data for {}. Error: {}", map_name, msg)
        }
    }
    match tokio::fs::write(format!("./cache/static/Static_{}-{}.json", map_name, shard), serde_json::to_string(&static_data).unwrap()).await {
        Ok(()) => {},
        Err(msg) => {
            println!("Error at writing static data for {}. Error: {}", map_name, msg)
        },
    };
}

pub async fn save_war_report(war_report: WarReport, map_name: &str, shard: &str) {
    create_cache_dirs().await;

    match tokio::fs::write(format!("./cache/war_reports/Report_{}-{}.json", map_name, shard), serde_json::to_string(&war_report).unwrap()).await {
        Ok(()) => {},
        Err(msg) => {
            println!("Error at writing war report data for {}. Error: {}", map_name, msg)
        }
    }
}

pub async fn load_map_cache(map_name: &str, shard: &str) -> Option<(DynamicMapData, StaticMapData)> {
    let dynamic_data_str = (tokio::fs::read_to_string(format!("./cache/dynamic/Dynamic_{}-{}.json", map_name, shard)).await).ok();

    let static_data_str = (tokio::fs::read_to_string(format!("./cache/static/Static_{}-{}.json", map_name,shard)).await).ok();

    if dynamic_data_str.is_some() && static_data_str.is_some() {
        let dynamic_data = serde_json::from_str(&dynamic_data_str.unwrap()).unwrap();
        let static_data = serde_json::from_str(&static_data_str.unwrap()).unwrap();
        Some((dynamic_data, static_data))
    } else {
        None
    }
    
}

pub async fn load_war_report(map_name: &str, shard: &str) -> Option<WarReport> {
    match tokio::fs::read_to_string(format!("./cache/war_reports/Report_{}-{}.json", map_name, shard)).await {
        Ok(data) => serde_json::from_str(&data).unwrap(),
        Err(_) => {
            None
        }
    }
}