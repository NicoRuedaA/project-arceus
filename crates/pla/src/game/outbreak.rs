//! Mass outbreaks (Brief 39): regular outbreaks from decoded data, massive ones blocked.
//!
//! Regular outbreaks (`data/world_events_outbreak_areas.tsv`, `..._outbreak_spawners.tsv`):
//! - every area 01..05 has `outbreak_count` outbreak ids (46/44/44/48/45) and a story unlock gate
//!   (VERIFIED links);
//! - for each id exactly one spawner is gated `WSYS_MASS_GANERATION_AREAnn == id` (the outbreak
//!   spawner: one species, 4 alive) and up to four regular spawners are gated `!= id` (switched off
//!   during it). The disabled set is rebuilt here from `data/field_spawn_table.tsv` (every row
//!   gated `WSYS_MASS_GANERATION_AREAnn=id` that is not the outbreak spawner) and checked against
//!   the sheet's `disabled_spawners` count: 227/227 match (test below);
//! - outside their outbreak, the 227 outbreak spawners are OFF (they are in the regular spawn
//!   table, which the viewer otherwise loads unconditionally);
//! - ASSUMED: one outbreak per unlocked area per game day, id drawn uniformly from the session
//!   seed and the day; it ends after `total_min..=total_max` (10..15, ASSUMED meaning) Pokémon are
//!   defeated or caught, or when the day changes. How the game draws ids is UNKNOWN.
//!
//! Massive mass outbreaks: the per-area settings and the 166 groups are decoded, but none of the
//! encounter keys the groups use exists in any decoded table (the key names are built at run time
//! in code that no local file reaches), so the **group to species mapping is UNKNOWN**.
//! [`massive_outbreak_report`] keeps the proof. The runtime instead uses:
//! - the decoded per-area settings (`data/world_events_mmo_areas.tsv`: wave sizes 8..10 and 6..7,
//!   `f14`/`f17` = 12 extra rare tries) — VERIFIED values, PROBABLE meaning (external notes);
//! - the per-zone species pools of the external player notes ([`MMO_SPECIES_POOLS`]), labelled
//!   PROBABLE, split into base forms (names that the decoded outbreak spawners of the zone also
//!   carry, VERIFIED subset) and evolution-stage extras (notes only);
//! - the second-wave outcome split (765/120/85/30 per mille) and the second-wave alpha chances
//!   (10 % base / 9 % evolution), both PROBABLE (external notes; the decoded group child weights
//!   81/10/9 carry the same 10/9 pair).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use bevy::math::Vec3;

use super::distortion::{EventSpawnSlot, SpawnerRole};
use super::wild::{hash_str, splitmix64};
use super::world_events::{parse_slot, rng_range, EventSlot, Sheet, StoryGate};

/// One area row of `world_events_outbreak_areas.tsv`.
#[derive(Debug, Clone, PartialEq)]
pub struct OutbreakArea {
    pub area: String,
    pub work: String,
    pub gate: StoryGate,
    pub outbreak_count: u16,
    pub total_min: u16,
    pub total_max: u16,
    /// Unexplained raw values (20 and 25 in every row).
    pub f1_raw: u32,
    pub f7_raw: u32,
}

impl OutbreakArea {
    /// Extra rare tries of a standard mass outbreak (`f7_raw` = 25 in every area; VERIFIED
    /// value, PROBABLE meaning from the external notes).
    pub fn rare_bonus_tries(&self) -> u32 {
        self.f7_raw
    }

    /// Activation chance per map transition in per cent (`f1_raw` = 20 in every area; VERIFIED
    /// value, PROBABLE meaning from the external notes).
    pub fn activation_percent(&self) -> u32 {
        self.f1_raw
    }
}

/// The single spawner of one outbreak id.
#[derive(Debug, Clone, PartialEq)]
pub struct OutbreakSpawner {
    pub spawner_id: String,
    pub pos: Vec3,
    pub radius: f32,
    pub min_count: u16,
    pub max_count: u16,
    pub slots: Vec<EventSlot>,
    /// The sheet's `disabled_spawners` count (for the cross-check).
    pub disabled_count: u16,
}

#[derive(Debug, Clone, Default)]
pub struct OutbreakData {
    pub areas: BTreeMap<String, OutbreakArea>,
    /// `(area, outbreak id)`.
    pub spawners: HashMap<(String, u16), OutbreakSpawner>,
    /// `(area, outbreak id)` -> regular spawner ids switched off during it.
    pub disabled: HashMap<(String, u16), BTreeSet<String>>,
}

