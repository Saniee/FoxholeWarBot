// For now.
#![allow(unused)]

use ftail::Ftail;
use image_processing::place_info;
use log::{debug, error};
use api_definitions::foxhole::{StaticMapData, DynamicMapData};
use std::env;

mod api_definitions;
mod image_processing;

#[tokio::main]
async fn main() {
    Ftail::new().console(log::LevelFilter::Debug).single_file("error.log", false, log::LevelFilter::Error).init().unwrap();
    let args: Vec<String> = env::args().collect();

    // debug!("Hello World!");

    let client = reqwest::Client::new();

    let map_name = &args[1];

    let dynamic_res = client.get(format!("https://war-service-live.foxholeservices.com/api/worldconquest/maps/{}/dynamic/public", map_name)).send().await;
    
    let static_res = client.get(format!("https://war-service-live.foxholeservices.com/api/worldconquest/maps/{}/static", map_name)).send().await;

    let dynamic_data = match dynamic_res {
        Ok(response) => response.json::<DynamicMapData>().await.unwrap(),
        Err(msg) => {
            error!("An error occured at the dynamic map data request: {}", msg);
            return;
        }
    };

    let static_data = match static_res {
        Ok(response) => response.json::<StaticMapData>().await.unwrap(),
        Err(msg) => {
            error!("An error occured at the static map data request: {}", msg);
            return;
        }
    };

    // debug!("Dynamic: {:?}\nStatic: {:?}", dynamic_data.map_items, static_data.map_items);
    let img = match place_info(dynamic_data, static_data, true, &format!("./assets/Maps/Map{}.TGA", map_name)) {
        Some(i) => i,
        None => {
            return;
        }
    };

    img.save("final.png");
}
