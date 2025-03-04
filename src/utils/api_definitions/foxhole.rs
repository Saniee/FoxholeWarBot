use serde::{Serialize, Deserialize};

pub type Maps = Vec<String>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct War {
    pub war_id: String,
    pub war_number: i64,
    pub winner: String,
    pub conquest_start_time: i64,
    pub conquest_end_time: Option<serde_json::Value>,
    pub resistance_start_time: Option<serde_json::Value>,
    pub required_victory_towns: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarReport {
    pub total_enlistments: i64,
    pub colonial_casualties: i64,
    pub warden_casualties: i64,
    pub day_of_war: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicMapData {
    pub region_id: i64,
    pub scorched_victory_towns: i64,
    pub map_items: Vec<MapItem>,
    pub last_updated: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapItem {
    pub team_id: TeamId,
    pub icon_type: i64,
    pub x: f64,
    pub y: f64,
    pub flags: i64,
    pub view_direction: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, std::cmp::PartialEq)]
pub enum TeamId {
    #[serde(rename = "COLONIALS")]
    Colonials,
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "WARDENS")]
    Wardens,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticMapData {
    pub region_id: i64,
    pub scorched_victory_towns: i64,
    pub map_text_items: Vec<MapTextItem>,
    pub last_updated: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapTextItem {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub map_marker_type: MapMarkerType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MapMarkerType {
    Major,
    Minor,
}