impl OutbreakData {
    /// `spawn_table` is `data/field_spawn_table.tsv` (only its `gate_flags` column is used).
    pub fn parse(areas: &str, spawners: &str, spawn_table: &str) -> Self {
        let mut d = Self::default();
        let s = Sheet::parse(areas);
        for r in 0..s.len() {
            let Ok(count) = s.get(r, "outbreak_count").parse::<u16>() else { continue };
            let area = s.get(r, "area").to_string();
            d.areas.insert(
                area.clone(),
                OutbreakArea {
                    area,
                    work: s.get(r, "work").to_string(),
                    gate: StoryGate::new(s.get(r, "story_cmp"), s.get(r, "story_value"), ""),
                    outbreak_count: count,
                    total_min: s.get(r, "total_min").parse().unwrap_or(10),
                    total_max: s.get(r, "total_max").parse().unwrap_or(15),
                    f1_raw: s.get(r, "f1_raw").parse().unwrap_or(0),
                    f7_raw: s.get(r, "f7_raw").parse().unwrap_or(0),
                },
            );
        }
        let s = Sheet::parse(spawners);
        for r in 0..s.len() {
            let Ok(id) = s.get(r, "outbreak_id").parse::<u16>() else { continue };
            let f = |n: &str| s.get(r, n).parse::<f32>().ok().filter(|v| v.is_finite());
            let (Some(x), Some(y), Some(z)) = (f("x"), f("y"), f("z")) else { continue };
            let Some(slot) = parse_slot(&s, r) else { continue };
            let min_count = s.get(r, "min_count").parse().unwrap_or(4);
            let entry = d.spawners.entry((s.get(r, "area").to_string(), id)).or_insert_with(|| OutbreakSpawner {
                spawner_id: s.get(r, "spawner_id").to_string(),
                pos: Vec3::new(x, y, z),
                radius: f("radius").unwrap_or(7.0),
                min_count,
                max_count: s.get(r, "max_count").parse().unwrap_or(min_count).max(min_count),
                slots: Vec::new(),
                disabled_count: s.get(r, "disabled_spawners").parse().unwrap_or(0),
            });
            entry.slots.push(slot);
        }
        // Regular spawners gated on the outbreak work: `WSYS_MASS_GANERATION_AREAnn=<id>`.
        let s = Sheet::parse(spawn_table);
        for r in 0..s.len() {
            let gate = s.get(r, "gate_flags");
            let Some((work, value)) = gate.split_once('=') else { continue };
            let Some(area) = d.areas.values().find(|a| a.work == work).map(|a| a.area.clone()) else { continue };
            let Ok(id) = value.parse::<u16>() else { continue };
            let sid = s.get(r, "spawner_id");
            if d.spawners.get(&(area.clone(), id)).is_some_and(|o| o.spawner_id == sid) {
                continue;
            }
            d.disabled.entry((area, id)).or_default().insert(sid.to_string());
        }
        d
    }

    /// Spawner ids of every outbreak of `area`: they stay off unless their outbreak is active.
    pub fn outbreak_spawner_ids(&self, area: &str) -> BTreeSet<String> {
        self.spawners.iter().filter(|((a, _), _)| a == area).map(|(_, o)| o.spawner_id.clone()).collect()
    }

    /// Whether outbreaks are unlocked in `area` at `story`.
    pub fn unlocked(&self, area: &str, story: u32) -> bool {
        self.areas.get(area).is_some_and(|a| a.gate.matches(story))
    }

    /// ASSUMED draw: the outbreak id of `area` on game `day` (uniform over `1..=outbreak_count`),
    /// `None` when outbreaks are locked or the area has none.
    pub fn draw(&self, area: &str, story: u32, day: u32, seed: u64) -> Option<u16> {
        let a = self.areas.get(area)?;
        if !a.gate.matches(story) || a.outbreak_count == 0 {
            return None;
        }
        let mut rng = seed ^ hash_str(area) ^ (u64::from(day) << 32 | 0x0b5e);
        let id = 1 + (splitmix64(&mut rng) % u64::from(a.outbreak_count)) as u16;
        self.spawners.contains_key(&(area.to_string(), id)).then_some(id)
    }

