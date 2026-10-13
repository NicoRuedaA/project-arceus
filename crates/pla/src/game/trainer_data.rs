//! Trainer parties and Noble (boss) settings read from the committed sheets.
//!
//! Inputs (decoded by `.tools/trainer_decode.py` / `.tools/trainer_boss_decode.py`, see
//! `reports/data-progress/trainers.md`):
//! - `data/trainer_index.tsv`: one row per `bin/trainer/trdata_<id>.bin` (377 files).
//! - `data/trainer_teams.tsv`: one row per party member (620 members).
//! - `data/boss_battles.tsv`: the 16 Noble settings (8 battles + 8 `_r` rematches).
//!
//! Confidence of the fields used here (from the data report): species, form, moves, party
//! vector and the AI module bytes are VERIFIED or LIKELY; the level, nature and effort slots are
//! LIKELY; reading slots 16..21 as Basic/Strong/Expert/Allowance/Item/Change is LIKELY; the
//! nature number -> stat table below is ASSUMED (main-series order; 12 is the neutral value
//! most members carry, which fits); `prize_base` as a prize base is ASSUMED. Party order is
//! the stored order; whether it is the send-out order is UNKNOWN.
//!
//! Pure module: no Bevy, no game files; only the committed sheets.

use std::collections::BTreeMap;
use std::path::Path;

use crate::battle::{BattleData, BattleMove};
use crate::battle_engine::progression::{initial_moves, ProgressionTables};
use crate::battle_engine::Combatant;

/// One sheet loaded as header-name -> value maps (`#` lines skipped, types stripped).
struct Sheet {
    rows: Vec<BTreeMap<String, String>>,
}

impl Sheet {
    fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self::parse(&text))
    }

    fn parse(text: &str) -> Self {
        let mut lines = text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty());
        let cols: Vec<String> = lines
            .next()
            .map(|h| h.split('\t').map(|c| c.split(':').next().unwrap_or("").to_string()).collect())
            .unwrap_or_default();
        let rows = lines
            .map(|l| cols.iter().cloned().zip(l.split('\t').map(str::to_string)).collect())
            .collect();
        Self { rows }
    }
}

fn text(row: &BTreeMap<String, String>, col: &str) -> String {
    row.get(col).cloned().unwrap_or_default()
}

fn opt_text(row: &BTreeMap<String, String>, col: &str) -> Option<String> {
    row.get(col).filter(|v| !v.is_empty()).cloned()
}

fn num<T: std::str::FromStr>(row: &BTreeMap<String, String>, col: &str) -> Option<T> {
    row.get(col).and_then(|v| v.trim().parse().ok())
}

fn flag(row: &BTreeMap<String, String>, col: &str) -> bool {
    row.get(col).is_some_and(|v| v == "1")
}

/// The six AI module bools of a trainer (root slots 16..21; mapping LIKELY).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrainerAi {
    pub basic: bool,
    pub strong: bool,
    pub expert: bool,
    pub allowance: bool,
    pub item: bool,
    pub change: bool,
}

impl TrainerAi {
    /// Short profile such as `basic+strong+change`.
    pub fn profile(&self) -> String {
        let parts: Vec<&str> = [
            (self.basic, "basic"),
            (self.strong, "strong"),
            (self.expert, "expert"),
            (self.allowance, "allowance"),
            (self.item, "item"),
            (self.change, "change"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, n)| *n)
        .collect();
        if parts.is_empty() {
            "none".into()
        } else {
            parts.join("+")
        }
    }
}

/// One party member of a trainer (`data/trainer_teams.tsv`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerMon {
    pub slot: u8,
    pub species_id: u16,
    pub species: String,
    pub form: u8,
    pub level: u8,
    pub nature: u8,
    pub sex: u8,
    /// Effort levels in SHEET order: HP, Atk, Def, Spe, SpA, SpD (LIKELY file order).
    pub gv_sheet: [u8; 6],
    /// Move ids (0 = empty slot) with the "mastered" bool (LIKELY).
    pub moves: Vec<(u16, bool)>,
}

impl TrainerMon {
    /// Effort levels in the engine order HP, Atk, Def, SpA, SpD, Spe.
    pub fn effort_engine(&self) -> [u8; 6] {
        let g = self.gv_sheet;
        [g[0], g[1], g[2], g[4], g[5], g[3]]
    }

