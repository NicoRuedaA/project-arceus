//! Experience, level-up, move learning, effort items and evolution (pure rules, no Bevy).
//!
//! Confidence labels used below:
//! - VERIFIED: read from the data tables or the recovered native code
//!   (`FUN_02b524b0` threshold lookup, `FUN_02b5fb2c` level-from-exp scan, `FUN_02b5fee8`
//!   exp cap, `FUN_02b5fce4` level delta, stat formulas in `Combatant`).
//! - ASSUMED: franchise convention, because the native exp-yield formula was not located
//!   (the amounts arrive in the battle-result record at +0x1a8 of the result UI, whose
//!   producer was not found). See `reports/data-progress/progression.md`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::Combatant;
use crate::battle::BattleSpecies;
use crate::pml::{EvolutionEntry, PmlDatabase};

/// Hard limit of known moves (matches `save_game::MAX_MOVES`).
pub const MAX_KNOWN_MOVES: usize = 4;
/// Highest level / effort level.
pub const MAX_LEVEL: u8 = 100;
pub const MAX_EFFORT: u8 = 10;
/// ASSUMED: friendship needed by the friendship evolution methods (1/2/3).
pub const FRIENDSHIP_EVOLVE: u8 = 220;

/// The persistent facts the rules need about one party member. `exp` is the cumulative
/// experience (the save format does not store it yet; see `game::party_progress`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressMember {
    pub species: u16,
    pub form: u8,
    pub level: u8,
    pub exp: u32,
    pub moves: Vec<u16>,
    pub effort: [u8; 6],
}

/// Data needed by every rule. Built once from the checked-in tables.
#[derive(Debug, Clone)]
pub struct ProgressionTables {
    /// `growth[group][level]` for level 1..=100 (index 0 unused, 0).
    growth: [[u32; 101]; 6],
    /// (species, form) -> (base_exp, growth group)
    personal: HashMap<(u16, u8), (u16, u8)>,
    /// (species, form) -> level-up moves as (level, mastery level, move id), table order.
    level_moves: HashMap<(u16, u8), Vec<(u8, u8, u16)>>,
    evolutions: Vec<EvolutionEntry>,
    items: HashMap<u16, ItemProgress>,
}

/// What an item does to progression (columns `ev_amount_*`, `level_up`, `evolution_item`
/// of `data/item_effects.tsv`; the column meaning is VERIFIED by item names, the
/// amount-to-level mapping is ASSUMED).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ItemProgress {
    /// Raw effort amounts in HP/Atk/Def/SpA/SpD/Spe order (negative lowers).
    pub effort_amount: [i32; 6],
    pub level_up: bool,
    pub evolution_item: bool,
}

impl Default for ProgressionTables {
    fn default() -> Self {
        Self {
            growth: [[0; 101]; 6],
            personal: HashMap::new(),
            level_moves: HashMap::new(),
            evolutions: Vec::new(),
            items: HashMap::new(),
        }
    }
}

impl ProgressionTables {
    pub fn from_pml(db: &PmlDatabase) -> Self {
        let mut t = Self::default();
        for g in &db.growth {
            if (g.group as usize) < 6 && (1..=100).contains(&g.level) {
                t.growth[g.group as usize][g.level as usize] = g.cumulative_exp;
            }
        }
        for p in &db.personal {
            t.personal
                .insert((p.national_dex, p.form), (p.base_exp_yield, p.growth_group));
        }
        for l in &db.learnsets {
            if l.learn_method == "level_up" {
                t.level_moves
                    .entry((l.species, l.form))
                    .or_default()
                    .push((l.level, l.mastery_level, l.move_id));
            }
        }
        t.evolutions = db.evolutions.clone();
        t
    }