    /// How many Pokémon this outbreak holds in total (ASSUMED `total_min..=total_max`).
    pub fn roll_total(&self, area: &str, id: u16, day: u32, seed: u64) -> u16 {
        let Some(a) = self.areas.get(area) else { return 0 };
        let mut rng = seed ^ hash_str(area) ^ u64::from(id) ^ (u64::from(day) << 40);
        rng_range(&mut rng, u32::from(a.total_min), u32::from(a.total_max.max(a.total_min))) as u16
    }

    /// The alive slots of outbreak `id`: `max_count` (4) slots around the spawner centre,
    /// spread inside its radius (spread ASSUMED; centre and radius are decoded).
    pub fn spawn_slots(&self, area: &str, id: u16, seed: u64) -> Vec<EventSpawnSlot> {
        let Some(o) = self.spawners.get(&(area.to_string(), id)) else { return Vec::new() };
        let mut rng = seed ^ hash_str(&o.spawner_id) ^ u64::from(id);
        (0..o.max_count.max(1))
            .map(|k| {
                let ang = (splitmix64(&mut rng) % 3600) as f32 / 3600.0 * std::f32::consts::TAU;
                let r = o.radius.max(1.0) * ((splitmix64(&mut rng) % 1000) as f32 / 1000.0).sqrt();
                EventSpawnSlot {
                    key: format!("ob:{area}:{id}:{k}"),
                    pos: o.pos + Vec3::new(ang.cos() * r, 0.0, ang.sin() * r),
                    radius: o.radius,
                    slots: o.slots.clone(),
                    role: SpawnerRole::Main,
                }
            })
            .collect()
    }

    /// Main species name of an outbreak (its non-alpha slot) for logs.
    pub fn species(&self, area: &str, id: u16) -> Option<&str> {
        self.spawners
            .get(&(area.to_string(), id))
            .and_then(|o| o.slots.iter().find(|s| !s.alpha).or(o.slots.first()))
            .map(|s| s.species.as_str())
    }
}

/// Result of resolving the massive-outbreak groups against every decoded encounter table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MassiveOutbreakReport {
    pub groups: usize,
    pub keys: usize,
    pub resolved_keys: usize,
    /// Groups with at least one resolved key (the only ones that could ever spawn).
    pub usable_groups: usize,
}

/// `groups` is `world_events_mmo_groups.tsv`; `known_keys` every encounter key of the decoded
/// tables (regular spawns, distortions, outbreaks).
pub fn massive_outbreak_report(groups: &str, known_keys: &BTreeSet<String>) -> MassiveOutbreakReport {
    let s = Sheet::parse(groups);
    let mut by_group: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for r in 0..s.len() {
        by_group.entry(s.get(r, "group_key")).or_default().push(s.get(r, "encount_key"));
    }
    let keys: BTreeSet<&str> = by_group.values().flatten().copied().collect();
    MassiveOutbreakReport {
        groups: by_group.len(),
        keys: keys.len(),
        resolved_keys: keys.iter().filter(|k| known_keys.contains(**k)).count(),
        usable_groups: by_group.values().filter(|ks| ks.iter().any(|k| known_keys.contains(*k))).count(),
    }
}

/// Every `encount_key` column value of a sheet (helper for [`massive_outbreak_report`]).
pub fn encount_keys(text: &str) -> BTreeSet<String> {
    let s = Sheet::parse(text);
    (0..s.len()).map(|r| s.get(r, "encount_key").to_string()).filter(|k| !k.is_empty()).collect()
}

// ------------------------------------------------- massive mass outbreaks (MMO) data

/// `--outbreak mmo`: force a massive mass outbreak in the start area.
pub const MMO_SENTINEL: u16 = u16::MAX;

/// Extra rare tries an event spawner id carries: `we:ob:` (standard outbreak) = 25 (`f7_raw`,
/// decoded), `we:mmo` (massive outbreak) = 12 (`f14`/`f17`, decoded), anything else = 0.
/// The meaning of both numbers is PROBABLE (external notes).
pub fn event_rare_bonus_tries(spawner_id: &str) -> u32 {
    if spawner_id.starts_with("we:ob") {
        super::rare_palette::RARE_TRY_BONUS_OUTBREAK
    } else if spawner_id.starts_with("we:mmo") {
        super::rare_palette::RARE_TRY_BONUS_MMO
    } else {
        0
    }
}

