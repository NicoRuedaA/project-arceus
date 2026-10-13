//! Agile and Strong styles and the PLA action-order queue (brief 29), plus the opt-in
//! Fixated and Alpha multipliers.
//!
//! Everything here is additive and opt-in: a `Battle` that never calls
//! [`Battle::set_move_styles`], [`Battle::enable_action_queue`], [`Battle::set_fixated`] or
//! [`Battle::play_turn_styled`] behaves exactly as before.
//!
//! Confidence (VERIFIED = read from the update `main` code and/or game data, checked by a
//! hand-computed test; LIKELY = reader or layout identified, consumer not fully traced;
//! ASSUMED = engine choice, nothing read either way). Function names and addresses are listed
//! only in `reports/data-progress/battle_styles.md`; no game code is reproduced here.
//!
//! * Style numbering in the game's move-data readers: 0 normal, 1 strong ("Powerful"),
//!   2 agile ("Technical") (VERIFIED: config keys `*_Powerful` / `*_Technical` sit on the same
//!   style branches as the data slots, and the strong slots hold the higher power, longer
//!   durations and bigger heals).
//! * Power per style: `power` / `agile_power` / `strong_power` (VERIFIED data + reader);
//!   accuracy: agile uses the normal accuracy, strong uses `strong_accuracy` (VERIFIED reader).
//! * Action points per style: self `raw_f69` normal, `raw_f70` agile, `raw_f71` strong; target
//!   `raw_f72` normal and strong, `raw_f73` agile (VERIFIED readers). Agile values are weakened
//!   by consecutive agile uses: a negative self value or a positive target value is divided by
//!   3^min(streak, 4), rounded away from zero, but never smaller in size than the normal-style
//!   value (VERIFIED). Whether `streak` counts the current use is LIKELY "no" (the preview and
//!   the commit read the counter before the move updates it).
//! * Queue: each combatant waits `W = ceil(16 - agility^0.4)` ticks (VERIFIED). After an
//!   action the actor's wait becomes `W + pending`, every other combatant's wait drops by the
//!   elapsed ticks (the actor's wait when it was picked) and is corrected by `W - previous W`
//!   plus its pending points; stored waits are clamped at 0 (VERIFIED). The next actor is the
//!   lowest wait; ties: not the previous actor, then the higher agility, then the lower battle
//!   position (VERIFIED; position = side index here, ASSUMED). The game shows 6 upcoming
//!   turns (VERIFIED). A settle step with no actor at the start is LIKELY.
//! * PP cost of a style: UNKNOWN (not found in code). The engine spends 1 PP whatever the
//!   style (ASSUMED) and exposes `StyleRules::extra_pp_per_style` for callers.
//! * Fixated: the move it is fixated on deals x150/100, a fixated defender takes x133/100
//!   (VERIFIED values and placement); what makes a Pokémon fixated is UNKNOWN, so the caller
//!   sets it.
//! * Alpha: Atk, Def, SpA, SpD and Spe x1.5 (`oybnPowerCoef`, VERIFIED value and stats;
//!   truncation ASSUMED). Nobles use `nsPowerCoef` = 1 (no change).

use std::collections::HashMap;
use std::path::Path;

use super::{agility, Action, Battle, BattleRng, Combatant, Event};

/// A move's style. `Normal` is the plain move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Style {
    #[default]
    Normal,
    Agile,
    Strong,
}

impl Style {
    pub const ALL: [Style; 3] = [Style::Normal, Style::Agile, Style::Strong];

    /// Index used by the game's move-data readers: 0 normal, 1 strong, 2 agile (VERIFIED).
    pub fn game_index(self) -> u8 {
        match self {
            Style::Normal => 0,
            Style::Strong => 1,
            Style::Agile => 2,
        }
    }

    pub fn from_game_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Style::Normal),
            1 => Some(Style::Strong),
            2 => Some(Style::Agile),
            _ => None,
        }
    }

    fn slot(self) -> usize {
        match self {
            Style::Normal => 0,
            Style::Agile => 1,
            Style::Strong => 2,
        }
    }
}

/// VERIFIED: divisor base and exponent cap of the consecutive-agile weakening.
pub const AGILE_STREAK_BASE: f32 = 3.0;
pub const AGILE_STREAK_CAP: u8 = 4;
/// VERIFIED: number of upcoming turns the game lists.
pub const TURN_ORDER_PREVIEW_LEN: usize = 6;
/// VERIFIED: base of the wait formula `ceil(16 - agility^0.4)`.
pub const WAIT_BASE: f64 = 16.0;
/// VERIFIED: exponent of the wait formula, stored by the game as a single-precision 0.4.
pub const WAIT_EXPONENT: f32 = 0.4;
/// VERIFIED: Fixated percentages (`wazaDrunk_WazaDamageRatio`, `_Receive`).
pub const FIXATED_DAMAGE_DEALT_PERCENT: u32 = 150;
pub const FIXATED_DAMAGE_TAKEN_PERCENT: u32 = 133;
/// VERIFIED: Alpha stat coefficient (`oybnPowerCoef`); nobles use `nsPowerCoef` = 1.
pub const ALPHA_POWER_COEF: f32 = 1.5;
pub const NOBLE_POWER_COEF: f32 = 1.0;

/// Per-style values of one move, indexed by [`Style`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveStyleData {
    pub name: String,
    /// `raw_f68`: the move is a PLA move whose styles can be unlocked (VERIFIED reader; the
    /// unlock itself needs mastery, see brief D3).
    pub styles_usable: bool,
    power: [u16; 3],
    accuracy: [u8; 3],
    self_points: [i32; 3],
    target_points: [i32; 3],
    /// LIKELY (reader identified): heal or HP-cost percent per style (signed).
    heal_percent: [i8; 3],
    /// LIKELY (reader identified): turns of a rank change per style.
    rank_turns: [u8; 3],
    /// LIKELY (layout): turns of an inflicted status per style (normal = data maximum).
    ailment_turns: [u8; 3],
    /// LIKELY (layout): critical rank, secondary-ailment chance, stat-change chance and
    /// drain/recoil percent; only strong differs from normal.
    crit_rank: [u8; 3],
    ailment_chance: [u8; 3],
    stat_chance: [u8; 3],
    drain_percent: [i8; 3],
}