    /// Load `pml_*.tsv` and `item_effects.tsv` from a data root (for example `data`).
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let root = data_root.as_ref();
        let mut t = Self::from_pml(&PmlDatabase::load_from_dir(root)?);
        let path = root.join("item_effects.tsv");
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        t.items = parse_item_progress(&text)?;
        Ok(t)
    }

    pub fn growth_group(&self, species: u16, form: u8) -> u8 {
        self.personal_row(species, form).map_or(0, |r| r.1)
    }
    pub fn base_exp(&self, species: u16, form: u8) -> u16 {
        self.personal_row(species, form).map_or(0, |r| r.0)
    }
    fn personal_row(&self, species: u16, form: u8) -> Option<(u16, u8)> {
        self.personal
            .get(&(species, form))
            .or_else(|| self.personal.get(&(species, 0)))
            .copied()
    }
    pub fn item(&self, item_id: u16) -> Option<ItemProgress> {
        self.items.get(&item_id).copied()
    }
    /// Level-up moves of a species/form, falling back to form 0 when the form has none.
    fn moves_of(&self, species: u16, form: u8) -> &[(u8, u8, u16)] {
        self.level_moves
            .get(&(species, form))
            .or_else(|| self.level_moves.get(&(species, 0)))
            .map_or(&[], Vec::as_slice)
    }
}