/// One area of `data/world_events_mmo_areas.tsv` (columns `f02_raw`..`f17_raw`).
///
/// VERIFIED values: `wave1` 8..10 (`f07`/`f08`), `wave2` 6..7 (`f09`/`f10`), `f14` and `f17` = 12.
/// PROBABLE meaning (external notes): first wave size, second wave size, +12 extra rare tries.
/// The external note says the second wave holds 6-8; the decoded pair is 6..7, so the port keeps
/// the decoded 6..7.
#[derive(Debug, Clone, PartialEq)]
pub struct MmoArea {
    pub area: String,
    pub open_flag: String,
    pub wave1_min: u16,
    pub wave1_max: u16,
    pub wave2_min: u16,
    pub wave2_max: u16,
    pub rare_bonus_tries: u32,
}

impl MmoArea {
    pub fn parse(text: &str) -> BTreeMap<String, MmoArea> {
        let s = Sheet::parse(text);
        let u = |r: usize, n: &str| s.get(r, n).parse::<u32>().unwrap_or(0);
        (0..s.len())
            .map(|r| {
                let area = s.get(r, "area").to_string();
                let wave1_min = u(r, "f07_raw") as u16;
                let wave1_max = u(r, "f08_raw") as u16;
                let wave2_min = u(r, "f09_raw") as u16;
                let wave2_max = u(r, "f10_raw") as u16;
                (
                    area.clone(),
                    MmoArea {
                        area,
                        open_flag: s.get(r, "open_flag").to_string(),
                        wave1_min,
                        wave1_max: wave1_max.max(wave1_min),
                        wave2_min,
                        wave2_max: wave2_max.max(wave2_min),
                        rare_bonus_tries: u(r, "f14_raw"),
                    },
                )
            })
            .collect()
    }
}

/// Per-zone massive-outbreak species pool.
///
/// **PROBABLE: external player notes only** (they are not shipped with the repository; the report
/// cites the figures, not the document). The note's list for every zone is split by
/// [`mmo_pool_split`] against the decoded outbreak spawners of the zone:
/// - names that the decoded spawners also carry are the zone's **base forms** (VERIFIED subset);
/// - the rest are **evolution-stage extras** the notes attribute to the second waves.
///
/// The decoded massive-outbreak group keys never resolved against any encounter table, so this
/// per-zone pool is the runtime's only species source and stays labelled PROBABLE.
pub const MMO_SPECIES_POOLS: &[(&str, &[&str])] = &[
    (
        "ha_area01",
        &[
            "pikachu", "raichu", "pichu", "zubat", "golbat", "crobat", "paras", "parasect",
            "psyduck", "golduck", "abra", "kadabra", "alakazam", "geodude", "graveler", "golem",
            "ponyta", "rapidash", "mr-mime", "mime-jr", "scyther", "magikarp", "gyarados",
            "eevee", "snorlax", "munchlax", "qwilfish", "stantler", "wurmple", "silcoon",
            "beautifly", "cascoon", "dustox", "spheal", "sealeo", "chimchar", "monferno",
            "infernape", "starly", "staravia", "staraptor", "bidoof", "bibarel", "kricketot",
            "kricketune", "shinx", "luxio", "luxray", "combee", "buizel", "floatzel", "cherubi",
            "shellos", "gastrodon", "drifloon", "drifblim", "buneary", "lopunny", "happiny",
            "chansey",
        ],
    ),
    (
        "ha_area02",
        &[
            "paras", "parasect", "psyduck", "golduck", "geodude", "graveler", "gastly",
            "haunter", "gengar", "onix", "steelix", "lickitung", "lickilicky", "rhyhorn",
            "rhydon", "rhyperior", "chansey", "happiny", "tangela", "tangrowth", "cyndaquil",
            "quilava", "typhlosion", "togepi", "sudowoodo", "bonsly", "yanma", "yanmega",
            "murkrow", "honchkrow", "unown", "teddiursa", "ursaring", "ursaluna", "swinub",
            "piloswine", "roselia", "budew", "roserade", "barboach", "whiscash", "turtwig",
            "grotle", "torterra", "kricketot", "kricketune", "combee", "pachirisu", "stunky",
            "skuntank", "bronzor", "bronzong", "spiritomb", "hippopotas", "hippowdon", "skorupi",
            "drapion", "croagunk", "toxicroak", "carnivine", "petilil", "lilligant", "goomy",
            "sliggoo", "goodra",
        ],
    ),
    (
        "ha_area03",
        &[
            "vulpix", "growlithe", "machop", "machoke", "tentacool", "tentacruel", "chansey",
            "happiny", "tangela", "tangrowth", "magmar", "magby", "togepi", "aipom", "ambipom",
            "qwilfish", "remoraid", "octillery", "mantine", "mantyke", "duskull", "dusclops",
            "spheal", "sealeo", "walrein", "piplup", "prinplup", "empoleon", "starly", "staravia",
            "staraptor", "buizel", "floatzel", "shellos", "gastrodon", "drifloon", "drifblim",
            "glameow", "purugly", "chatot", "skorupi", "drapion", "finneon", "lumineon",
            "basculin",
        ],
    ),
    (
        "ha_area04",
        &[
            "clefairy", "clefable", "cleffa", "zubat", "golbat", "geodude", "graveler", "golem",
            "ponyta", "rapidash", "gastly", "haunter", "gengar", "onix", "voltorb", "electrode",
            "rhyhorn", "rhydon", "rhyperior", "chansey", "happiny", "mr-mime", "mime-jr",
            "scyther", "electabuzz", "elekid", "electivire", "magikarp", "gyarados", "sudowoodo",
            "bonsly", "yanma", "yanmega", "misdreavus", "mismagius", "gligar", "gliscor",
            "heracross", "sneasel", "sneasler", "teddiursa", "ursaring", "nosepass", "probopass",
            "roselia", "budew", "duskull", "dusclops", "dusknoir", "chimecho", "chingling",
            "shinx", "luxio", "luxray", "stunky", "skuntank", "bronzor", "bronzong", "gible",
            "gabite", "garchomp", "hippopotas", "hippowdon", "snover", "abomasnow", "croagunk",
            "toxicroak", "rotom", "goomy", "sliggoo", "rowlet", "dartrix", "decidueye",
        ],
    ),
    (
        "ha_area05",
        &[
            "vulpix", "ninetales", "abra", "kadabra", "alakazam", "machop", "machoke", "machamp",
            "gastly", "haunter", "gengar", "lickitung", "lickilicky", "chansey", "happiny",
            "magmar", "magby", "eevee", "aipom", "ambipom", "misdreavus", "mismagius", "gligar",
            "sneasel", "swinub", "piloswine", "mamoswine", "stantler", "ralts", "kirlia",
            "gardevoir", "gallade", "duskull", "dusclops", "dusknoir", "froslass", "chimecho",
            "chingling", "snorunt", "glalie", "bidoof", "bibarel", "pachirisu", "drifloon",
            "drifblim", "buneary", "lopunny", "glameow", "purugly", "bronzor", "bronzong",
            "munchlax", "gible", "gabite", "riolu", "lucario", "snover", "abomasnow", "basculin",
            "oshawott", "dewott", "samurott", "zorua", "zoroark", "rufflet", "braviary",
            "bergmite", "avalugg",
        ],
    ),
];