    /// Build the engine combatant: form-aware species row, the stored moves (unknown ids are
    /// skipped; an empty list falls back to Tackle), effort levels and the ASSUMED nature.
    pub fn to_combatant(&self, data: &BattleData) -> Result<Combatant, String> {
        let species = data
            .species_row_by_dex_form(self.species_id, self.form)?
            .ok_or_else(|| format!("unknown species #{} form {}", self.species_id, self.form))?;
        let ids: Vec<u16> = self.moves.iter().map(|(id, _)| *id).filter(|id| *id != 0).collect();
        let moves = moves_by_id(data, &ids)?;
        Ok(Combatant::with_effort_nature(
            species,
            self.level,
            moves,
            self.effort_engine(),
            nature_percent(self.nature),
        ))
    }
}

fn moves_by_id(data: &BattleData, ids: &[u16]) -> Result<Vec<BattleMove>, String> {
    let mut moves: Vec<BattleMove> = ids
        .iter()
        .filter_map(|id| data.move_row_by_id(*id).ok().flatten())
        .take(4)
        .collect();
    if moves.is_empty() {
        moves.push(data.move_row("tackle")?.ok_or("move table has no tackle")?);
    }
    Ok(moves)
}

/// Nature number (0..24) -> stat percentages in engine order HP/Atk/Def/SpA/SpD/Spe.
///
/// ASSUMED: the main-series table (raised stat = n / 5, lowered = n % 5 over Atk, Def, Spe,
/// SpA, SpD; equal = neutral). It fits the data (12, the neutral "Serious" slot, is the value
/// of most trainer members) but the game's own nature table was not read.
pub fn nature_percent(nature: u8) -> [u16; 6] {
    let mut out = [100u16; 6];
    if nature >= 25 {
        return out;
    }
    // main-series order Atk, Def, Spe, SpA, SpD -> engine indices
    const ENGINE: [usize; 5] = [1, 2, 5, 3, 4];
    let (up, down) = (usize::from(nature / 5), usize::from(nature % 5));
    if up != down {
        out[ENGINE[up]] = 110;
        out[ENGINE[down]] = 90;
    }
    out
}

/// One trainer battle (`data/trainer_index.tsv` + its party).
#[derive(Debug, Clone, PartialEq)]
pub struct TrainerDef {
    pub id: String,
    pub group: String,
    pub variant: String,
    /// Internal name label (`CAPTAIN_GANKUTSU`); game texts are out of scope.
    pub name_label: Option<String>,
    pub class_label: Option<String>,
    pub model_pack: Option<String>,
    pub loss_label: Option<String>,
    pub bgm: String,
    /// Root slots 9 (LIKELY battle position x, y, z + yaw; the area is not stored).
    pub position: [f32; 3],
    pub yaw_deg: f32,
    /// Root slot 11, ASSUMED prize base (no reader found in the code).
    pub prize_base: Option<u32>,
    /// Bag items the trainer may use (slots 12..15; LIKELY).
    pub items: Vec<u16>,
    pub ai: TrainerAi,
    pub party: Vec<TrainerMon>,
    pub restriction_species: Option<u16>,
}

impl TrainerDef {
    /// Development label for logs and the HUD (internal label, prettified; not a game text).
    pub fn display_label(&self) -> String {
        let raw = self.name_label.clone().unwrap_or_else(|| self.group.to_uppercase());
        pretty_label(&raw)
    }

    /// Party built as engine combatants (stored order).
    pub fn combatants(&self, data: &BattleData) -> Result<Vec<Combatant>, String> {
        self.party.iter().map(|m| m.to_combatant(data)).collect()
    }
}