fn signed_byte(value: i64) -> i8 {
    (value as u8) as i8
}

impl MoveStyleData {
    /// Build from one merged row of `pml_moves.tsv` and `pml_move_effects.tsv` (column name
    /// without its type suffix -> value). Missing effect columns read as 0.
    pub fn from_move_row(row: &HashMap<String, String>) -> Result<Self, String> {
        let num = |key: &str| -> Result<i64, String> {
            match row.get(key) {
                None => Ok(0),
                Some(v) if v.is_empty() => Ok(0),
                Some(v) => v
                    .parse::<f64>()
                    .map(|f| f as i64)
                    .map_err(|e| format!("column {key}: {e}")),
            }
        };
        let name = row
            .get("move_name")
            .cloned()
            .ok_or("missing move_name column")?;
        let power = num("power")? as u16;
        let agile_power = num("agile_power")? as u16;
        let strong_power = num("strong_power")? as u16;
        let accuracy = num("accuracy")? as u8;
        let strong_accuracy = num("strong_accuracy")? as u8;
        let or = |value: i64, normal: i64| if value != 0 { value } else { normal };
        let heal = num("heal_pct")?;
        let crit = num("crit_rank")?;
        let ailment_chance = num("ailment_chance")?;
        let stat_chance = num("stat1_chance")?;
        let drain = num("drain_pct")?;
        let ailment_max = num("raw_f15")?;
        let rank = num("raw_f31")?;
        Ok(Self {
            name,
            styles_usable: num("raw_f68")? != 0,
            // A style power of 0 on a damaging move means "no style data": keep the normal value.
            power: [
                power,
                if agile_power == 0 { power } else { agile_power },
                if strong_power == 0 {
                    power
                } else {
                    strong_power
                },
            ],
            accuracy: [
                accuracy,
                accuracy,
                if strong_accuracy == 0 {
                    accuracy
                } else {
                    strong_accuracy
                },
            ],
            self_points: [
                num("raw_f69")? as i32,
                num("raw_f70")? as i32,
                num("raw_f71")? as i32,
            ],
            target_points: [
                num("raw_f72")? as i32,
                num("raw_f73")? as i32,
                num("raw_f72")? as i32,
            ],
            heal_percent: [
                signed_byte(heal),
                signed_byte(or(num("raw_f75")?, heal)),
                signed_byte(or(num("raw_f88")?, heal)),
            ],
            rank_turns: [
                rank as u8,
                or(num("raw_f78")?, rank) as u8,
                or(num("raw_f86")?, rank) as u8,
            ],
            ailment_turns: [
                ailment_max as u8,
                or(num("raw_f76")?, ailment_max) as u8,
                or(num("raw_f84")?, ailment_max) as u8,
            ],
            crit_rank: [crit as u8, crit as u8, or(num("raw_f81")?, crit) as u8],
            ailment_chance: [
                ailment_chance as u8,
                ailment_chance as u8,
                or(num("raw_f82")?, ailment_chance) as u8,
            ],
            stat_chance: [
                stat_chance as u8,
                stat_chance as u8,
                or(num("raw_f83")?, stat_chance) as u8,
            ],
            drain_percent: [
                signed_byte(drain),
                signed_byte(drain),
                signed_byte(or(num("raw_f87")?, drain)),
            ],
        })
    }

    /// VERIFIED: power of the move in this style.
    pub fn power(&self, style: Style) -> u16 {
        self.power[style.slot()]
    }
    /// VERIFIED: accuracy in this style (agile keeps the normal accuracy; 101 never misses).
    pub fn accuracy(&self, style: Style) -> u8 {
        self.accuracy[style.slot()]
    }
    /// VERIFIED raw action points added to the user's wait, before the agile weakening.
    pub fn raw_self_points(&self, style: Style) -> i32 {
        self.self_points[style.slot()]
    }
    /// VERIFIED raw action points added to the target's wait, before the agile weakening.
    pub fn raw_target_points(&self, style: Style) -> i32 {
        self.target_points[style.slot()]
    }
    /// VERIFIED: points added to the user's wait. `agile_streak` = consecutive agile uses
    /// before this one.
    pub fn self_action_points(&self, style: Style, agile_streak: u8) -> i32 {
        let value = self.raw_self_points(style);
        if style == Style::Agile && value < 0 {
            weaken_agile(value, self.raw_self_points(Style::Normal), agile_streak)
        } else {
            value
        }
    }
    /// VERIFIED: points added to the target's wait (not applied when the user targets itself).
    pub fn target_action_points(&self, style: Style, agile_streak: u8) -> i32 {
        let value = self.raw_target_points(style);
        if style == Style::Agile && value > 0 {
            weaken_agile(value, self.raw_target_points(Style::Normal), agile_streak)
        } else {
            value
        }
    }
    pub fn heal_percent(&self, style: Style) -> i8 {
        self.heal_percent[style.slot()]
    }
    pub fn rank_turns(&self, style: Style) -> u8 {
        self.rank_turns[style.slot()]
    }
    pub fn ailment_turns(&self, style: Style) -> u8 {
        self.ailment_turns[style.slot()]
    }
    pub fn crit_rank(&self, style: Style) -> u8 {
        self.crit_rank[style.slot()]
    }
    pub fn ailment_chance(&self, style: Style) -> u8 {
        self.ailment_chance[style.slot()]
    }
    pub fn stat_chance(&self, style: Style) -> u8 {
        self.stat_chance[style.slot()]
    }
    pub fn drain_percent(&self, style: Style) -> i8 {
        self.drain_percent[style.slot()]
    }
}