fn parse_item_progress(text: &str) -> Result<HashMap<u16, ItemProgress>, String> {
    let mut lines = text.lines().filter(|l| !l.is_empty() && !l.starts_with('#'));
    let header: Vec<&str> = lines
        .next()
        .ok_or("item effects: missing header")?
        .split('\t')
        .map(|h| h.split(':').next().unwrap_or(h).trim_end_matches(['*', '?']))
        .collect();
    let col = |n: &str| {
        header
            .iter()
            .position(|h| *h == n)
            .ok_or_else(|| format!("item effects: missing column {n}"))
    };
    let c_id = col("item_id")?;
    let c_amt = [
        col("ev_amount_hp")?,
        col("ev_amount_attack")?,
        col("ev_amount_defense")?,
        col("ev_amount_sp_attack")?,
        col("ev_amount_sp_defense")?,
        col("ev_amount_speed")?,
    ];
    let c_lvl = col("level_up")?;
    let c_evo = col("evolution_item")?;
    let mut out = HashMap::new();
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != header.len() {
            continue;
        }
        let id: u16 = f[c_id].parse().map_err(|_| "item effects: bad item_id")?;
        let mut effort_amount = [0i32; 6];
        for (slot, c) in effort_amount.iter_mut().zip(c_amt) {
            *slot = f[c].parse().unwrap_or(0);
        }
        let p = ItemProgress {
            effort_amount,
            level_up: f[c_lvl] == "1",
            evolution_item: f[c_evo] == "1",
        };
        if p != ItemProgress::default() {
            out.insert(id, p);
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ experience

/// VERIFIED (`FUN_02b524b0`): cumulative exp needed to be at `level`; level clamps to 100,
/// level 0 is treated as 1.
pub fn exp_for_level(t: &ProgressionTables, group: u8, level: u8) -> u32 {
    t.growth[usize::from(group.min(5))][usize::from(level.clamp(1, MAX_LEVEL))]
}

/// VERIFIED (`FUN_02b5fb2c`): the largest level whose threshold is `<= exp` (a Pokemon with
/// exactly the threshold is already at that level), minimum 1, maximum 100.
pub fn exp_to_level(t: &ProgressionTables, group: u8, exp: u32) -> u8 {
    let mut level = 1;
    for l in 2..=MAX_LEVEL {
        if exp < exp_for_level(t, group, l) {
            break;
        }
        level = l;
    }
    level
}

/// Exp still needed for the next level (`None` at level 100).
pub fn exp_to_next(t: &ProgressionTables, group: u8, level: u8, exp: u32) -> Option<u32> {
    (level < MAX_LEVEL).then(|| exp_for_level(t, group, level + 1).saturating_sub(exp))
}

/// ASSUMED (franchise convention, native formula not found): experience a defeated wild
/// Pokemon gives to ONE participant, `base_exp * level / 7`, times 1.5 for trainers, divided
/// by the number of participants (at least 1 each when anything was defeated).
pub fn exp_yield(base_exp: u16, defeated_level: u8, participants: u32, trainer: bool) -> u32 {
    let mut v = u64::from(base_exp) * u64::from(defeated_level.max(1)) / 7;
    if trainer {
        v = v * 3 / 2;
    }
    let n = u64::from(participants.max(1));
    ((v / n).max(1)) as u32
}

/// Convenience: yield of defeating `species`/`form` at `level` using the table's base exp.
pub fn exp_yield_for(
    t: &ProgressionTables,
    species: u16,
    form: u8,
    level: u8,
    participants: u32,
    trainer: bool,
) -> u32 {
    exp_yield(t.base_exp(species, form), level, participants, trainer)
}

/// Result of giving experience to a member.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AwardOutcome {
    pub gained: u32,
    pub old_level: u8,
    pub new_level: u8,
    /// Moves learned automatically (a free slot existed), as (level, move id).
    pub learned: Vec<(u8, u16)>,
    /// Moves that need a player decision because all 4 slots are taken, as (level, move id).
    pub pending: Vec<(u8, u16)>,
}

impl AwardOutcome {
    pub fn leveled_up(&self) -> bool {
        self.new_level > self.old_level
    }
}

/// Add experience (capped at the level-100 threshold, VERIFIED `FUN_02b5fee8`), update the
/// level and learn the moves of every level crossed. Stats are NOT recomputed here: call
/// [`level_up_combatant`] on the engine type afterwards.
pub fn award_exp(t: &ProgressionTables, m: &mut ProgressMember, amount: u32) -> AwardOutcome {
    let group = t.growth_group(m.species, m.form);
    let cap = exp_for_level(t, group, MAX_LEVEL);
    let old_level = m.level;
    m.exp = m.exp.saturating_add(amount).min(cap);
    let new_level = exp_to_level(t, group, m.exp).max(old_level);
    m.level = new_level;
    let mut out = AwardOutcome {
        gained: amount,
        old_level,
        new_level,
        ..Default::default()
    };
    for (lvl, mv) in new_moves_at_level(t, m.species, m.form, old_level, new_level) {
        match learn_move(m, mv) {
            Ok(()) | Err(LearnError::AlreadyKnown) => {
                if !out.learned.contains(&(lvl, mv)) && m.moves.contains(&mv) {
                    out.learned.push((lvl, mv));
                }
            }
            Err(LearnError::NoFreeSlot) => out.pending.push((lvl, mv)),
        }
    }
    out
}

/// Exp set to the threshold of the next level (VERIFIED idea of `FUN_02b5fce4`: level delta
/// sets exp to the threshold of the target level, capped at 100). Used by rare candy.
pub fn raise_levels(t: &ProgressionTables, m: &mut ProgressMember, levels: u8) -> AwardOutcome {
    let group = t.growth_group(m.species, m.form);
    let target = m.level.saturating_add(levels).min(MAX_LEVEL);
    let need = exp_for_level(t, group, target).saturating_sub(m.exp);
    award_exp(t, m, need)
}

/// Recompute the engine stats after a level change (VERIFIED formulas live in `Combatant`);
/// current HP grows by the same amount as max HP (never shrinks, fainted stays fainted).
pub fn level_up_combatant(c: &mut Combatant, new_level: u8) {
    let old_max = c.max_hp;
    let fresh = Combatant::with_effort_nature(
        c.species.clone(),
        new_level,
        Vec::new(),
        c.effort,
        c.nature_percent,
    );
    c.level = fresh.level;
    c.max_hp = fresh.max_hp;
    c.attack = fresh.attack;
    c.defense = fresh.defense;
    c.special_attack = fresh.special_attack;
    c.special_defense = fresh.special_defense;
    c.speed = fresh.speed;
    if c.hp > 0 {
        c.hp = (c.hp + c.max_hp.saturating_sub(old_max)).min(c.max_hp);
    }
}

// ------------------------------------------------------------------ move learning

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnError {
    AlreadyKnown,
    NoFreeSlot,
}

/// Level-up moves with `from_level < learn level <= to_level`, in table order
/// (VERIFIED against `pml_learnsets.tsv`). Returns (level, move id).
pub fn new_moves_at_level(
    t: &ProgressionTables,
    species: u16,
    form: u8,
    from_level: u8,
    to_level: u8,
) -> Vec<(u8, u16)> {
    let mut v: Vec<(u8, u16)> = t
        .moves_of(species, form)
        .iter()
        .filter(|(l, _, _)| *l > from_level && *l <= to_level && *l > 0)
        .map(|(l, _, m)| (*l, *m))
        .collect();
    v.sort_by_key(|(l, _)| *l);
    v
}

/// The moves a fresh Pokemon of `level` would know: the last 4 distinct level-up moves
/// at or below that level (ASSUMED: "most recent four" rule).
pub fn initial_moves(t: &ProgressionTables, species: u16, form: u8, level: u8) -> Vec<u16> {
    let mut all: Vec<(u8, u16)> = t
        .moves_of(species, form)
        .iter()
        .filter(|(l, _, _)| *l <= level)
        .map(|(l, _, m)| (*l, *m))
        .collect();
    all.sort_by_key(|(l, _)| *l);
    let mut out: Vec<u16> = Vec::new();
    for (_, m) in all {
        if !out.contains(&m) {
            out.push(m);
        }
    }
    let skip = out.len().saturating_sub(MAX_KNOWN_MOVES);
    out.split_off(skip)
}

/// Learn into a free slot.
pub fn learn_move(m: &mut ProgressMember, move_id: u16) -> Result<(), LearnError> {
    if m.moves.contains(&move_id) {
        return Err(LearnError::AlreadyKnown);
    }
    if m.moves.len() >= MAX_KNOWN_MOVES {
        return Err(LearnError::NoFreeSlot);
    }
    m.moves.push(move_id);
    Ok(())
}

/// Replace the move in `slot`; returns the forgotten move.
pub fn replace_move(m: &mut ProgressMember, slot: usize, move_id: u16) -> Result<u16, LearnError> {
    if m.moves.contains(&move_id) {
        return Err(LearnError::AlreadyKnown);
    }
    let old = m.moves.get_mut(slot).ok_or(LearnError::NoFreeSlot)?;
    Ok(std::mem::replace(old, move_id))
}

// ------------------------------------------------------------------ effort

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffortError {
    AlreadyMax,
    AlreadyZero,
    BadStat,
    NotAnEffortItem,
}

/// Change one effort level by `delta` (+/-); refuses when nothing would change
/// (level 10 cannot go to 11). Levels 0..=10 VERIFIED (table K has 11 entries).
pub fn change_effort(m: &mut ProgressMember, stat: usize, delta: i8) -> Result<u8, EffortError> {
    let cur = *m.effort.get(stat).ok_or(EffortError::BadStat)?;
    let next = i16::from(cur) + i16::from(delta);
    if delta > 0 && cur >= MAX_EFFORT {
        return Err(EffortError::AlreadyMax);
    }
    if delta < 0 && cur == 0 {
        return Err(EffortError::AlreadyZero);
    }
    let next = next.clamp(0, i16::from(MAX_EFFORT)) as u8;
    m.effort[stat] = next;
    Ok(next)
}

/// Apply an item from `item_effects.tsv`. ASSUMED mapping: `ev_amount / 10` effort levels
/// (vitamin-like items carry +10, feather-like items -10). Returns the stats changed.
pub fn use_progress_item(
    t: &ProgressionTables,
    m: &mut ProgressMember,
    item_id: u16,
) -> Result<AwardOutcome, EffortError> {
    let p = t.item(item_id).ok_or(EffortError::NotAnEffortItem)?;
    let mut touched = false;
    let mut last_err = None;
    for (stat, amount) in p.effort_amount.iter().enumerate() {
        if *amount == 0 {
            continue;
        }
        let delta = (amount / 10).clamp(-10, 10) as i8;
        let delta = if delta == 0 { amount.signum() as i8 } else { delta };
        match change_effort(m, stat, delta) {
            Ok(_) => touched = true,
            Err(e) => last_err = Some(e),
        }
    }
    if p.level_up {
        return Ok(raise_levels(t, m, 1));
    }
    if touched {
        Ok(AwardOutcome { old_level: m.level, new_level: m.level, ..Default::default() })
    } else {
        Err(last_err.unwrap_or(EffortError::NotAnEffortItem))
    }
}

// ------------------------------------------------------------------ evolution

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Gender {
    Male,
    Female,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeOfDay {
    #[default]
    Day,
    Night,
}

/// Why the check is being made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EvolutionTrigger {
    /// Just after a level change.
    #[default]
    LevelUp,
    /// The player used this item on the member.
    UseItem(u16),
    /// Linking cord / trade.
    Trade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Place {
    #[default]
    Anywhere,
    MagneticField,
    MossRock,
    IceRock,
}

/// Facts outside the member that conditions may read. Anything the game cannot supply stays
/// at its default and the matching conditions simply do not fire.
#[derive(Debug, Clone, Default)]
pub struct EvolutionContext {
    pub trigger: EvolutionTrigger,
    pub gender: Gender,
    pub time: TimeOfDay,
    pub full_moon: bool,
    pub raining: bool,
    pub friendship: Option<u8>,
    pub place: Place,
    /// Species ids currently in the party (for "level up with party species").
    pub party_species: Vec<u16>,
    /// Type ids of the moves the member knows (for "move type affection").
    pub known_move_types: Vec<u8>,
    /// Personality roll for Wurmple (ASSUMED: `< 5` Silcoon, otherwise Cascoon).
    pub personality: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evolution {
    pub target_species: u16,
    pub target_form: u8,
    pub method: u8,
    pub evolution_index: u8,
}

/// Outcome of testing one table row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionResult {
    Met,
    NotMet,
    /// The game has no data to evaluate this condition yet (needs a system we lack).
    Unsupported(&'static str),
}

fn level_gate(m: &ProgressMember, e: &EvolutionEntry, ctx: &EvolutionContext) -> bool {
    ctx.trigger == EvolutionTrigger::LevelUp && m.level >= e.level
}

fn item_gate(ctx: &EvolutionContext, e: &EvolutionEntry) -> bool {
    ctx.trigger == EvolutionTrigger::UseItem(e.argument)
}

/// Evaluate one row. Method numbers: see `reports/data-progress/progression.md`.
pub fn condition_met(
    m: &ProgressMember,
    e: &EvolutionEntry,
    ctx: &EvolutionContext,
) -> ConditionResult {
    use ConditionResult::{Met, NotMet, Unsupported};
    let b = |x: bool| if x { Met } else { NotMet };
    match e.method {
        1 => b(ctx.trigger == EvolutionTrigger::LevelUp
            && ctx.friendship.is_some_and(|f| f >= FRIENDSHIP_EVOLVE)),
        2 => b(ctx.trigger == EvolutionTrigger::LevelUp
            && ctx.time == TimeOfDay::Day
            && ctx.friendship.is_some_and(|f| f >= FRIENDSHIP_EVOLVE)),
        3 => b(ctx.trigger == EvolutionTrigger::LevelUp
            && ctx.time == TimeOfDay::Night
            && ctx.friendship.is_some_and(|f| f >= FRIENDSHIP_EVOLVE)),
        4 => b(level_gate(m, e, ctx)),
        5 => b(ctx.trigger == EvolutionTrigger::Trade),
        8 => b(item_gate(ctx, e)),
        12 => b(level_gate(m, e, ctx) && ctx.personality.is_some_and(|p| p < 5)),
        13 => b(level_gate(m, e, ctx) && ctx.personality.is_some_and(|p| p >= 5)),
        17 => b(item_gate(ctx, e) && ctx.gender == Gender::Male && m.level >= e.level),
        18 => b(item_gate(ctx, e) && ctx.gender == Gender::Female && m.level >= e.level),
        21 => b(ctx.trigger == EvolutionTrigger::LevelUp && m.moves.contains(&e.argument)),
        22 => b(ctx.trigger == EvolutionTrigger::LevelUp && ctx.party_species.contains(&e.argument)),
        23 => b(level_gate(m, e, ctx) && ctx.gender == Gender::Male),
        24 => b(level_gate(m, e, ctx) && ctx.gender == Gender::Female),
        25 => b(ctx.trigger == EvolutionTrigger::LevelUp && ctx.place == Place::MagneticField),
        26 => b(ctx.trigger == EvolutionTrigger::LevelUp && ctx.place == Place::MossRock),
        27 => b(ctx.trigger == EvolutionTrigger::LevelUp && ctx.place == Place::IceRock),
        29 => b(ctx.trigger == EvolutionTrigger::LevelUp
            && ctx.known_move_types.contains(&(e.argument as u8))
            && ctx.friendship.is_some_and(|f| f >= FRIENDSHIP_EVOLVE)),
        31 => b(level_gate(m, e, ctx) && ctx.raining),
        48 => b(item_gate(ctx, e) && ctx.time == TimeOfDay::Day),
        49 => b(item_gate(ctx, e) && ctx.time == TimeOfDay::Night),
        50 => b(item_gate(ctx, e) && ctx.full_moon),
        51 => Unsupported("agile style move counter: threshold not in the tables"),
        52 => Unsupported("strong style move counter: threshold not in the tables"),
        53 | 54 => Unsupported("recoil damage counter: threshold not in the tables"),
        _ => Unsupported("unknown evolution method"),
    }
}

/// All evolutions the member can take right now (table order, so Eevee's first match wins
/// in [`check_evolution`]).
pub fn evolution_candidates(
    t: &ProgressionTables,
    m: &ProgressMember,
    ctx: &EvolutionContext,
) -> Vec<Evolution> {
    t.evolutions
        .iter()
        .filter(|e| e.species == m.species && e.form == m.form)
        .filter(|e| condition_met(m, e, ctx) == ConditionResult::Met)
        .map(|e| Evolution {
            target_species: e.target_species,
            target_form: e.target_form,
            method: e.method,
            evolution_index: e.evolution_index,
        })
        .collect()
}

/// The evolution to perform, if any (first candidate).
pub fn check_evolution(
    t: &ProgressionTables,
    m: &ProgressMember,
    ctx: &EvolutionContext,
) -> Option<Evolution> {
    evolution_candidates(t, m, ctx).into_iter().next()
}

/// Apply an evolution to the persistent data (species/form change; level, moves, effort and
/// exp are kept; exp is raised to the threshold of the new growth group if lower).
pub fn evolve(t: &ProgressionTables, m: &mut ProgressMember, evo: &Evolution) {
    m.species = evo.target_species;
    m.form = evo.target_form;
    let group = t.growth_group(m.species, m.form);
    m.exp = m.exp.max(exp_for_level(t, group, m.level));
}

/// Rebuild the engine combatant for the evolved species, keeping moves, status, effort,
/// nature and the HP lost so far.
pub fn evolve_combatant(c: &mut Combatant, new_species: BattleSpecies) {
    let lost = c.max_hp.saturating_sub(c.hp);
    let alive = c.hp > 0;
    let mut fresh = Combatant::with_effort_nature(new_species, c.level, Vec::new(), c.effort, c.nature_percent);
    fresh.moves = std::mem::take(&mut c.moves);
    fresh.status = c.status;
    fresh.caught = c.caught;
    fresh.hp = if alive { fresh.max_hp.saturating_sub(lost).max(1) } else { 0 };
    *c = fresh;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tables() -> ProgressionTables {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        ProgressionTables::load(root).expect("tables")
    }
    fn member(species: u16, level: u8, exp: u32) -> ProgressMember {
        ProgressMember { species, form: 0, level, exp, moves: vec![], effort: [0; 6] }
    }

    #[test]
    fn thresholds_match_the_table() {
        let t = tables();
        // Medium Fast (group 0) is n^3: 8, 27 ... 1_000_000.
        assert_eq!(exp_for_level(&t, 0, 2), 8);
        assert_eq!(exp_for_level(&t, 0, 6), 216);
        assert_eq!(exp_for_level(&t, 0, 100), 1_000_000);
        assert_eq!(exp_for_level(&t, 0, 200), 1_000_000);
        assert_eq!(exp_for_level(&t, 5, 100), 1_250_000);
        assert_eq!(exp_to_level(&t, 0, 215), 5);
        assert_eq!(exp_to_level(&t, 0, 216), 6, "exactly the threshold is the new level");
        assert_eq!(exp_to_level(&t, 0, u32::MAX), 100);
    }

    #[test]
    fn level_five_reaches_six_exactly_at_threshold() {
        let t = tables();
        // Pikachu: growth group 0, learns moves while levelling.
        let mut m = member(25, 5, 125);
        let o = award_exp(&t, &mut m, 90);
        assert_eq!(m.exp, 215);
        assert_eq!(m.level, 5);
        assert!(!o.leveled_up());
        let o = award_exp(&t, &mut m, 1);
        assert_eq!((o.old_level, o.new_level, m.level), (5, 6, 6));
        assert_eq!(exp_to_next(&t, 0, 6, m.exp), Some(343 - 216));
    }

    #[test]
    fn exp_is_capped_at_level_100() {
        let t = tables();
        let mut m = member(25, 99, 970_299);
        award_exp(&t, &mut m, 5_000_000);
        assert_eq!((m.level, m.exp), (100, 1_000_000));
    }

    #[test]
    fn yield_formula_is_the_assumed_one() {
        // 112 * 10 / 7 = 160; trainer x1.5 = 240; split in two = 120.
        assert_eq!(exp_yield(112, 10, 1, false), 160);
        assert_eq!(exp_yield(112, 10, 1, true), 240);
        assert_eq!(exp_yield(112, 10, 2, true), 120);
        assert_eq!(exp_yield(0, 1, 1, false), 1);
        let t = tables();
        assert_eq!(t.base_exp(25, 0), 112);
    }

    #[test]
    fn learnset_at_known_levels() {
        let t = tables();
        // Pikachu (form 0) rows: thunder-shock(84) L1, quick-attack(98) L5, thunder-wave(86) L9,
        // swift(129) L15.
        assert_eq!(initial_moves(&t, 25, 0, 1), vec![84]);
        assert_eq!(initial_moves(&t, 25, 0, 9), vec![84, 98, 86]);
        assert_eq!(initial_moves(&t, 25, 0, 15), vec![84, 98, 86, 129]);
        assert_eq!(initial_moves(&t, 25, 0, 21), vec![98, 86, 129, 209], "only the last four");
        assert_eq!(new_moves_at_level(&t, 25, 0, 4, 9), vec![(5, 98), (9, 86)]);
        assert_eq!(new_moves_at_level(&t, 25, 0, 5, 8), vec![]);
        // Form 1 has its own list (last four of its level-1 and level-5 rows).
        assert_eq!(initial_moves(&t, 25, 1, 5), vec![39, 45, 98, 609]);
    }

    #[test]
    fn learn_and_replace_respect_the_four_move_limit() {
        let mut m = member(1, 5, 125);
        for id in [1, 2, 3, 4] {
            learn_move(&mut m, id).unwrap();
        }
        assert_eq!(learn_move(&mut m, 5), Err(LearnError::NoFreeSlot));
        assert_eq!(learn_move(&mut m, 2), Err(LearnError::AlreadyKnown));
        assert_eq!(replace_move(&mut m, 1, 5), Ok(2));
        assert_eq!(m.moves, vec![1, 5, 3, 4]);
        assert_eq!(replace_move(&mut m, 9, 6), Err(LearnError::NoFreeSlot));
    }

    #[test]
    fn award_learns_into_free_slot_then_queues_pending() {
        let t = tables();
        let (l, mv) = *new_moves_at_level(&t, 25, 0, 1, 100).first().unwrap();
        let mut m = member(25, l - 1, exp_for_level(&t, 0, l - 1));
        m.moves = vec![84];
        let need = exp_for_level(&t, 0, l) - m.exp;
        let o = award_exp(&t, &mut m, need);
        assert!(o.learned.contains(&(l, mv)));
        assert!(m.moves.contains(&mv));
        // Full set of moves: the next learnable becomes pending instead.
        let mut full = member(25, l - 1, exp_for_level(&t, 0, l - 1));
        full.moves = vec![901, 902, 903, 904];
        let need = exp_for_level(&t, 0, l) - full.exp;
        let o = award_exp(&t, &mut full, need);
        assert!(o.pending.contains(&(l, mv)));
        assert_eq!(full.moves, vec![901, 902, 903, 904]);
    }

    #[test]
    fn level_evolution_zubat_to_golbat_at_22() {
        let t = tables();
        let ctx = EvolutionContext::default();
        assert_eq!(check_evolution(&t, &member(41, 21, 0), &ctx), None);
        let evo = check_evolution(&t, &member(41, 22, 0), &ctx).unwrap();
        assert_eq!((evo.target_species, evo.method), (42, 4));
        let mut m = member(41, 22, 10_648);
        evolve(&t, &mut m, &evo);
        assert_eq!((m.species, m.level), (42, 22));
    }

    #[test]
    fn item_evolution_pikachu_needs_thunder_stone() {
        let t = tables();
        let m = member(25, 20, 8000);
        let none = EvolutionContext::default();
        assert_eq!(check_evolution(&t, &m, &none), None);
        let wrong = EvolutionContext { trigger: EvolutionTrigger::UseItem(82), ..Default::default() };
        assert_eq!(check_evolution(&t, &m, &wrong), None);
        let ok = EvolutionContext { trigger: EvolutionTrigger::UseItem(83), ..Default::default() };
        assert_eq!(check_evolution(&t, &m, &ok).unwrap().target_species, 26);
    }

    #[test]
    fn gender_time_and_unsupported_conditions() {
        let t = tables();
        let ctx = EvolutionContext { gender: Gender::Female, ..Default::default() };
        let female = check_evolution(&t, &member(412, 20, 0), &ctx).unwrap();
        assert_eq!(female.target_species, 413);
        let male = EvolutionContext { gender: Gender::Male, ..Default::default() };
        assert_eq!(check_evolution(&t, &member(412, 20, 0), &male).unwrap().target_species, 414);
        // Sneasel (form 0) + Razor Claw only at night.
        let day = EvolutionContext { trigger: EvolutionTrigger::UseItem(326), ..Default::default() };
        assert_eq!(check_evolution(&t, &member(215, 30, 0), &day), None);
        let night = EvolutionContext { time: TimeOfDay::Night, ..day };
        assert_eq!(check_evolution(&t, &member(215, 30, 0), &night).unwrap().target_species, 461);
        // Stantler: agile-style counter has no data -> never fires.
        let e = t.evolutions.iter().find(|e| e.species == 234).unwrap();
        assert!(matches!(
            condition_met(&member(234, 50, 0), e, &EvolutionContext::default()),
            ConditionResult::Unsupported(_)
        ));
    }

    #[test]
    fn effort_items_cap_at_ten_and_reject_eleven() {
        let t = tables();
        let mut m = member(25, 10, 1000);
        m.effort[0] = 9;
        // item 45 raises HP effort (ev_amount_hp = 10).
        use_progress_item(&t, &mut m, 45).unwrap();
        assert_eq!(m.effort[0], 10);
        assert_eq!(use_progress_item(&t, &mut m, 45), Err(EffortError::AlreadyMax));
        assert_eq!(change_effort(&mut m, 0, 1), Err(EffortError::AlreadyMax));
        // item 169 lowers HP effort.
        use_progress_item(&t, &mut m, 169).unwrap();
        assert_eq!(m.effort[0], 9);
        assert_eq!(use_progress_item(&t, &mut m, 1), Err(EffortError::NotAnEffortItem));
    }

    #[test]
    fn rare_candy_adds_exactly_one_level() {
        let t = tables();
        let mut m = member(25, 5, 150);
        let o = use_progress_item(&t, &mut m, 50).unwrap();
        assert_eq!((o.old_level, o.new_level, m.exp), (5, 6, 216));
    }

    #[test]
    fn level_up_recomputes_stats_and_grows_current_hp() {
        let sp = BattleSpecies {
            dex: 25,
            name: "pikachu".into(),
            hp: 35,
            attack: 55,
            defense: 40,
            special_attack: 50,
            special_defense: 50,
            speed: 90,
            types: ["Electric".into(), "Electric".into()],
            abilities: [0; 3],
        };
        let mut c = Combatant::from_base(sp.clone(), 5, vec![]);
        c.hp -= 3;
        let (old_max, old_hp, old_atk) = (c.max_hp, c.hp, c.attack);
        level_up_combatant(&mut c, 6);
        assert_eq!(c.level, 6);
        assert!(c.max_hp > old_max && c.attack >= old_atk);
        assert_eq!(c.hp, old_hp + (c.max_hp - old_max));
        let fresh = Combatant::from_base(sp, 6, vec![]);
        assert_eq!((c.max_hp, c.attack, c.speed), (fresh.max_hp, fresh.attack, fresh.speed));
    }
}
