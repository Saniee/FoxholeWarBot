use super::api_definitions::foxhole::{DynamicMapData, Maps, StaticMapData};

async fn create_dir() {
    let path = std::path::Path::new("./cache");

    if !path.exists() {
        tokio::fs::create_dir("./cache").await.unwrap();
    }
}

pub async fn save_maps_cache() {
    let client = reqwest::Client::new();
    let maps = client.get("https://war-service-live.foxholeservices.com/api/worldconquest/maps").send().await.unwrap().json::<Maps>().await.unwrap();

    let path = std::path::Path::new("./maps.json");

    tokio::fs::write(path, serde_json::to_string(&maps).unwrap()).await.unwrap();
}

pub async fn load_maps() -> Maps {
    let path = std::path::Path::new("./maps.json");

    let data = tokio::fs::read_to_string(path).await.unwrap();

    serde_json::from_str(&data).unwrap()
}

pub async fn save_cache(dynamic_data: DynamicMapData, static_data: StaticMapData, map_name: &str) {
    create_dir().await;
    
    match tokio::fs::write(format!("./cache/Dynamic_{}.json", map_name), serde_json::to_string(&dynamic_data).unwrap()).await {
        Ok(()) => {},
        Err(msg) => {
            println!("Error at writing dynamic data for {}. Error: {}", map_name, msg)
        }
    }
    match tokio::fs::write(format!("./cache/Static_{}.json", map_name), serde_json::to_string(&static_data).unwrap()).await {
        Ok(()) => {},
        Err(msg) => {
            println!("Error at writing static data for {}. Error: {}", map_name, msg)
        },
    };
}

pub async fn load_cache(map_name: &str) -> Option<(DynamicMapData, StaticMapData)> {
    let dynamic_data_str = match tokio::fs::read_to_string(format!("./cache/Dynamic_{}.json", map_name)).await {
        Ok(data) => Some(data),
        Err(msg) => {
            println!("Error at reading dynamic data for {}. Error: {}", map_name, msg);
            None
        }
    };

    let static_data_str = match tokio::fs::read_to_string(format!("./cache/Static_{}.json", map_name)).await {
        Ok(data) => Some(data),
        Err(msg) => {
            println!("Error at reading static data for {}. Error: {}", map_name, msg);
            None
        }
    };

    if dynamic_data_str.is_some() && static_data_str.is_some() {
        let dynamic_data = serde_json::from_str(&dynamic_data_str.unwrap()).unwrap();
        let static_data = serde_json::from_str(&static_data_str.unwrap()).unwrap();
        Some((dynamic_data, static_data))
    } else {
        None
    }
    
}