/// VERIFIED consecutive-agile weakening: `value / 3^min(streak, 4)` in single precision,
/// rounded away from zero, unless that is smaller in size than the normal-style value.
pub fn weaken_agile(value: i32, normal_value: i32, agile_streak: u8) -> i32 {
    const POWERS: [f32; 5] = [1.0, 3.0, 9.0, 27.0, 81.0];
    debug_assert_eq!(POWERS[1], AGILE_STREAK_BASE);
    let divisor = POWERS[usize::from(agile_streak.min(AGILE_STREAK_CAP))];
    let scaled = value as f32 / divisor;
    let rounded = if scaled < 0.0 {
        scaled.floor()
    } else {
        scaled.ceil()
    } as i32;
    if rounded.abs() >= normal_value.abs() {
        rounded
    } else {
        normal_value
    }
}

/// VERIFIED wait (ticks until the next action) of a combatant with this agility.
pub fn base_wait(agility: u32) -> i32 {
    let ticks = (WAIT_BASE - f64::from(agility).powf(f64::from(WAIT_EXPONENT))) as f32;
    ticks.ceil() as i32
}

/// Per-style move data for every move, loaded from the two checked-in move sheets.
#[derive(Debug, Clone, Default)]
pub struct MoveStyleTable {
    by_name: HashMap<String, MoveStyleData>,
}

fn parse_sheet(text: &str) -> Result<Vec<HashMap<String, String>>, String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty());
    let header = lines.next().ok_or("empty sheet")?;
    let columns: Vec<String> = header
        .split('\t')
        .map(|h| h.split(':').next().unwrap_or("").to_string())
        .collect();
    Ok(lines
        .map(|line| {
            columns
                .iter()
                .cloned()
                .zip(line.split('\t').map(str::to_string))
                .collect()
        })
        .collect())
}

impl MoveStyleTable {
    /// Load `pml_moves.tsv` and `pml_move_effects.tsv` from a data root (`data/`).
    pub fn load(data_root: impl AsRef<Path>) -> Result<Self, String> {
        let root = data_root.as_ref();
        let read = |name: &str| {
            std::fs::read_to_string(root.join(name)).map_err(|e| format!("{name}: {e}"))
        };
        Self::from_tsv(&read("pml_moves.tsv")?, &read("pml_move_effects.tsv")?)
    }

    pub fn from_tsv(moves_tsv: &str, effects_tsv: &str) -> Result<Self, String> {
        let mut effects: HashMap<String, HashMap<String, String>> = HashMap::new();
        for row in parse_sheet(effects_tsv)? {
            if let Some(name) = row.get("move_name").cloned() {
                effects.entry(name).or_insert(row);
            }
        }
        let mut by_name = HashMap::new();
        for mut row in parse_sheet(moves_tsv)? {
            let Some(name) = row.get("move_name").cloned() else {
                continue;
            };
            if by_name.contains_key(&name) {
                continue;
            }
            if let Some(fx) = effects.get(&name) {
                for (k, v) in fx {
                    row.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
            by_name.insert(name, MoveStyleData::from_move_row(&row)?);
        }
        Ok(Self { by_name })
    }

    pub fn get(&self, name: &str) -> Option<&MoveStyleData> {
        self.by_name.get(name)
    }
    pub fn len(&self) -> usize {
        self.by_name.len()
    }
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/// One combatant slot of the action-order queue (one per battle side here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueEntry {
    pub side: usize,
    pub combatant: usize,
    /// Ticks until this combatant acts.
    pub wait: i32,
    /// Wait computed at the previous step (speed changes adjust by the difference).
    pub last_base_wait: i32,
    /// Action points received since the last step.
    pub pending_points: i32,
    /// Entries with this flag are picked before all others (cause UNKNOWN; never set here).
    pub first: bool,
    pub agility: u32,
}

/// One line of the upcoming-turn list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnOrderEntry {
    pub side: usize,
    pub combatant: usize,
    /// Ticks from now until this turn.
    pub wait: i32,
}

/// The PLA action-order queue. See the module docs for the rules and their confidence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActionQueue {
    pub entries: Vec<QueueEntry>,
    /// Side that acted last (excluded on ties).
    pub last_actor: Option<usize>,
}

fn step(entries: &mut [QueueEntry], actor: Option<usize>, elapsed: i32) {
    let elapsed = elapsed.max(0);
    for entry in entries.iter_mut() {
        let wait = base_wait(entry.agility);
        if Some(entry.side) == actor {
            entry.wait = wait;
            entry.first = false;
        } else {
            entry.wait = entry.wait - elapsed + wait - entry.last_base_wait;
        }
        entry.last_base_wait = wait;
        entry.wait += entry.pending_points;
        entry.pending_points = 0;
    }
}

fn select(entries: &[QueueEntry], previous: Option<usize>) -> Option<usize> {
    let mut candidates: Vec<usize> = (0..entries.len()).filter(|&i| entries[i].first).collect();
    if candidates.is_empty() {
        candidates = (0..entries.len()).collect();
    }
    let min_wait = candidates.iter().map(|&i| entries[i].wait).min()?;
    candidates.retain(|&i| entries[i].wait == min_wait);
    if candidates.len() > 1 {
        if let Some(prev) = previous {
            candidates.retain(|&i| entries[i].side != prev);
        }
    }
    if candidates.len() > 1 {
        let fastest = candidates.iter().map(|&i| entries[i].agility).max()?;
        candidates.retain(|&i| entries[i].agility == fastest);
    }
    candidates.into_iter().min_by_key(|&i| entries[i].side)
}