/// `CAPTAIN_GANKUTSU` -> `Captain Gankutsu`.
pub fn pretty_label(raw: &str) -> String {
    raw.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let lower = w.to_lowercase();
            let mut c = lower.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every trainer of the sheets, by id.
#[derive(Debug, Clone, Default)]
pub struct TrainerCatalog {
    pub trainers: BTreeMap<String, TrainerDef>,
}

impl TrainerCatalog {
    /// Read `trainer_index.tsv` and `trainer_teams.tsv` from `data_root`.
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let root = data_root.as_ref();
        let index = Sheet::load(&root.join("trainer_index.tsv"))?;
        let teams = Sheet::load(&root.join("trainer_teams.tsv"))?;
        Ok(Self::from_sheets(&index, &teams))
    }

    /// Same as [`load`](Self::load) from sheet texts (tests).
    pub fn parse(index: &str, teams: &str) -> Self {
        Self::from_sheets(&Sheet::parse(index), &Sheet::parse(teams))
    }

    fn from_sheets(index: &Sheet, teams: &Sheet) -> Self {
        let mut parties: BTreeMap<String, Vec<TrainerMon>> = BTreeMap::new();
        for r in &teams.rows {
            let moves = (1..=4)
                .map(|k| (num(r, &format!("move{k}_id")).unwrap_or(0), flag(r, &format!("move{k}_mastered"))))
                .collect();
            let gv = ["gv_hp", "gv_atk", "gv_def", "gv_spe", "gv_spa", "gv_spd"].map(|c| num(r, c).unwrap_or(0));
            parties.entry(text(r, "trainer_id")).or_default().push(TrainerMon {
                slot: num(r, "slot").unwrap_or(0),
                species_id: num(r, "species_id").unwrap_or(0),
                species: text(r, "species"),
                form: num(r, "form").unwrap_or(0),
                level: num(r, "level").unwrap_or(1),
                nature: num(r, "nature").unwrap_or(12),
                sex: num(r, "sex").unwrap_or(0),
                gv_sheet: gv,
                moves,
            });
        }
        for p in parties.values_mut() {
            p.sort_by_key(|m| m.slot);
        }
        let mut trainers = BTreeMap::new();
        for r in &index.rows {
            let id = text(r, "id");
            let items = ["item1", "item2", "item3", "item4"]
                .iter()
                .filter_map(|c| num::<u16>(r, c))
                .filter(|i| *i != 0)
                .collect();
            let def = TrainerDef {
                id: id.clone(),
                group: text(r, "group"),
                variant: text(r, "variant"),
                name_label: opt_text(r, "trname_label"),
                class_label: opt_text(r, "trtype_label"),
                model_pack: opt_text(r, "model_pack"),
                loss_label: opt_text(r, "loss_msg_label"),
                bgm: text(r, "bgm"),
                position: [num(r, "pos_x").unwrap_or(0.0), num(r, "pos_y").unwrap_or(0.0), num(r, "pos_z").unwrap_or(0.0)],
                yaw_deg: num(r, "yaw_deg").unwrap_or(0.0),
                prize_base: num(r, "prize_base"),
                items,
                ai: TrainerAi {
                    basic: flag(r, "ai_basic"),
                    strong: flag(r, "ai_strong"),
                    expert: flag(r, "ai_expert"),
                    allowance: flag(r, "ai_allowance"),
                    item: flag(r, "ai_item"),
                    change: flag(r, "ai_change"),
                },
                party: parties.remove(&id).unwrap_or_default(),
                restriction_species: num(r, "restriction_species"),
            };
            trainers.insert(id, def);
        }
        Self { trainers }
    }

    pub fn len(&self) -> usize {
        self.trainers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trainers.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&TrainerDef> {
        self.trainers.get(id)
    }

    /// Trainers whose model pack is `pack`, sorted by id, without the 242 species-restricted
    /// `single_*` challenges (they share the captain's model).
    pub fn by_model_pack(&self, pack: &str) -> Vec<&TrainerDef> {
        self.trainers
            .values()
            .filter(|t| t.model_pack.as_deref() == Some(pack) && t.group != "single")
            .collect()
    }
}

/// One Noble setting (`data/boss_battles.tsv`).
#[derive(Debug, Clone, PartialEq)]
pub struct NobleDef {
    pub id: String,
    pub rematch: bool,
    pub ns_key: String,
    pub bgm: String,
    pub species_id: u16,
    pub species: String,
    pub form: u8,
    pub level: u8,
    pub nature_raw: Option<i32>,
    /// LIKELY frenzy gauge capacity (absent on rematches).
    pub gauge_max: Option<u32>,
    pub balm_item: u16,
    pub moves: Vec<u16>,
    pub field_attacks: u16,
    pub field_attacks_h: u16,
}

impl NobleDef {
    /// The Noble as an engine combatant. Rows without stored moves take the level-up moves
    /// known at its level (needs the progression tables; Tackle otherwise). Effort level 10 on
    /// rematches (stored) is not in this sheet's columns, so effort stays 0 (UNKNOWN for the
    /// first battles).
    pub fn to_combatant(&self, data: &BattleData, progression: Option<&ProgressionTables>) -> Result<Combatant, String> {
        let species = data
            .species_row_by_dex_form(self.species_id, self.form)?
            .ok_or_else(|| format!("unknown species #{} form {}", self.species_id, self.form))?;
        let ids: Vec<u16> = if self.moves.is_empty() {
            progression
                .map(|t| initial_moves(t, self.species_id, self.form, self.level))
                .unwrap_or_default()
        } else {
            self.moves.clone()
        };
        let moves = moves_by_id(data, &ids)?;
        let nature = self.nature_raw.and_then(|n| u8::try_from(n).ok()).unwrap_or(12);
        Ok(Combatant::with_effort_nature(species, self.level, moves, [0; 6], nature_percent(nature)))
    }

    pub fn display_label(&self) -> String {
        format!("Noble {}", pretty_label(&self.species))
    }
}

/// The 16 Noble settings, by id.
#[derive(Debug, Clone, Default)]
pub struct NobleCatalog {
    pub nobles: BTreeMap<String, NobleDef>,
}

impl NobleCatalog {
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let sheet = Sheet::load(&data_root.as_ref().join("boss_battles.tsv"))?;
        Ok(Self::from_sheet(&sheet))
    }

    fn from_sheet(sheet: &Sheet) -> Self {
        let mut nobles = BTreeMap::new();
        for r in &sheet.rows {
            let id = text(r, "id");
            let moves = ["move1_id", "move2_id", "move3_id", "move4_id"]
                .iter()
                .filter_map(|c| num::<u16>(r, c))
                .filter(|m| *m != 0)
                .collect();
            nobles.insert(
                id.clone(),
                NobleDef {
                    id,
                    rematch: flag(r, "rematch"),
                    ns_key: text(r, "ns_key"),
                    bgm: text(r, "bgm"),
                    species_id: num(r, "species_id").unwrap_or(0),
                    species: text(r, "species"),
                    form: num(r, "form").unwrap_or(0),
                    level: num(r, "level").unwrap_or(1),
                    nature_raw: num(r, "nature_raw"),
                    gauge_max: num(r, "gauge_max"),
                    balm_item: num(r, "balm_item").unwrap_or(0),
                    moves,
                    field_attacks: num(r, "field_attack_count").unwrap_or(0),
                    field_attacks_h: num(r, "field_attack_h_count").unwrap_or(0),
                },
            );
        }
        Self { nobles }
    }

    pub fn get(&self, id: &str) -> Option<&NobleDef> {
        self.nobles.get(id)
    }

    pub fn len(&self) -> usize {
        self.nobles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nobles.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    #[test]
    fn nature_table_is_neutral_on_the_diagonal_and_signed_elsewhere() {
        for n in [0u8, 6, 12, 18, 24] {
            assert_eq!(nature_percent(n), [100; 6], "nature {n} is neutral");
        }
        // 3 = Adamant in main-series order: +Atk -SpA
        assert_eq!(nature_percent(3), [100, 110, 100, 90, 100, 100]);
        // 10 = Timid: +Spe -Atk
        assert_eq!(nature_percent(10), [100, 90, 100, 100, 100, 110]);
        assert_eq!(nature_percent(99), [100; 6]);
    }

    #[test]
    fn effort_goes_from_sheet_order_to_engine_order() {
        let m = TrainerMon {
            slot: 1,
            species_id: 1,
            species: "x".into(),
            form: 0,
            level: 5,
            nature: 12,
            sex: 0,
            gv_sheet: [1, 2, 3, 4, 5, 6], // HP Atk Def Spe SpA SpD
            moves: vec![],
        };
        assert_eq!(m.effort_engine(), [1, 2, 3, 5, 6, 4]);
    }

    #[test]
    fn parse_joins_party_rows_in_slot_order() {
        let index = "id:string*\tgroup:string\tvariant:string\ttrname_label:string?\tmodel_pack:string?\tbgm:string\tprize_base:u32?\titem1:u16?\tai_basic:bool\tai_strong:bool\tai_item:bool\tai_change:bool\n\
                     t1\ttrainer99\t01\tBANDIT_FIRST\ttr0033\tPLAY_BGM\t20\t24\t1\t1\t1\t0\n";
        let teams = "id:string*\ttrainer_id:string\tslot:u8\tspecies_id:u16\tspecies:string\tform:u16\tlevel:u8\tmove1_id:u16\n\
                     t1_2\tt1\t2\t399\tbidoof\t0\t9\t33\n\
                     t1_1\tt1\t1\t396\tstarly\t0\t8\t33\n";
        let c = TrainerCatalog::parse(index, teams);
        let t = c.get("t1").unwrap();
        assert_eq!(t.party.iter().map(|m| m.species.as_str()).collect::<Vec<_>>(), ["starly", "bidoof"]);
        assert_eq!(t.prize_base, Some(20));
        assert_eq!(t.items, vec![24]);
        assert_eq!(t.ai.profile(), "basic+strong+item");
        assert_eq!(t.display_label(), "Bandit First");
    }

    /// Counts of the committed sheets (reports/data-progress/trainers.md): 377 trainers,
    /// 620 members, party sizes 1x269 2x26 3x52 4x10 5x17 6x3, 10 trainers with items.
    #[test]
    fn real_sheets_load_every_trainer_and_member() {
        let Ok(c) = TrainerCatalog::load(data_root()) else {
            eprintln!("[skip] trainer sheets missing");
            return;
        };
        assert_eq!(c.len(), 377);
        let members: usize = c.trainers.values().map(|t| t.party.len()).sum();
        assert_eq!(members, 620);
        let mut sizes = [0usize; 7];
        for t in c.trainers.values() {
            sizes[t.party.len().min(6)] += 1;
        }
        assert_eq!(sizes, [0, 269, 26, 52, 10, 17, 3]);
        assert_eq!(c.trainers.values().filter(|t| !t.items.is_empty()).count(), 10);
        assert_eq!(c.trainers.values().filter(|t| t.ai.item).count(), 10);
        let g = c.get("giratina01").unwrap();
        assert_eq!((g.party[0].species_id, g.party[0].level), (487, 70));
        assert_eq!(g.bgm, "PLAY_BGM_VS_BOSS_03");
    }

    #[test]
    fn every_real_party_member_builds_an_engine_combatant() {
        let (Ok(c), Ok(data)) = (TrainerCatalog::load(data_root()), BattleData::load(data_root())) else {
            eprintln!("[skip] sheets missing");
            return;
        };
        let mut built = 0;
        let mut failed = Vec::new();
        for t in c.trainers.values() {
            for m in &t.party {
                match m.to_combatant(&data) {
                    Ok(cb) => {
                        assert_eq!(cb.level, m.level.max(1));
                        assert!(cb.max_hp > 0);
                        built += 1;
                    }
                    Err(e) => failed.push(format!("{} {}: {e}", t.id, m.species)),
                }
            }
        }
        assert!(failed.is_empty(), "{failed:?}");
        assert_eq!(built, 620);
    }

    #[test]
    fn hisuian_form_resolves_to_its_own_row() {
        let Ok(data) = BattleData::load(data_root()) else { return };
        // Arcanine form 0 is not an active row; form 1 (Hisuian) is Fire/Rock.
        let s = data.species_row_by_dex_form(59, 1).unwrap().unwrap();
        assert_eq!(s.types, ["Fire".to_string(), "Rock".to_string()]);
        let fallback = data.species_row_by_dex_form(59, 0).unwrap().unwrap();
        assert_eq!(fallback.dex, 59, "falls back to an active form");
    }

    #[test]
    fn real_noble_sheet_has_sixteen_settings() {
        let (Ok(n), Ok(data)) = (NobleCatalog::load(data_root()), BattleData::load(data_root())) else {
            return;
        };
        assert_eq!(n.len(), 16);
        assert_eq!(n.nobles.values().filter(|x| x.rematch).count(), 8);
        let k = n.get("nsi_ex_01").unwrap();
        assert_eq!((k.species_id, k.level, k.gauge_max), (900, 18, Some(1000)));
        let t = ProgressionTables::load(data_root()).ok();
        for noble in n.nobles.values() {
            let c = noble.to_combatant(&data, t.as_ref()).unwrap();
            assert!(!c.moves.is_empty());
        }
    }
}
