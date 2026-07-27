//! Mapping from Foxhole API region identifiers (`*Hex`) to the names the game
//! actually displays.
//!
//! The old autocomplete derived the label with `name.replace("Hex", "")`, which
//! is wrong for a good third of the map: `MooringCountyHex` is "The Moors",
//! `DeadLandsHex` is "Deadlands", `OarbreakerHex` is "Oarbreaker Isles". An
//! explicit table is the only thing that gets these right. See
//! `specs/active/full-map-renderer.md` for the derivation (and the grid
//! coordinates, which the full-map renderer will need).

/// All 53 world-conquest regions, `(api_name, display_name)`.
pub const REGIONS: &[(&str, &str)] = &[
    ("OlavisWakeHex", "Olavis Wake"),
    ("PariPeakHex", "Pari Peak"),
    ("PalantineBermHex", "Palantine Berm"),
    ("OarbreakerHex", "Oarbreaker Isles"),
    ("KuuraStrandHex", "Kuura Strand"),
    ("GutterHex", "The Gutter"),
    ("FishermansRowHex", "Fishermans Row"),
    ("StemaLandingHex", "Stema Landing"),
    ("NevishLineHex", "Nevish Line"),
    ("FarranacCoastHex", "Farranac Coast"),
    ("WestgateHex", "Westgate"),
    ("OriginHex", "Origin"),
    ("CallumsCapeHex", "Callums Cape"),
    ("StonecradleHex", "Stonecradle"),
    ("KingsCageHex", "Kings Cage"),
    ("SableportHex", "Sableport"),
    ("AshFieldsHex", "Ash Fields"),
    ("SpeakingWoodsHex", "Speaking Woods"),
    ("MooringCountyHex", "The Moors"),
    ("LinnMercyHex", "The Linn of Mercy"),
    ("LochMorHex", "Loch Mór"),
    ("HeartlandsHex", "The Heartlands"),
    ("RedRiverHex", "Red River"),
    ("BasinSionnachHex", "Basin Sionnach"),
    ("ReachingTrailHex", "Reaching Trail"),
    ("CallahansPassageHex", "Callahans Passage"),
    ("DeadLandsHex", "Deadlands"),
    ("UmbralWildwoodHex", "Umbral Wildwood"),
    ("GreatMarchHex", "Great March"),
    ("KalokaiHex", "Kalokai"),
    ("HowlCountyHex", "Howl County"),
    ("ViperPitHex", "Viper Pit"),
    ("MarbanHollowHex", "Marban Hollow"),
    ("DrownedValeHex", "The Drowned Vale"),
    ("ShackledChasmHex", "Shackled Chasm"),
    ("AcrithiaHex", "Acrithia"),
    ("ClansheadValleyHex", "Clanshead Valley"),
    ("WeatheredExpanseHex", "Weathered Expanse"),
    ("ClahstraHex", "The Clahstra"),
    ("AllodsBightHex", "Allods Bight"),
    ("TerminusHex", "Terminus"),
    ("MorgensCrossingHex", "Morgens Crossing"),
    ("StlicanShelfHex", "Stlican Shelf"),
    ("EndlessShoreHex", "Endless Shore"),
    ("ReaversPassHex", "Reavers Pass"),
    ("GodcroftsHex", "Godcrofts"),
    ("TempestIslandHex", "Tempest Island"),
    ("WrestaHex", "Wresta"),
    ("OnyxHex", "Onyx"),
    ("LykosIsleHex", "Lykos Isle"),
    ("TheFingersHex", "The Fingers"),
    ("TyrantFoothillsHex", "Tyrant Foothills"),
    ("PipersEnclaveHex", "Pipers Enclave"),
];

/// The display name for an API region identifier.
///
/// Unknown identifiers — a region Siege Camp ships before we update the table —
/// fall back to de-camel-casing, which is wrong-ish but readable, and never
/// hides the region from users.
pub fn display_name(api_name: &str) -> String {
    if let Some((_, display)) = REGIONS.iter().find(|(api, _)| *api == api_name) {
        return (*display).to_string();
    }

    split_camel_case(api_name.strip_suffix("Hex").unwrap_or(api_name))
}

fn split_camel_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);

    for (i, ch) in s.char_indices() {
        if i > 0 && ch.is_uppercase() {
            out.push(' ');
        }
        out.push(ch);
    }

    out
}