impl ActionQueue {
    /// Queue from `(side, combatant, agility)` triples; every wait starts at 0 and one settle
    /// step without an actor gives each combatant its base wait (LIKELY game start).
    pub fn new(members: &[(usize, usize, u32)]) -> Self {
        let mut queue = Self {
            entries: members
                .iter()
                .map(|&(side, combatant, agility)| QueueEntry {
                    side,
                    combatant,
                    wait: 0,
                    last_base_wait: 0,
                    pending_points: 0,
                    first: false,
                    agility,
                })
                .collect(),
            last_actor: None,
        };
        queue.settle(None);
        queue
    }

    fn entry_mut(&mut self, side: usize) -> Option<&mut QueueEntry> {
        self.entries.iter_mut().find(|e| e.side == side)
    }

    /// Add action points to a side's pending total (applied at the next settle).
    pub fn add_points(&mut self, side: usize, points: i32) {
        if let Some(entry) = self.entry_mut(side) {
            entry.pending_points += points;
        }
    }

    /// Refresh the combatant and agility of a side (done before every settle).
    pub fn set_member(&mut self, side: usize, combatant: usize, agility: u32) {
        if let Some(entry) = self.entry_mut(side) {
            entry.combatant = combatant;
            entry.agility = agility;
        }
    }

    /// The step after `actor` (a side) acted: its wait restarts, the others advance by the
    /// ticks it waited, pending points apply, stored waits are clamped at 0.
    pub fn settle(&mut self, actor: Option<usize>) {
        let elapsed = actor
            .and_then(|side| self.entries.iter().find(|e| e.side == side))
            .map_or(0, |e| e.wait);
        step(&mut self.entries, actor, elapsed);
        for entry in &mut self.entries {
            entry.wait = entry.wait.max(0);
        }
        self.last_actor = actor;
    }

    /// The side whose turn is next.
    pub fn next(&self) -> Option<&QueueEntry> {
        select(&self.entries, self.last_actor).map(|i| &self.entries[i])
    }

    /// The next `count` turns (the game lists [`TURN_ORDER_PREVIEW_LEN`]), simulated with base
    /// waits only: pending points are applied at the first simulated step.
    pub fn preview(&self, count: usize) -> Vec<TurnOrderEntry> {
        let mut entries = self.entries.clone();
        let mut previous = self.last_actor;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let Some(i) = select(&entries, previous) else {
                break;
            };
            let picked = entries[i].clone();
            out.push(TurnOrderEntry {
                side: picked.side,
                combatant: picked.combatant,
                wait: picked.wait,
            });
            step(&mut entries, Some(picked.side), picked.wait);
            previous = Some(picked.side);
        }
        out
    }
}

/// Opt-in switches for [`Battle::play_turn_styled`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StyleRules {
    /// Extra PP spent by an agile or strong use. UNKNOWN in the game; default 0 (ASSUMED).
    pub extra_pp_per_style: u8,
    /// Use the strong-style critical rank (`raw_f81`, LIKELY) instead of the slot's rank.
    pub strong_crit_rank: bool,
}

/// An action with its chosen style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyledAction {
    pub action: Action,
    pub style: Style,
}

impl From<Action> for StyledAction {
    fn from(action: Action) -> Self {
        Self {
            action,
            style: Style::Normal,
        }
    }
}

/// Style results, kept apart from `Event` (matched exhaustively elsewhere).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleEvent {
    /// The move was used in this style (`streak` = consecutive agile uses before it).
    StyleUsed {
        actor: usize,
        style: Style,
        streak: u8,
    },
    /// The requested style is not available for this move; it was used normally.
    StyleUnavailable { actor: usize, style: Style },
    /// Action points added to a combatant's wait (negative = sooner).
    ActionPoints { combatant: usize, points: i32 },
}

/// Private per-battle state for this module.
#[derive(Debug, Clone, Default)]
pub(crate) struct StyleState {
    table: Option<MoveStyleTable>,
    rules: StyleRules,
    queue: Option<ActionQueue>,
    agile_streak: Vec<u8>,
    fixated: Vec<Option<String>>,
    events: Vec<StyleEvent>,
}

/// Multiply Atk, Def, SpA, SpD and Spe by the Alpha coefficient (truncation ASSUMED).
pub fn apply_alpha_power(combatant: &mut Combatant) {
    for stat in [
        &mut combatant.attack,
        &mut combatant.defense,
        &mut combatant.special_attack,
        &mut combatant.special_defense,
        &mut combatant.speed,
    ] {
        *stat = (*stat as f32 * ALPHA_POWER_COEF) as u32;
    }
}

impl Battle {
    pub fn set_move_styles(&mut self, table: MoveStyleTable) {
        self.styles.table = Some(table);
    }
    pub fn move_styles(&self) -> Option<&MoveStyleTable> {
        self.styles.table.as_ref()
    }
    pub fn set_style_rules(&mut self, rules: StyleRules) {
        self.styles.rules = rules;
    }
    pub fn take_style_events(&mut self) -> Vec<StyleEvent> {
        std::mem::take(&mut self.styles.events)
    }
    /// Consecutive agile uses of this combatant so far.
    pub fn agile_streak(&self, combatant: usize) -> u8 {
        self.styles
            .agile_streak
            .get(combatant)
            .copied()
            .unwrap_or(0)
    }