/// Split a zone's PROBABLE pool against the decoded outbreak species of the zone.
/// Returns `(base, extras)` where `base` are the names the decoded spawners also carry.
pub fn mmo_pool_split(area: &str, decoded: &BTreeSet<String>) -> (Vec<&'static str>, Vec<&'static str>) {
    let pool = MMO_SPECIES_POOLS
        .iter()
        .find(|(a, _)| *a == area)
        .map(|(_, names)| *names)
        .unwrap_or(&[]);
    let mut base = Vec::new();
    let mut extras = Vec::new();
    for n in pool {
        if decoded.contains(*n) {
            base.push(*n);
        } else {
            extras.push(*n);
        }
    }
    (base, extras)
}

/// Second-wave outcome of a massive outbreak.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MmoWave2Outcome {
    /// No second wave.
    None,
    /// Standard second wave (evolutions / alphas).
    Standard,
    /// Wave marked with the Guaya (Occa) berry icon.
    Berry,
    /// Wave marked with the star icon (guaranteed second wave).
    Star,
}

/// **PROBABLE (external notes)**: second-wave outcome split, per mille (765/120/85/30).
/// The decoded files do not carry this split anywhere the generator found.
pub const MMO_WAVE2_WEIGHTS: [(MmoWave2Outcome, u32); 4] = [
    (MmoWave2Outcome::None, 765),
    (MmoWave2Outcome::Standard, 120),
    (MmoWave2Outcome::Berry, 85),
    (MmoWave2Outcome::Star, 30),
];

