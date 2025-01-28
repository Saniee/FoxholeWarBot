use ab_glyph::{FontRef, PxScale};
use image::{imageops::overlay, ImageBuffer, Rgba};
use imageproc::drawing::draw_text_mut;

use crate::utils::api_definitions::foxhole::{DynamicMapData, StaticMapData};

pub fn place_image_info<P>(dynamic_data: &DynamicMapData, static_data: &StaticMapData, draw_text: bool, background_img_path: &P)
-> Option<ImageBuffer<Rgba<u8>, Vec<u8>>> 
where 
    P: AsRef<std::path::Path>
{
    let mut bg_img = image::open(background_img_path).unwrap().to_rgba8();
    let bg_width: f64 = bg_img.width().into();
    let bg_height: f64 = bg_img.height().into();

    // Dynamic Data processing.
    for dynamic_map_item in dynamic_data.map_items.clone() {
        let path = format!("./assets/MapIcons/{}{:?}.png", dynamic_map_item.icon_type, dynamic_map_item.team_id);
        let icon_path = std::path::Path::new(&path);
        
        if !icon_path.exists() {
            println!("Icon path: {:?} doesn't exist!", icon_path);
            println!("Using debug icon as fallback for missing icon...");
            let mut icon = load_img("./assets/MapIcons/DebugIcon.png").unwrap();

            let icon_desired_width: u32 = (icon.width() as f64 * 0.5) as u32;
            let icon_desired_height: u32 = (icon.height() as f64 * 0.5) as u32;

            icon = image::imageops::resize(&icon, icon_desired_width, icon_desired_height, image::imageops::FilterType::Lanczos3);
            
            overlay(&mut bg_img, &icon, (dynamic_map_item.x * bg_width) as i64, (dynamic_map_item.y * bg_height) as i64);
        } else {
            let mut icon = match load_img(icon_path) {
                Some(i) => i,
                None => {
                    println!("Error loading image: {:?}", icon_path);
                    return None;
                }
            };
            
            let icon_desired_width: u32 = (icon.width() as f64 * 0.5) as u32;
            let icon_desired_height: u32 = (icon.height() as f64 * 0.5) as u32;
            
            icon = image::imageops::resize(&icon, icon_desired_width, icon_desired_height, image::imageops::FilterType::Lanczos3);
            
            overlay(&mut bg_img, &icon, (dynamic_map_item.x * bg_width) as i64, (dynamic_map_item.y * bg_height) as i64);
        }
    }

    // Static Data Processing.
    if draw_text {
        let font_data = include_bytes!("../../assets/Inter-Bold.ttf") as &[u8];
        let font = FontRef::try_from_slice(font_data).unwrap();
        let scale = PxScale {
            x: 25.0,
            y: 25.0
        };
        for map_text in static_data.map_text_items.clone() {
            draw_text_mut(&mut bg_img, Rgba([0, 0, 0, 255]), (map_text.x * bg_width) as i32, (map_text.y * bg_height) as i32, scale, &font, &map_text.text);
        }
    }

    Some(bg_img)
}

pub fn load_img<P>(path: &P) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>>
where 
    P: AsRef<std::path::Path> + std::fmt::Debug + ?Sized
{
    let sb_img = match image::open(path) {
        Ok(img) => img,
        Err(msg) => {
            println!("An error occured at loading img with path: {:?}, Error Msg: {}", path, msg);
            return None;
        }
    };

    Some(sb_img.to_rgba8())
}