    /// Mark (or clear) the move a combatant is fixated on. The trigger is UNKNOWN in the game.
    pub fn set_fixated(&mut self, combatant: usize, move_name: Option<String>) {
        if self.styles.fixated.len() <= combatant {
            self.styles.fixated.resize(combatant + 1, None);
        }
        self.styles.fixated[combatant] = move_name;
    }
    pub fn fixated_on(&self, combatant: usize) -> Option<&str> {
        self.styles.fixated.get(combatant)?.as_deref()
    }
    /// VERIFIED Fixated percentages, applied after the other damage factors. No-op when no
    /// combatant is fixated. Applying them after the minimum-1 clamp gives the same result
    /// because both factors are above 1.
    pub(super) fn fixated_damage(
        &self,
        attacker: usize,
        defender: usize,
        move_name: &str,
        damage: u32,
    ) -> u32 {
        if damage == 0 || self.styles.fixated.is_empty() {
            return damage;
        }
        let mut damage = damage;
        if self.fixated_on(attacker) == Some(move_name) {
            damage = damage * FIXATED_DAMAGE_DEALT_PERCENT / 100;
        }
        if self.fixated_on(defender).is_some() {
            damage = damage * FIXATED_DAMAGE_TAKEN_PERCENT / 100;
        }
        damage
    }

    fn queue_agility(&self, combatant: usize) -> u32 {
        // The queue reads the VERIFIED agility without the reversal.
        agility(&self.combatants[combatant], self.weather, false)
    }

    fn sync_queue(&mut self) {
        let members: Vec<(usize, usize, u32)> = (0..self.sides.len())
            .map(|side| {
                let c = self.active[side];
                (side, c, self.queue_agility(c))
            })
            .collect();
        if let Some(queue) = self.styles.queue.as_mut() {
            for (side, c, agi) in members {
                queue.set_member(side, c, agi);
            }
        }
    }

    /// Start the action-order queue from the active combatants (one entry per side).
    pub fn enable_action_queue(&mut self) {
        let members: Vec<(usize, usize, u32)> = (0..self.sides.len())
            .map(|side| {
                let c = self.active[side];
                (side, c, self.queue_agility(c))
            })
            .collect();
        self.styles.queue = Some(ActionQueue::new(&members));
    }
    pub fn action_queue(&self) -> Option<&ActionQueue> {
        self.styles.queue.as_ref()
    }
    /// The combatant whose turn is next (queue enabled only).
    pub fn next_actor(&self) -> Option<usize> {
        let queue = self.styles.queue.as_ref()?;
        let mut copy = queue.clone();
        for entry in &mut copy.entries {
            entry.combatant = self.active[entry.side];
        }
        copy.next().map(|e| e.combatant)
    }
    /// Upcoming turns for the HUD list (queue enabled only).
    pub fn turn_order_preview(&self, count: usize) -> Vec<TurnOrderEntry> {
        let Some(queue) = self.styles.queue.as_ref() else {
            return Vec::new();
        };
        let mut copy = queue.clone();
        for entry in &mut copy.entries {
            entry.combatant = self.active[entry.side];
        }
        copy.preview(count)
    }
    /// End a queue step for a combatant that acted without a move (switch, item, capture):
    /// it waits its base wait again (cost of non-move actions ASSUMED to be zero points).
    pub fn advance_action_queue(&mut self, actor: usize) {
        let Some(side) = self.sides.iter().position(|s| s.contains(&actor)) else {
            return;
        };
        self.sync_queue();
        if let Some(queue) = self.styles.queue.as_mut() {
            queue.settle(Some(side));
        }
    }

    /// `play_turn` with a style per action. With every style `Normal`, no move-style table
    /// and no queue, the result is identical to `play_turn`. Styled power and accuracy apply
    /// for the move only; action points and the queue step follow each move that was used.
    pub fn play_turn_styled(
        &mut self,
        actions: &[StyledAction],
        rng: &mut impl BattleRng,
    ) -> Vec<Event> {
        let n = self.combatants.len();
        if self.styles.agile_streak.len() < n {
            self.styles.agile_streak.resize(n, 0);
        }
        let rules = self.styles.rules;
        // (actor, slot, style, original power, accuracy, crit rank, pp before)
        let mut chosen = Vec::new();
        for sa in actions {
            let a = sa.action;
            let Some(slot) = self
                .combatants
                .get(a.actor)
                .and_then(|c| c.moves.get(a.move_slot))
            else {
                continue;
            };
            let data = self
                .styles
                .table
                .as_ref()
                .and_then(|t| t.get(&slot.data.name))
                .cloned();
            let mut style = sa.style;
            if style != Style::Normal && !data.as_ref().is_some_and(|d| d.styles_usable) {
                self.styles.events.push(StyleEvent::StyleUnavailable {
                    actor: a.actor,
                    style,
                });
                style = Style::Normal;
            }
            let original = (
                slot.data.power,
                slot.data.accuracy,
                slot.critical_rank,
                slot.pp,
            );
            if let (Some(d), true) = (&data, style != Style::Normal) {
                let slot = &mut self.combatants[a.actor].moves[a.move_slot];
                slot.data.power = d.power(style);
                slot.data.accuracy = d.accuracy(style);
                if rules.strong_crit_rank && style == Style::Strong {
                    slot.critical_rank = d.crit_rank(style);
                }
            }
            chosen.push((a.actor, a.move_slot, style, original, data));
        }
        let plain: Vec<Action> = actions.iter().map(|sa| sa.action).collect();
        let alive_before: Vec<bool> = self.combatants.iter().map(Combatant::is_alive).collect();
        let events = self.play_turn(&plain, rng);
        // Restore the slot's normal values; PP changes made by the turn are kept.
        for (actor, slot, _, (power, accuracy, crit, _), _) in &chosen {
            let s = &mut self.combatants[*actor].moves[*slot];
            s.data.power = *power;
            s.data.accuracy = *accuracy;
            s.critical_rank = *crit;
        }
        // Which actions were used as moves, in the order the turn ran them.
        let mut order: Vec<usize> = Vec::new();
        for event in &events {
            let actor = match event {
                Event::UsedMove { actor, .. }
                | Event::Missed { actor, .. }
                | Event::UnableToAct { actor, .. } => *actor,
                _ => continue,
            };
            if !order.contains(&actor) {
                order.push(actor);
            }
        }
        for (actor, ..) in &chosen {
            if !order.contains(actor) && alive_before.get(*actor) == Some(&true) {
                order.push(*actor);
            }
        }
        for actor in order {
            let Some(&(_, slot, style, (_, _, _, pp_before), ref data)) =
                chosen.iter().find(|c| c.0 == actor)
            else {
                continue;
            };
            let pp_now = self.combatants[actor].moves[slot].pp;
            let used_move = pp_now < pp_before
                || events.iter().any(|e| {
                    matches!(e, Event::UsedMove { actor: a, .. } | Event::Missed { actor: a, .. } if *a == actor)
                });
            if used_move {
                if style != Style::Normal && rules.extra_pp_per_style > 0 {
                    let s = &mut self.combatants[actor].moves[slot];
                    s.pp = s.pp.saturating_sub(rules.extra_pp_per_style);
                }
                let streak = self.styles.agile_streak[actor];
                self.styles.events.push(StyleEvent::StyleUsed {
                    actor,
                    style,
                    streak,
                });
                if let Some(d) = data {
                    let target = events
                        .iter()
                        .find_map(|e| match e {
                            Event::UsedMove {
                                actor: a, target, ..
                            }
                            | Event::Missed {
                                actor: a, target, ..
                            } if *a == actor => Some(*target),
                            _ => None,
                        })
                        .or_else(|| self.target_for(actor));
                    let own = d.self_action_points(style, streak);
                    self.add_action_points(actor, own);
                    if let Some(t) = target.filter(|&t| t != actor) {
                        if self.combatants[t].is_alive() {
                            self.add_action_points(t, d.target_action_points(style, streak));
                        }
                    }
                }
                self.styles.agile_streak[actor] = if style == Style::Agile {
                    streak.saturating_add(1)
                } else {
                    0
                };
            }
            self.advance_action_queue(actor);
        }
        events
    }