/// **PROBABLE (external notes)**: alpha chance of a base-form second-wave creature, per mille.
/// The decoded massive-outbreak group children carry the weights 81/10/9, the same 10/9 pair.
pub const MMO_WAVE2_ALPHA_BASE_PER_MILLE: u32 = 100;
/// **PROBABLE (external notes)**: alpha chance of an evolution second-wave creature, per mille.
pub const MMO_WAVE2_ALPHA_EVOLUTION_PER_MILLE: u32 = 90;

/// One massive-outbreak wave plan for a zone (deterministic in `seed`).
#[derive(Debug, Clone, PartialEq)]
pub struct MmoWavePlan {
    pub wave1_species: String,
    pub wave1_count: u16,
    pub wave2: Option<MmoWave2Plan>,
}

/// The second wave of an [`MmoWavePlan`].
#[derive(Debug, Clone, PartialEq)]
pub struct MmoWave2Plan {
    pub outcome: MmoWave2Outcome,
    pub species: String,
    pub count: u16,
    /// Per-mille chance that a second-wave creature is an alpha (100 base form, 90 evolution).
    pub alpha_per_mille: u32,
}

/// Plan the waves of one massive outbreak spot: wave 1 from the zone's base forms, wave 2 from
/// the note's outcome split (its species is an evolution extra when the zone has one). `None`
/// when the zone has no usable pool.
pub fn mmo_wave_plan(area: &str, settings: &MmoArea, seed: u64) -> Option<MmoWavePlan> {
    // The decoded outbreak species of the zone: read from the same sheets the runtime uses.
    let decoded = decoded_outbreak_species(area);
    let (base, extras) = mmo_pool_split(area, &decoded);
    if base.is_empty() {
        return None;
    }
    let mut rng = seed ^ hash_str(area) ^ 0x4d4d_4f00;
    let pick = |list: &[&'static str], rng: &mut u64| -> String {
        list[(splitmix64(rng) % list.len() as u64) as usize].to_string()
    };
    let wave1_species = pick(&base, &mut rng);
    let wave1_count = rng_range(
        &mut rng,
        u32::from(settings.wave1_min.max(1)),
        u32::from(settings.wave1_max.max(settings.wave1_min)),
    ) as u16;
    let total: u32 = MMO_WAVE2_WEIGHTS.iter().map(|(_, w)| w).sum();
    let mut roll = (splitmix64(&mut rng) % u64::from(total)) as u32;
    let mut outcome = MmoWave2Outcome::None;
    for (o, w) in MMO_WAVE2_WEIGHTS {
        if roll < w {
            outcome = o;
            break;
        }
        roll -= w;
    }
    let wave2 = (outcome != MmoWave2Outcome::None).then(|| {
        let (species, alpha_per_mille) = if extras.is_empty() {
            (pick(&base, &mut rng), MMO_WAVE2_ALPHA_BASE_PER_MILLE)
        } else {
            (pick(&extras, &mut rng), MMO_WAVE2_ALPHA_EVOLUTION_PER_MILLE)
        };
        MmoWave2Plan {
            outcome,
            species,
            count: rng_range(
                &mut rng,
                u32::from(settings.wave2_min.max(1)),
                u32::from(settings.wave2_max.max(settings.wave2_min)),
            ) as u16,
            alpha_per_mille,
        }
    });
    Some(MmoWavePlan {
        wave1_species,
        wave1_count,
        wave2,
    })
}

/// The species the decoded outbreak spawners of `area` carry (`world_events_outbreak_spawners`).
/// Falls back to an empty set when the sheet is unavailable.
pub fn decoded_outbreak_species(area: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(path) = super::world_objects::find_data_file("world_events_outbreak_spawners.tsv")
    else {
        return out;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return out;
    };
    let s = Sheet::parse(&text);
    for r in 0..s.len() {
        if s.get(r, "area") == area {
            let sp = s.get(r, "species").to_lowercase();
            if !sp.is_empty() {
                out.insert(sp);
            }
        }
    }
    out
}

/// National dex id of a lower-case species name (`data/pml_personal.tsv`, cached).
pub fn species_id_by_name(name: &str) -> Option<u16> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static MAP: OnceLock<HashMap<String, u16>> = OnceLock::new();
    let map = MAP.get_or_init(|| {
        let mut m = HashMap::new();
        let Some(path) = super::world_objects::find_data_file("pml_personal.tsv") else {
            return m;
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return m;
        };
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.starts_with("dex_index")) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() >= 2 {
                if let Ok(id) = f[0].parse::<u16>() {
                    m.entry(f[1].to_lowercase()).or_insert(id);
                }
            }
        }
        m
    });
    map.get(&name.to_lowercase()).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::world_objects::find_data_file;

    fn read(name: &str) -> Option<String> {
        find_data_file(name).and_then(|p| std::fs::read_to_string(p).ok())
    }

    pub(crate) fn real() -> Option<OutbreakData> {
        Some(OutbreakData::parse(
            &read("world_events_outbreak_areas.tsv")?,
            &read("world_events_outbreak_spawners.tsv")?,
            &read("field_spawn_table.tsv")?,
        ))
    }

    #[test]
    fn real_outbreak_data_and_disabled_sets_match_the_sheet() {
        let Some(d) = real() else {
            eprintln!("[skip] outbreak sheets not found");
            return;
        };
        let counts: Vec<u16> = d.areas.values().map(|a| a.outbreak_count).collect();
        assert_eq!(counts, vec![46, 44, 44, 48, 45]);
        assert_eq!(d.spawners.len(), 227, "one spawner per outbreak id");
        let mut ok = 0;
        for (k, o) in &d.spawners {
            let got = d.disabled.get(k).map_or(0, BTreeSet::len);
            assert_eq!(got, usize::from(o.disabled_count), "{k:?}");
            assert_eq!((o.min_count, o.max_count), (4, 4), "{k:?}");
            ok += 1;
        }
        assert_eq!(ok, 227, "227/227 disabled sets rebuilt from field_spawn_table");
        // Area 01, outbreak 1 = Pikachu at the decoded spawner.
        assert_eq!(d.species("ha_area01", 1), Some("pikachu"));
        assert_eq!(d.outbreak_spawner_ids("ha_area01").len(), 46);
        // Story gates: area 01 opens at 14800.
        assert!(!d.unlocked("ha_area01", 14_799));
        assert!(d.unlocked("ha_area01", 14_800));
        assert!(!d.unlocked("ha_area00", 1_000_000), "no outbreaks in the village");
    }

    #[test]
    fn draw_is_deterministic_in_range_and_gated() {
        let Some(d) = real() else {
            eprintln!("[skip] outbreak sheets not found");
            return;
        };
        let mut ids = BTreeSet::new();
        for day in 0..400 {
            let id = d.draw("ha_area02", 100_000, day, 9).expect("unlocked");
            assert!((1..=44).contains(&id));
            assert_eq!(Some(id), d.draw("ha_area02", 100_000, day, 9));
            ids.insert(id);
        }
        assert!(ids.len() > 30, "uniform draw reaches most of the 44 ids, got {}", ids.len());
        assert_eq!(d.draw("ha_area02", 0, 0, 9), None, "locked before the story gate");
        for day in 0..50 {
            let t = d.roll_total("ha_area03", 5, day, 1);
            assert!((10..=15).contains(&t));
        }
    }

    #[test]
    fn outbreak_slots_stay_inside_the_spawner_radius() {
        let Some(d) = real() else {
            eprintln!("[skip] outbreak sheets not found");
            return;
        };
        let o = &d.spawners[&("ha_area01".to_string(), 1)];
        let slots = d.spawn_slots("ha_area01", 1, 5);
        assert_eq!(slots.len(), 4);
        for s in &slots {
            assert!(s.pos.distance(o.pos) <= o.radius + 1e-3);
            let e = s.entries(1, 2);
            assert_eq!(e.len(), 1, "alpha slot skipped, one species");
            assert_eq!((e[0].species_name.as_str(), e[0].min_level, e[0].max_level), ("pikachu", 11, 13));
        }
        assert_eq!(slots, d.spawn_slots("ha_area01", 1, 5));
    }

    #[test]
    fn massive_outbreak_species_stay_unknown() {
        let (Some(groups), Some(cond), Some(dist)) = (
            read("world_events_mmo_groups.tsv"),
            read("field_spawn_conditions.tsv"),
            read("world_events_distortion_encounters.tsv"),
        ) else {
            eprintln!("[skip] massive outbreak sheets not found");
            return;
        };
        let mut known = encount_keys(&cond);
        known.extend(encount_keys(&dist));
        let r = massive_outbreak_report(&groups, &known);
        assert_eq!(r.groups, 166);
        assert_eq!(r.keys, 334);
        assert_eq!((r.resolved_keys, r.usable_groups), (0, 0), "no species may be invented");
        // A synthetic resolvable key is counted, so the check is not vacuous.
        let first = encount_keys(&groups).into_iter().next().unwrap();
        known.insert(first);
        assert_eq!(massive_outbreak_report(&groups, &known).resolved_keys, 1);
    }

    /// The regular mass-outbreak values the runtime reads: 20 (activation %), 10..15 (size),
    /// 25 (extra rare tries), all VERIFIED as values, cross-checked against the external notes.
    #[test]
    fn regular_outbreak_values_are_the_decoded_ones() {
        let Some(d) = real() else {
            eprintln!("[skip] outbreak sheets not found");
            return;
        };
        for a in d.areas.values() {
            assert_eq!(a.activation_percent(), 20);
            assert_eq!(a.rare_bonus_tries(), 25);
            assert_eq!((a.total_min, a.total_max), (10, 15));
        }
    }

    /// The massive-outbreak settings are the decoded ones and the note's two 12s live in them.
    #[test]
    fn mmo_settings_match_the_decoded_sheet() {
        let Some(text) = read("world_events_mmo_areas.tsv") else {
            eprintln!("[skip] mmo areas sheet not found");
            return;
        };
        let areas = MmoArea::parse(&text);
        assert_eq!(areas.len(), 5);
        for (area, s) in &areas {
            assert_eq!(
                s.open_flag,
                format!(
                    "FSYS_NEW_OUTBREAK_OPEN_{}",
                    area.trim_start_matches("ha_").to_uppercase()
                )
            );
            assert_eq!((s.wave1_min, s.wave1_max), (8, 10), "{area}");
            assert_eq!((s.wave2_min, s.wave2_max), (6, 7), "{area}");
            assert_eq!(s.rare_bonus_tries, 12, "{area}");
        }
    }

    /// Every name of the PROBABLE pools must resolve in `pml_personal.tsv`, the base part must
    /// be a subset of the decoded outbreak spawners, and the split must cover the whole pool.
    #[test]
    fn mmo_species_pools_resolve_and_split() {
        let mut checked = 0;
        for (area, pool) in MMO_SPECIES_POOLS {
            let decoded = decoded_outbreak_species(area);
            assert!(!decoded.is_empty(), "{area}: decoded outbreak spawners");
            let (base, extras) = mmo_pool_split(area, &decoded);
            assert_eq!(base.len() + extras.len(), pool.len(), "{area}: split covers the pool");
            assert!(base.len() > 20, "{area}: plenty of base forms match the game data");
            for n in base.iter().chain(extras.iter()) {
                assert!(
                    species_id_by_name(n).is_some(),
                    "{area}: pool name {n} not in pml_personal.tsv"
                );
                checked += 1;
            }
            assert!(base.iter().all(|n| decoded.contains(*n)), "{area}: base is the verified part");
        }
        assert!(checked > 250, "the five pools carry hundreds of names, got {checked}");
    }

    /// The wave plan is deterministic, inside the decoded sizes, and its second wave uses the
    /// evolution extras with the note's alpha chance.
    #[test]
    fn mmo_wave_plan_is_deterministic_and_bounded() {
        let Some(text) = read("world_events_mmo_areas.tsv") else {
            eprintln!("[skip] mmo areas sheet not found");
            return;
        };
        let areas = MmoArea::parse(&text);
        let s = &areas["ha_area02"];
        let mut outcomes = BTreeSet::new();
        let mut species = BTreeSet::new();
        for seed in 0..500 {
            let plan = mmo_wave_plan("ha_area02", s, seed).expect("pool");
            assert!((8..=10).contains(&plan.wave1_count), "wave 1 size");
            assert_eq!(plan, mmo_wave_plan("ha_area02", s, seed).expect("pool"));
            species.insert(plan.wave1_species.clone());
            if let Some(w2) = plan.wave2 {
                assert!((6..=7).contains(&w2.count), "wave 2 size");
                assert!(
                    w2.alpha_per_mille == MMO_WAVE2_ALPHA_BASE_PER_MILLE
                        || w2.alpha_per_mille == MMO_WAVE2_ALPHA_EVOLUTION_PER_MILLE
                );
                outcomes.insert(w2.outcome);
                species.insert(w2.species);
            } else {
                outcomes.insert(MmoWave2Outcome::None);
            }
        }
        assert!(species.len() > 5, "both waves draw several species, got {}", species.len());
        assert!(outcomes.len() >= 3, "the outcome split fires over 500 seeds: {outcomes:?}");
    }
}