    fn add_action_points(&mut self, combatant: usize, points: i32) {
        if points == 0 {
            return;
        }
        self.styles
            .events
            .push(StyleEvent::ActionPoints { combatant, points });
        let Some(side) = self.sides.iter().position(|s| s.contains(&combatant)) else {
            return;
        };
        if let Some(queue) = self.styles.queue.as_mut() {
            queue.add_points(side, points);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::BattleData;
    use std::path::PathBuf;

    struct Fixed(Vec<u32>);
    impl BattleRng for Fixed {
        fn next_u32(&mut self) -> u32 {
            if self.0.is_empty() {
                1
            } else {
                self.0.remove(0)
            }
        }
    }
    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }
    fn table() -> MoveStyleTable {
        MoveStyleTable::load(root()).unwrap()
    }
    fn duel(a: (&str, &[&str], u8), b: (&str, &[&str], u8)) -> Battle {
        let d = BattleData::load(root()).unwrap();
        let mon = |(species, moves, level): (&str, &[&str], u8)| {
            Combatant::from_base(
                d.species_row(species).unwrap().unwrap(),
                level,
                moves
                    .iter()
                    .map(|m| d.move_row(m).unwrap().unwrap())
                    .collect(),
            )
        };
        Battle::new(
            vec![mon(a), mon(b)],
            vec![vec![0], vec![1]],
            d.type_chart().clone(),
        )
        .unwrap()
    }
    fn entry(side: usize, agility: u32) -> (usize, usize, u32) {
        (side, side, agility)
    }

    #[test]
    fn style_indices_follow_the_game_readers() {
        assert_eq!(Style::Strong.game_index(), 1);
        assert_eq!(Style::Agile.game_index(), 2);
        assert_eq!(Style::from_game_index(2), Some(Style::Agile));
        assert_eq!(Style::from_game_index(3), None);
    }

    #[test]
    fn move_sheets_give_style_power_accuracy_and_action_points() {
        let t = table();
        let tackle = t.get("tackle").unwrap();
        assert!(tackle.styles_usable);
        assert_eq!(
            Style::ALL.map(|s| tackle.power(s)),
            [40, 30, 50],
            "agile lower, strong higher"
        );
        assert_eq!(Style::ALL.map(|s| tackle.raw_self_points(s)), [0, 0, 5]);
        assert_eq!(Style::ALL.map(|s| tackle.raw_target_points(s)), [0, 3, 0]);
        let hydro = t.get("hydro-pump").unwrap();
        assert_eq!(Style::ALL.map(|s| hydro.accuracy(s)), [85, 85, 95]);
        let quick = t.get("quick-attack").unwrap();
        assert_eq!(Style::ALL.map(|s| quick.raw_self_points(s)), [-4, -4, 2]);
        let beam = t.get("hyper-beam").unwrap();
        assert_eq!(Style::ALL.map(|s| beam.raw_self_points(s)), [5, 2, 8]);
        let bite = t.get("bite").unwrap();
        assert_eq!(Style::ALL.map(|s| bite.raw_target_points(s)), [3, 5, 3]);
        let recover = t.get("recover").unwrap();
        assert_eq!(Style::ALL.map(|s| recover.heal_percent(s)), [50, 30, 70]);
        let sd = t.get("swords-dance").unwrap();
        assert_eq!(Style::ALL.map(|s| sd.rank_turns(s)), [5, 4, 7]);
        assert_eq!(Style::ALL.map(|s| sd.raw_self_points(s)), [-3, -4, 5]);
        // Signed heal bytes: Self-Destruct costs 80 % normally, 70 % agile, 90 % strong.
        let boom = t.get("self-destruct").unwrap();
        assert_eq!(Style::ALL.map(|s| boom.heal_percent(s)), [-80, -70, -90]);
        assert!(!t.get("giga-drain").unwrap().styles_usable);
    }

    #[test]
    fn consecutive_agile_uses_lose_their_bonus() {
        // Swords Dance agile self -4, normal -3.
        assert_eq!(weaken_agile(-4, -3, 0), -4);
        // -4/3 = -1.33 -> -2, smaller than |-3| -> normal value.
        assert_eq!(weaken_agile(-4, -3, 1), -3);
        // Target +3 (normal 0): 3/3 = 1, 3/9 = 0.33 -> 1 (away from zero), capped at 3^4.
        assert_eq!(weaken_agile(3, 0, 1), 1);
        assert_eq!(weaken_agile(3, 0, 2), 1);
        assert_eq!(
            weaken_agile(5, 3, 1),
            3,
            "Bite agile 5/3 -> 2 < 3 -> normal 3"
        );
        assert_eq!(weaken_agile(-9, 0, 9), -1);
        let t = table();
        let tackle = t.get("tackle").unwrap();
        assert_eq!(tackle.target_action_points(Style::Agile, 0), 3);
        assert_eq!(tackle.target_action_points(Style::Agile, 1), 1);
        // Strong values are never weakened.
        assert_eq!(tackle.self_action_points(Style::Strong, 3), 5);
    }

    #[test]
    fn base_wait_matches_hand_computed_values() {
        // 100^0.4 = 6.31 -> 16 - 6.31 = 9.69 -> 10; 50^0.4 = 4.78 -> 11.22 -> 12.
        assert_eq!(base_wait(100), 10);
        assert_eq!(base_wait(50), 12);
        // 200^0.4 = 8.33 -> 7.67 -> 8; 300^0.4 = 9.79 -> 6.21 -> 7; 0 -> 16.
        assert_eq!(base_wait(200), 8);
        assert_eq!(base_wait(300), 7);
        assert_eq!(base_wait(0), 16);
        // 1024^0.4 = 16 exactly in the formula's terms: nothing left to wait.
        assert!(base_wait(1024) <= 0);
    }

    #[test]
    fn queue_starts_with_base_waits_and_picks_the_fastest() {
        let q = ActionQueue::new(&[entry(0, 100), entry(1, 50)]);
        assert_eq!(q.entries[0].wait, 10);
        assert_eq!(q.entries[1].wait, 12);
        assert_eq!(q.next().unwrap().side, 0);
        // Hand-computed list: A@10, B@12 -> after A (elapsed 10): A 10, B 2 -> B;
        // after B (2): A 8, B 12 -> A; after A (8): A 10, B 4 -> B; then A 6 vs B 12 -> A;
        // after A (6): A 10, B 6 -> B.
        let order: Vec<(usize, i32)> = q.preview(6).iter().map(|e| (e.side, e.wait)).collect();
        assert_eq!(order, vec![(0, 10), (1, 2), (0, 8), (1, 4), (0, 6), (1, 6)]);
    }

    #[test]
    fn strong_style_lets_an_equal_speed_foe_act_twice() {
        // Equal agility 100 (wait 10). Tie at 10: lower side acts first.
        let mut q = ActionQueue::new(&[entry(0, 100), entry(1, 100)]);
        assert_eq!(q.next().unwrap().side, 0);
        // Side 0 uses a strong move: +5 to itself.
        q.add_points(0, 5);
        q.settle(Some(0));
        assert_eq!((q.entries[0].wait, q.entries[1].wait), (15, 0));
        let order: Vec<usize> = q.preview(4).iter().map(|e| e.side).collect();
        // B@0, then B (10) before A (15-0=15), then A (5), then B.
        assert_eq!(order, vec![1, 1, 0, 1]);
    }

    #[test]
    fn agile_style_delays_the_target_and_ties_skip_the_last_actor() {
        let mut q = ActionQueue::new(&[entry(0, 100), entry(1, 100)]);
        // Side 0 uses an agile tackle: +3 to the target.
        q.add_points(1, 3);
        q.settle(Some(0));
        assert_eq!((q.entries[0].wait, q.entries[1].wait), (10, 3));
        assert_eq!(q.next().unwrap().side, 1);
        // Equal waits: the side that acted last is skipped.
        let mut q = ActionQueue::new(&[entry(0, 100), entry(1, 100)]);
        q.settle(Some(0));
        q.entries[1].wait = 10;
        q.last_actor = Some(1);
        assert_eq!(q.next().unwrap().side, 0);
        q.last_actor = Some(0);
        assert_eq!(q.next().unwrap().side, 1);
    }

    #[test]
    fn speed_changes_shift_the_wait_by_the_difference() {
        let mut q = ActionQueue::new(&[entry(0, 100), entry(1, 100)]);
        q.settle(Some(0)); // A 10, B 0
        q.set_member(1, 1, 50); // B slowed to agility 50 (base wait 12)
        q.settle(Some(1)); // B acts (elapsed 0): B restarts at 12, A 10 - 0 + 0 = 10
        assert_eq!((q.entries[0].wait, q.entries[1].wait), (10, 12));
        q.set_member(0, 0, 50); // A slowed while B waits
        q.settle(Some(0)); // A acts after 10: A 12; B 12 - 10 + (12 - 12) = 2
        assert_eq!((q.entries[0].wait, q.entries[1].wait), (12, 2));
        // A speeds back up while waiting: its wait moves by 10 - 12 = -2.
        q.set_member(0, 0, 100);
        q.settle(Some(1)); // B acts after 2: B 12; A 12 - 2 + (10 - 12) = 8
        assert_eq!((q.entries[0].wait, q.entries[1].wait), (8, 12));
    }

    #[test]
    fn styled_turn_without_styles_equals_plain_turn() {
        let mut a = duel(
            ("pikachu", &["thunder-shock"], 30),
            ("starly", &["tackle"], 20),
        );
        let mut b = a.clone();
        let actions = [
            Action {
                actor: 0,
                move_slot: 0,
            },
            Action {
                actor: 1,
                move_slot: 0,
            },
        ];
        let rolls = vec![3, 40, 7, 1, 2, 60, 5, 9, 11, 13];
        let plain = a.play_turn(&actions, &mut Fixed(rolls.clone()));
        let styled = b.play_turn_styled(&actions.map(StyledAction::from), &mut Fixed(rolls));
        assert_eq!(plain, styled);
        assert_eq!(a.combatants[1].hp, b.combatants[1].hp);
        assert!(b.take_style_events().iter().all(|e| matches!(
            e,
            StyleEvent::StyleUsed {
                style: Style::Normal,
                ..
            }
        )));
    }

    #[test]
    fn styled_power_changes_damage_and_restores_the_slot() {
        let make = || {
            let mut b = duel(("eevee", &["tackle"], 30), ("bidoof", &["tackle"], 30));
            b.set_move_styles(table());
            b.set_pla_damage_formula(true);
            b
        };
        let damage = |style: Style| {
            let mut b = make();
            let before = b.combatants[1].hp;
            // crit roll 1 (no crit), accuracy 0, damage roll 0 (100 %).
            b.play_turn_styled(
                &[StyledAction {
                    action: Action {
                        actor: 0,
                        move_slot: 0,
                    },
                    style,
                }],
                &mut Fixed(vec![0, 1, 0]),
            );
            assert_eq!(b.combatants[0].moves[0].data.power, 40, "slot restored");
            assert_eq!(b.combatants[0].moves[0].pp, 29, "one PP whatever the style");
            before - b.combatants[1].hp
        };
        let (agile, normal, strong) = (
            damage(Style::Agile),
            damage(Style::Normal),
            damage(Style::Strong),
        );
        assert!(
            agile < normal && normal < strong,
            "{agile} {normal} {strong}"
        );
    }

    #[test]
    fn styled_turns_drive_the_queue_and_agile_streak() {
        let mut b = duel(
            ("eevee", &["tackle", "quick-attack"], 30),
            ("bidoof", &["tackle"], 30),
        );
        b.set_move_styles(table());
        b.combatants[0].speed = 100;
        b.combatants[1].speed = 100;
        b.enable_action_queue();
        assert_eq!(b.next_actor(), Some(0));
        let agile = |slot| StyledAction {
            action: Action {
                actor: 0,
                move_slot: slot,
            },
            style: Style::Agile,
        };
        b.play_turn_styled(&[agile(0)], &mut Fixed(vec![0, 1, 99]));
        // Eevee waits 10 again; Bidoof was pushed back by 3 -> 0 + 3 = 3.
        let q = b.action_queue().unwrap();
        assert_eq!((q.entries[0].wait, q.entries[1].wait), (10, 3));
        assert_eq!(b.agile_streak(0), 1);
        assert_eq!(b.next_actor(), Some(1));
        assert!(b.take_style_events().contains(&StyleEvent::ActionPoints {
            combatant: 1,
            points: 3
        }));
        // Bidoof acts normally after 3 ticks: Eevee 7.
        b.play_turn_styled(
            &[StyledAction::from(Action {
                actor: 1,
                move_slot: 0,
            })],
            &mut Fixed(vec![0, 1, 99]),
        );
        let q = b.action_queue().unwrap();
        assert_eq!((q.entries[0].wait, q.entries[1].wait), (7, 10));
        // Second agile use in a row: target bonus weakened to 1.
        b.play_turn_styled(&[agile(0)], &mut Fixed(vec![0, 1, 99]));
        let q = b.action_queue().unwrap();
        assert_eq!((q.entries[0].wait, q.entries[1].wait), (10, 4));
        assert_eq!(b.agile_streak(0), 2);
        // A normal move resets the streak.
        b.play_turn_styled(
            &[StyledAction::from(Action {
                actor: 0,
                move_slot: 1,
            })],
            &mut Fixed(vec![0, 1, 99]),
        );
        assert_eq!(b.agile_streak(0), 0);
        assert_eq!(b.turn_order_preview(TURN_ORDER_PREVIEW_LEN).len(), 6);
    }

    #[test]
    fn style_on_a_move_without_styles_falls_back_to_normal() {
        let mut b = duel(("eevee", &["tackle"], 30), ("bidoof", &["tackle"], 30));
        let mut t = table();
        t.by_name.get_mut("tackle").unwrap().styles_usable = false;
        b.set_move_styles(t);
        b.play_turn_styled(
            &[StyledAction {
                action: Action {
                    actor: 0,
                    move_slot: 0,
                },
                style: Style::Strong,
            }],
            &mut Fixed(vec![0, 1, 0]),
        );
        let events = b.take_style_events();
        assert!(events.contains(&StyleEvent::StyleUnavailable {
            actor: 0,
            style: Style::Strong
        }));
    }

    #[test]
    fn fixated_and_alpha_multipliers_are_opt_in() {
        let mut b = duel(("eevee", &["tackle"], 30), ("bidoof", &["tackle"], 30));
        assert_eq!(b.fixated_damage(0, 1, "tackle", 20), 20);
        b.set_fixated(0, Some("tackle".into()));
        assert_eq!(b.fixated_damage(0, 1, "tackle", 20), 30);
        assert_eq!(b.fixated_damage(0, 1, "quick-attack", 20), 20);
        // Fixated defender takes x133/100.
        assert_eq!(b.fixated_damage(1, 0, "tackle", 30), 39);
        assert_eq!(b.fixated_damage(1, 0, "tackle", 0), 0);
        let mut c = b.combatants[1].clone();
        c.attack = 41;
        c.speed = 33;
        let hp = c.max_hp;
        apply_alpha_power(&mut c);
        assert_eq!((c.attack, c.speed, c.max_hp), (61, 49, hp));
    }
}
