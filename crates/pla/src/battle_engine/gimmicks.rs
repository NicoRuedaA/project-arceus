//! Main-series battle gimmicks: Mega Evolution, Z-Moves, Dynamax and Terastallization.
//!
//! None of these exist in Legends: Arceus. This module is a port extension, not recovered
//! game behaviour: nothing here is VERIFIED against the binary and nothing feeds the
//! function-progress ledger. Rules follow WikiDex ("Mecánica de combate" and the page of each
//! mechanic, read 2026-10-16). Numbers WikiDex leaves out come from the Pokémon Showdown
//! simulator (`sim/dex-moves.ts`) and are marked SHOWDOWN. Engine choices are marked ASSUMED.
//!
//! Everything is opt-in, like `styles.rs`: a `Battle` that never calls
//! [`Battle::enable_gimmicks`] behaves exactly as before.
//!
//! * Port rule: **one gimmick kind per battle**. [`Battle::enable_gimmicks`] either fixes the
//!   kind up front or lets the first activation lock it; any other kind is then refused.
//! * Each side activates the battle's gimmick once (WikiDex: one Mega Evolution per side, one
//!   Z-Move and one Dynamax per trainer, Tera once per battle through the orb).
//! * Activation happens before the turn's moves, fastest activator first (WikiDex, Tera page).
//!   The new stats already count for that turn's order (ASSUMED, later-generation behaviour).
//! * Mega Evolution: needs a Mega Stone ([`Battle::give_mega_stone`]) and a species in
//!   [`MEGA_FORMS`]; +100 base stats spread per form, HP unchanged, new types where the form
//!   changes them; lasts until the battle ends and survives switching. The new ability is not
//!   applied: this engine only runs two abilities (`abilities.rs`).
//! * Z-Move: needs a Z-Crystal of the move's type ([`Battle::give_z_crystal`]). Damaging moves
//!   only; power from [`z_move_power`]; no secondary effects, ignores accuracy (Z-Moves have
//!   accuracy 101 in the move data), one hit; costs 1 PP of the base move; hits through
//!   Protect for 1/4 of the damage. Status Z-Moves and species-exclusive Z-Moves are not
//!   implemented and are refused.
//! * Dynamax: max HP and HP x(1.5 + 0.05 x Dynamax level, default level 10); lasts 3 of the
//!   user's actions (ASSUMED reading of "3 turns" for the PLA action queue) or until it
//!   switches out or faints. Damaging moves become Max Moves ([`max_move_power`], SHOWDOWN
//!   table; always hit; 1/4 damage through Protect, SHOWDOWN; a per-type side effect from
//!   [`max_effect`]); status moves become Max Guard, which also stops Max Moves. On reverting,
//!   HP scales back, rounded up. Gigantamax is not implemented (it needs form data).
//! * Terastallization: the Tera type replaces both types until the battle ends (switching
//!   keeps it). Main-series STAB: original types keep x1.5, a Tera type matching an original
//!   type gives x2, a new Tera type gives x1.5. This engine uses PLA's STAB of x1.25, so the
//!   ratio is kept: matching gives x1.25 x 2/1.5 = x5/3 (ASSUMED scaling). Tera-type moves
//!   below 60 power become 60 unless multi-hit or priority moves. Tera Blast takes the Tera type
//!   and turns physical when Attack is higher than Sp. Atk. The Tera type defaults to the first
//!   species type ([`Battle::set_tera_type`] overrides it). Stellar is not implemented.
//! * Not implemented: Primal Reversion, Ultra Burst, the Let's Go partner power, Move+ (Z-A)
//!   and the spin-off mechanics. PLA's own Agile/Strong styles live in `styles.rs`.

use super::effects::Stat;
use super::statuses::StatusEvent;
use super::styles::StyledAction;
use super::{calculate_stat, q12_mul, Action, Battle, BattleRng, BattleWeather, Event};
use crate::battle::{BattleMove, BattleSpecies};

/// Dynamax duration in actions of the Dynamaxed Pokémon (WikiDex: three turns).
pub const DYNAMAX_TURNS: u8 = 3;
/// Default Dynamax level (0-10). Levels are not tracked by this port (ASSUMED).
pub const DEFAULT_DYNAMAX_LEVEL: u8 = 10;
/// Damage share of a Z-Move or Max Move that hits through Protect (WikiDex, Z-Move page).
pub const PROTECT_PIERCE_Q12: u32 = 1024;
/// Minimum power of a Tera-type move (WikiDex).
pub const TERA_POWER_FLOOR: u16 = 60;
/// Extra factor on top of the engine STAB (x1.25) when the Tera type matches an original
/// type: 2/1.5 = 4/3 in Q12 (ASSUMED scaling of the main-series x2).
pub const TERA_MATCH_EXTRA_Q12: u32 = 5461;
/// The engine STAB factor (x1.25), restored for original-type moves after Terastallization.
pub const ENGINE_STAB_Q12: u32 = 0x1400;

/// One of the four mechanics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gimmick {
    Mega,
    ZMove,
    Dynamax,
    Terastal,
}

impl Gimmick {
    pub const ALL: [Gimmick; 4] = [
        Gimmick::Mega,
        Gimmick::ZMove,
        Gimmick::Dynamax,
        Gimmick::Terastal,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Gimmick::Mega => "Mega Evolution",
            Gimmick::ZMove => "Z-Move",
            Gimmick::Dynamax => "Dynamax",
            Gimmick::Terastal => "Terastallization",
        }
    }

    /// `mega`, `z`, `dynamax` / `dmax`, `tera` (case-insensitive, a few long forms too).
    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "mega" | "mega-evolution" => Some(Gimmick::Mega),
            "z" | "zmove" | "z-move" => Some(Gimmick::ZMove),
            "dynamax" | "dmax" => Some(Gimmick::Dynamax),
            "tera" | "terastal" | "terastallization" => Some(Gimmick::Terastal),
            _ => None,
        }
    }
}

/// A request to activate a gimmick for `actor` this turn (for a Z-Move, on the move of its
/// action this turn).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GimmickRequest {
    pub actor: usize,
    pub gimmick: Gimmick,
}

/// Why a gimmick was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// `enable_gimmicks` was never called.
    Disabled,
    /// The port rule: this battle already uses another gimmick.
    OtherGimmickThisBattle(Gimmick),
    AlreadyUsedBySide,
    NotActive,
    NoMegaStone,
    NoMegaForm,
    NoZCrystal,
    CrystalTypeMismatch,
    /// Status Z-Moves are not implemented.
    StatusZMove,
    /// No usable move for a Z-Move (bad slot or no PP).
    NoMove,
}

/// Side effect of a Max Move (WikiDex, "Movimiento Dinamax").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxEffect {
    LowerTarget(Stat),
    /// Raises the user's stat (Max Knuckle and Max Ooze raise every ally; singles = the user).
    RaiseUser(Stat),
    Weather(BattleWeather),
    /// A field effect the engine has no state for (terrains, sandstorm): logged only.
    Unsupported(&'static str),
}

/// Gimmick log, kept beside the stable `Event` stream. Drain with
/// [`Battle::take_gimmick_events`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GimmickEvent {
    /// The battle's gimmick kind is now fixed.
    Locked(Gimmick),
    MegaEvolved { combatant: usize, form: &'static str },
    Dynamaxed { combatant: usize, max_hp: u32 },
    DynamaxEnded { combatant: usize },
    Terastallized { combatant: usize, tera_type: String },
    ZMove { actor: usize, name: String, power: u16 },
    MaxMove { actor: usize, name: String, power: u16 },
    MaxGuard { actor: usize },
    ProtectPierced { attacker: usize, target: usize },
    MaxEffectApplied { actor: usize, target: usize, effect: MaxEffect },
    Refused { actor: usize, gimmick: Gimmick, reason: Refusal },
}

/// A Mega form: base-stat change and new types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MegaForm {
    pub species: &'static str,
    pub form: &'static str,
    /// Base-stat change in HP/Atk/Def/SpA/SpD/Spe order (WikiDex; HP never changes).
    pub delta: [i16; 6],
    /// New types when the form changes them.
    pub types: Option<[&'static str; 2]>,
}

const fn mega(species: &'static str, form: &'static str, delta: [i16; 6]) -> MegaForm {
    MegaForm { species, form, delta, types: None }
}

/// Mega forms listed on WikiDex for species in or near the Sinnoh/Hisui roster. Each delta
/// sums to +100.
pub static MEGA_FORMS: [MegaForm; 13] = [
    mega("alakazam", "Mega Alakazam", [0, 0, 20, 40, 10, 30]),
    mega("gengar", "Mega Gengar", [0, 0, 20, 40, 20, 20]),
    mega("steelix", "Mega Steelix", [0, 40, 30, 0, 30, 0]),
    mega("scizor", "Mega Scizor", [0, 20, 40, 10, 20, 10]),
    mega("heracross", "Mega Heracross", [0, 60, 40, 0, 10, -10]),
    mega("houndoom", "Mega Houndoom", [0, 0, 40, 30, 10, 20]),
    mega("tyranitar", "Mega Tyranitar", [0, 30, 40, 0, 20, 10]),
    mega("gardevoir", "Mega Gardevoir", [0, 20, 0, 40, 20, 20]),
    MegaForm {
        species: "lopunny",
        form: "Mega Lopunny",
        delta: [0, 60, 10, 0, 0, 30],
        types: Some(["Normal", "Fighting"]),
    },
    mega("garchomp", "Mega Garchomp", [0, 40, 20, 40, 10, -10]),
    mega("lucario", "Mega Lucario", [0, 35, 18, 25, 0, 22]),
    mega("abomasnow", "Mega Abomasnow", [0, 40, 30, 40, 20, -30]),
    mega("gallade", "Mega Gallade", [0, 40, 30, 0, 0, 30]),
];

pub fn mega_form(species: &str) -> Option<&'static MegaForm> {
    MEGA_FORMS.iter().find(|f| f.species == species)
}

/// Per type: Z-Move slug, Max Move slug and Max Move effect. The slugs are the move-sheet
/// names (`data/pml_moves.tsv`; Z-Moves carry a `--physical` / `--special` suffix there).
static TYPE_MOVES: [(&str, &str, &str, MaxEffect); 18] = [
    ("Normal", "breakneck-blitz", "max-strike", MaxEffect::LowerTarget(Stat::Speed)),
    ("Fighting", "all-out-pummeling", "max-knuckle", MaxEffect::RaiseUser(Stat::Attack)),
    ("Flying", "supersonic-skystrike", "max-airstream", MaxEffect::RaiseUser(Stat::Speed)),
    ("Poison", "acid-downpour", "max-ooze", MaxEffect::RaiseUser(Stat::SpecialAttack)),
    ("Ground", "tectonic-rage", "max-quake", MaxEffect::RaiseUser(Stat::SpecialDefense)),
    ("Rock", "continental-crush", "max-rockfall", MaxEffect::Unsupported("sandstorm")),
    ("Bug", "savage-spin-out", "max-flutterby", MaxEffect::LowerTarget(Stat::SpecialAttack)),
    ("Ghost", "never-ending-nightmare", "max-phantasm", MaxEffect::LowerTarget(Stat::Defense)),
    ("Steel", "corkscrew-crash", "max-steelspike", MaxEffect::RaiseUser(Stat::Defense)),
    ("Fire", "inferno-overdrive", "max-flare", MaxEffect::Weather(BattleWeather::Sun)),
    ("Water", "hydro-vortex", "max-geyser", MaxEffect::Weather(BattleWeather::Rain)),
    ("Grass", "bloom-doom", "max-overgrowth", MaxEffect::Unsupported("grassy terrain")),
    ("Electric", "gigavolt-havoc", "max-lightning", MaxEffect::Unsupported("electric terrain")),
    ("Psychic", "shattered-psyche", "max-mindstorm", MaxEffect::Unsupported("psychic terrain")),
    // Hail in the main series; the engine's closest weather is snow (ASSUMED).
    ("Ice", "subzero-slammer", "max-hailstorm", MaxEffect::Weather(BattleWeather::Snow)),
    ("Dragon", "devastating-drake", "max-wyrmwind", MaxEffect::LowerTarget(Stat::Attack)),
    ("Dark", "black-hole-eclipse", "max-darkness", MaxEffect::LowerTarget(Stat::SpecialDefense)),
    ("Fairy", "twinkle-tackle", "max-starfall", MaxEffect::Unsupported("misty terrain")),
];

fn type_moves(move_type: &str) -> Option<&'static (&'static str, &'static str, &'static str, MaxEffect)> {
    TYPE_MOVES.iter().find(|t| t.0 == move_type)
}

/// Max Move side effect for a move type.
pub fn max_effect(move_type: &str) -> Option<MaxEffect> {
    type_moves(move_type).map(|t| t.3)
}

/// Z-Moves with a fixed power instead of the table (WikiDex).
const Z_POWER_EXCEPTIONS: [(&str, u16); 7] = [
    ("hex", 160),
    ("mega-drain", 120),
    ("weather-ball", 160),
    ("core-enforcer", 140),
    ("flying-press", 170),
    ("gear-grind", 180),
    ("v-create", 220),
];

/// Z-Move power from the base move (WikiDex table; multi-hit moves count x3, SHOWDOWN).
pub fn z_move_power(move_name: &str, power: u16, multi_hit: bool) -> u16 {
    if let Some(&(_, fixed)) = Z_POWER_EXCEPTIONS.iter().find(|(n, _)| *n == move_name) {
        return fixed;
    }
    let power = if multi_hit { power.saturating_mul(3) } else { power };
    match power {
        0..=59 => 100,
        60..=69 => 120,
        70..=79 => 140,
        80..=89 => 160,
        90..=99 => 175,
        100..=109 => 180,
        110..=119 => 185,
        120..=129 => 190,
        130..=139 => 195,
        _ => 200,
    }
}

/// Max Move power from the base move (SHOWDOWN table; Fighting and Poison use the lower row).
pub fn max_move_power(move_type: &str, power: u16) -> u16 {
    let row: [u16; 7] = if matches!(move_type, "Fighting" | "Poison") {
        [100, 95, 90, 85, 80, 75, 70]
    } else {
        [150, 140, 130, 120, 110, 100, 90]
    };
    match power {
        0 => 100,
        150.. => row[0],
        110..=149 => row[1],
        75..=109 => row[2],
        65..=74 => row[3],
        55..=64 => row[4],
        45..=54 => row[5],
        _ => row[6],
    }
}

#[derive(Debug, Clone)]
enum Form {
    Mega { species: BattleSpecies, stats: [u32; 5] },
    Dynamax { turns_left: u8, base_max_hp: u32 },
    Tera { tera_type: String, original: [String; 2] },
}

/// Move-slot and modifier values replaced for one turn.
#[derive(Debug, Clone)]
struct SlotPatch {
    actor: usize,
    slot: usize,
    data: BattleMove,
    priority: i8,
    power_q12: u16,
    damage_q12: u16,
}

/// What the Protect hook decided for a gimmick move.
pub(super) enum ProtectOutcome {
    NotGimmick,
    Pierced,
    Blocked,
}

#[derive(Debug, Clone, Default)]
pub(super) struct GimmickState {
    enabled: bool,
    locked: Option<Gimmick>,
    side_used: Vec<bool>,
    mega_stone: Vec<bool>,
    z_crystal: Vec<Option<String>>,
    tera_type: Vec<Option<String>>,
    dynamax_level: Vec<Option<u8>>,
    form: Vec<Option<Form>>,
    patches: Vec<SlotPatch>,
    /// Actors turning their move into a Z-Move this turn.
    z_now: Vec<usize>,
    /// Actors whose move this turn is a Z-Move or a Max Move.
    piercing: Vec<usize>,
    /// Actors using Max Guard this turn.
    max_guard: Vec<usize>,
    /// (actor, move type) of the Max Moves used this turn, for their side effect.
    max_moves: Vec<(usize, String)>,
    /// Dynamaxed actors that acted this turn.
    dynamax_acted: Vec<usize>,
    events: Vec<GimmickEvent>,
}

fn grow<T: Default>(v: &mut Vec<T>, i: usize) -> &mut T {
    if v.len() <= i {
        v.resize_with(i + 1, T::default);
    }
    &mut v[i]
}

impl Battle {
    /// Turn the gimmicks on. `Some(kind)` fixes the battle's gimmick; `None` lets the first
    /// activation fix it. Either way only one kind is used in the battle.
    pub fn enable_gimmicks(&mut self, gimmick: Option<Gimmick>) {
        self.gimmicks.enabled = true;
        self.gimmicks.locked = gimmick;
    }
    pub fn gimmicks_enabled(&self) -> bool {
        self.gimmicks.enabled
    }
    /// The battle's gimmick kind, once fixed.
    pub fn battle_gimmick(&self) -> Option<Gimmick> {
        self.gimmicks.locked
    }
    pub fn give_mega_stone(&mut self, combatant: usize) {
        *grow(&mut self.gimmicks.mega_stone, combatant) = true;
    }
    pub fn give_z_crystal(&mut self, combatant: usize, move_type: &str) {
        *grow(&mut self.gimmicks.z_crystal, combatant) = Some(move_type.to_owned());
    }
    pub fn set_tera_type(&mut self, combatant: usize, tera_type: &str) {
        *grow(&mut self.gimmicks.tera_type, combatant) = Some(tera_type.to_owned());
    }
    pub fn set_dynamax_level(&mut self, combatant: usize, level: u8) {
        *grow(&mut self.gimmicks.dynamax_level, combatant) = Some(level.min(10));
    }
    pub fn take_gimmick_events(&mut self) -> Vec<GimmickEvent> {
        std::mem::take(&mut self.gimmicks.events)
    }

    /// Configured Tera type, or the first species type.
    pub fn tera_type(&self, combatant: usize) -> Option<String> {
        if let Some(Some(t)) = self.gimmicks.tera_type.get(combatant) {
            return Some(t.clone());
        }
        if let Some(Some(Form::Tera { original, .. })) = self.gimmicks.form.get(combatant) {
            return Some(original[0].clone());
        }
        self.combatants
            .get(combatant)
            .map(|c| c.species.types[0].clone())
    }
    pub fn has_mega_stone(&self, combatant: usize) -> bool {
        self.gimmicks.mega_stone.get(combatant).copied().unwrap_or(false)
    }
    pub fn z_crystal(&self, combatant: usize) -> Option<&str> {
        self.gimmicks.z_crystal.get(combatant)?.as_deref()
    }
    pub fn is_mega(&self, combatant: usize) -> bool {
        matches!(self.form_of(combatant), Some(Form::Mega { .. }))
    }
    /// Remaining Dynamax actions, `None` when not Dynamaxed.
    pub fn dynamax_turns(&self, combatant: usize) -> Option<u8> {
        match self.form_of(combatant) {
            Some(Form::Dynamax { turns_left, .. }) => Some(*turns_left),
            _ => None,
        }
    }
    /// Active Tera type, `None` when not Terastallized.
    pub fn terastallized(&self, combatant: usize) -> Option<&str> {
        match self.form_of(combatant) {
            Some(Form::Tera { tera_type, .. }) => Some(tera_type),
            _ => None,
        }
    }
    pub fn side_used_gimmick(&self, side: usize) -> bool {
        self.gimmicks.side_used.get(side).copied().unwrap_or(false)
    }

    fn form_of(&self, combatant: usize) -> Option<&Form> {
        self.gimmicks.form.get(combatant)?.as_ref()
    }
    fn side_of(&self, combatant: usize) -> Option<usize> {
        self.sides.iter().position(|s| s.contains(&combatant))
    }

    /// Why `actor` cannot use `gimmick` now; `None` when it can. `move_slot` is the move the
    /// actor will use this turn (needed for a Z-Move).
    pub fn gimmick_refusal(
        &self,
        actor: usize,
        gimmick: Gimmick,
        move_slot: Option<usize>,
    ) -> Option<Refusal> {
        let g = &self.gimmicks;
        if !g.enabled {
            return Some(Refusal::Disabled);
        }
        if let Some(locked) = g.locked {
            if locked != gimmick {
                return Some(Refusal::OtherGimmickThisBattle(locked));
            }
        }
        let Some(side) = self.side_of(actor) else {
            return Some(Refusal::NotActive);
        };
        if self.active.get(side) != Some(&actor) || !self.combatants[actor].is_alive() {
            return Some(Refusal::NotActive);
        }
        if self.side_used_gimmick(side) {
            return Some(Refusal::AlreadyUsedBySide);
        }
        let c = &self.combatants[actor];
        match gimmick {
            Gimmick::Mega => {
                if !self.has_mega_stone(actor) {
                    return Some(Refusal::NoMegaStone);
                }
                if mega_form(&c.species.name).is_none() {
                    return Some(Refusal::NoMegaForm);
                }
            }
            Gimmick::ZMove => {
                let Some(slot) = move_slot.and_then(|s| c.moves.get(s)) else {
                    return Some(Refusal::NoMove);
                };
                if slot.pp == 0 {
                    return Some(Refusal::NoMove);
                }
                if slot.data.category == "Status" {
                    return Some(Refusal::StatusZMove);
                }
                match self.z_crystal(actor) {
                    None => return Some(Refusal::NoZCrystal),
                    Some(t) if t != slot.data.move_type || type_moves(t).is_none() => {
                        return Some(Refusal::CrystalTypeMismatch)
                    }
                    Some(_) => {}
                }
            }
            Gimmick::Dynamax | Gimmick::Terastal => {}
        }
        None
    }

    /// Activate the requested gimmicks and turn this turn's moves into their gimmick versions.
    /// Call [`Battle::end_gimmick_turn`] with the turn's events afterwards. The
    /// `play_turn*_with_gimmicks` wrappers do both.
    pub fn begin_gimmick_turn(&mut self, actions: &[Action], requests: &[GimmickRequest]) {
        // A turn that was begun but never ended must not leave patched slots behind.
        self.restore_gimmick_patches();
        if !self.gimmicks.enabled {
            for r in requests {
                self.gimmicks.events.push(GimmickEvent::Refused {
                    actor: r.actor,
                    gimmick: r.gimmick,
                    reason: Refusal::Disabled,
                });
            }
            return;
        }
        let mut ordered = requests.to_vec();
        ordered.sort_by_key(|r| {
            std::cmp::Reverse(self.combatants.get(r.actor).map_or(0, |c| c.speed))
        });
        for r in ordered {
            let slot = actions
                .iter()
                .find(|a| a.actor == r.actor)
                .map(|a| a.move_slot);
            if let Some(reason) = self.gimmick_refusal(r.actor, r.gimmick, slot) {
                self.gimmicks.events.push(GimmickEvent::Refused {
                    actor: r.actor,
                    gimmick: r.gimmick,
                    reason,
                });
                continue;
            }
            if self.gimmicks.locked.is_none() {
                self.gimmicks.locked = Some(r.gimmick);
                self.gimmicks.events.push(GimmickEvent::Locked(r.gimmick));
            }
            if let Some(side) = self.side_of(r.actor) {
                *grow(&mut self.gimmicks.side_used, side) = true;
            }
            match r.gimmick {
                Gimmick::Mega => self.mega_evolve(r.actor),
                Gimmick::Dynamax => self.dynamax(r.actor),
                Gimmick::Terastal => self.terastallize(r.actor),
                Gimmick::ZMove => self.gimmicks.z_now.push(r.actor),
            }
        }
        for a in actions {
            self.patch_action(*a);
        }
    }

    /// Undo this turn's move changes, apply the Max Move side effects, count down Dynamax and
    /// revert every form once the battle has ended.
    pub fn end_gimmick_turn(&mut self, events: &[Event]) {
        self.restore_gimmick_patches();
        for (actor, move_type) in std::mem::take(&mut self.gimmicks.max_moves) {
            let target = events.iter().find_map(|e| match e {
                Event::UsedMove { actor: a, target, .. } if *a == actor => Some(*target),
                _ => None,
            });
            let (Some(target), Some(effect)) = (target, max_effect(&move_type)) else {
                continue;
            };
            match effect {
                MaxEffect::LowerTarget(stat) => {
                    if !self.combatants[target].is_alive() {
                        continue;
                    }
                    self.change_stat(target, stat, -1);
                }
                MaxEffect::RaiseUser(stat) => {
                    if !self.combatants[actor].is_alive() {
                        continue;
                    }
                    self.change_stat(actor, stat, 1);
                }
                MaxEffect::Weather(weather) => self.weather = weather,
                MaxEffect::Unsupported(_) => {}
            }
            self.gimmicks.events.push(GimmickEvent::MaxEffectApplied {
                actor,
                target,
                effect,
            });
        }
        for actor in std::mem::take(&mut self.gimmicks.dynamax_acted) {
            if let Some(Some(Form::Dynamax { turns_left, .. })) = self.gimmicks.form.get_mut(actor)
            {
                *turns_left = turns_left.saturating_sub(1);
            }
        }
        for c in 0..self.combatants.len() {
            let expired = matches!(self.form_of(c), Some(Form::Dynamax { turns_left: 0, .. }));
            let gone = !self.combatants[c].is_alive() || !self.active.contains(&c);
            if self.dynamax_turns(c).is_some() && (expired || gone) {
                self.end_dynamax(c);
            }
        }
        self.gimmicks.z_now.clear();
        self.gimmicks.piercing.clear();
        self.gimmicks.max_guard.clear();
        if self.ended {
            self.revert_gimmick_forms();
        }
    }

    /// `play_turn` with gimmick activations.
    pub fn play_turn_with_gimmicks(
        &mut self,
        actions: &[Action],
        requests: &[GimmickRequest],
        rng: &mut impl BattleRng,
    ) -> Vec<Event> {
        self.begin_gimmick_turn(actions, requests);
        let events = self.play_turn(actions, rng);
        self.end_gimmick_turn(&events);
        events
    }

    /// `play_turn_styled` with gimmick activations. A Z-Move or Max Move has no style (the
    /// style table does not know it), so its style falls back to normal.
    pub fn play_turn_styled_with_gimmicks(
        &mut self,
        actions: &[StyledAction],
        requests: &[GimmickRequest],
        rng: &mut impl BattleRng,
    ) -> Vec<Event> {
        let plain: Vec<Action> = actions.iter().map(|a| a.action).collect();
        self.begin_gimmick_turn(&plain, requests);
        let events = self.play_turn_styled(actions, rng);
        self.end_gimmick_turn(&events);
        events
    }

    /// Revert Mega Evolution, Dynamax and Terastallization (the battle is over).
    pub fn revert_gimmick_forms(&mut self) {
        for c in 0..self.gimmicks.form.len().min(self.combatants.len()) {
            match self.gimmicks.form[c].take() {
                Some(Form::Mega { species, stats }) => {
                    let cb = &mut self.combatants[c];
                    cb.species = species;
                    [cb.attack, cb.defense, cb.special_attack, cb.special_defense, cb.speed] =
                        stats;
                }
                Some(form @ Form::Dynamax { .. }) => {
                    self.gimmicks.form[c] = Some(form);
                    self.end_dynamax(c);
                }
                Some(Form::Tera { original, .. }) => self.combatants[c].species.types = original,
                None => {}
            }
        }
    }

    /// Called when a combatant leaves the field: Dynamax ends (WikiDex).
    pub(super) fn gimmick_on_leave(&mut self, combatant: usize) {
        if self.dynamax_turns(combatant).is_some() {
            self.end_dynamax(combatant);
        }
    }

    /// Protect hook (`status_pre_move`): a Z-Move or Max Move hits through Protect for 1/4,
    /// Max Guard stops a Max Move.
    pub(super) fn gimmick_protect(&mut self, attacker: usize, target: usize) -> ProtectOutcome {
        if !self.gimmicks.piercing.contains(&attacker) {
            return ProtectOutcome::NotGimmick;
        }
        if self.gimmicks.max_guard.contains(&target) {
            return ProtectOutcome::Blocked;
        }
        let c = &mut self.combatants[attacker];
        c.damage_modifier_q12 =
            q12_mul(u32::from(c.damage_modifier_q12), PROTECT_PIERCE_Q12).min(u32::from(u16::MAX)) as u16;
        self.gimmicks
            .events
            .push(GimmickEvent::ProtectPierced { attacker, target });
        ProtectOutcome::Pierced
    }

    /// Protect hook result for `statuses.rs`: event when Max Guard stopped a Max Move.
    pub(super) fn gimmick_blocked_event(attacker: usize, target: usize, move_name: &str) -> StatusEvent {
        StatusEvent::ProtectedFrom {
            target,
            attacker,
            move_name: move_name.to_owned(),
        }
    }

    fn mega_evolve(&mut self, c: usize) {
        let Some(form) = mega_form(&self.combatants[c].species.name) else {
            return;
        };
        let cb = &mut self.combatants[c];
        let species = cb.species.clone();
        let stats = [cb.attack, cb.defense, cb.special_attack, cb.special_defense, cb.speed];
        let old_base = [
            species.attack,
            species.defense,
            species.special_attack,
            species.special_defense,
            species.speed,
        ];
        let new_base: [u16; 5] = std::array::from_fn(|i| {
            (i32::from(old_base[i]) + i32::from(form.delta[i + 1])).max(1) as u16
        });
        // Scale the live stat by the formula ratio so earlier multipliers (Alpha) survive.
        let new_stats: [u32; 5] = std::array::from_fn(|i| {
            let calc = |base: u16| {
                calculate_stat(
                    u32::from(base),
                    cb.level,
                    cb.effort[i + 1],
                    cb.nature_percent[i + 1],
                    false,
                    false,
                )
                .max(1)
            };
            (u64::from(stats[i]) * u64::from(calc(new_base[i])) / u64::from(calc(old_base[i])))
                .max(1) as u32
        });
        [
            cb.species.attack,
            cb.species.defense,
            cb.species.special_attack,
            cb.species.special_defense,
            cb.species.speed,
        ] = new_base;
        [cb.attack, cb.defense, cb.special_attack, cb.special_defense, cb.speed] = new_stats;
        if let Some(types) = form.types {
            cb.species.types = types.map(str::to_owned);
        }
        *grow(&mut self.gimmicks.form, c) = Some(Form::Mega { species, stats });
        self.gimmicks.events.push(GimmickEvent::MegaEvolved {
            combatant: c,
            form: form.form,
        });
    }

    fn dynamax(&mut self, c: usize) {
        let level = self
            .gimmicks
            .dynamax_level
            .get(c)
            .copied()
            .flatten()
            .unwrap_or(DEFAULT_DYNAMAX_LEVEL);
        let percent = 150 + 5 * u32::from(level);
        let cb = &mut self.combatants[c];
        let base_max_hp = cb.max_hp;
        // Shedinja (max HP 1) gains no HP (WikiDex).
        if cb.max_hp > 1 {
            cb.max_hp = cb.max_hp * percent / 100;
            cb.hp = cb.hp * percent / 100;
        }
        let max_hp = cb.max_hp;
        *grow(&mut self.gimmicks.form, c) = Some(Form::Dynamax {
            turns_left: DYNAMAX_TURNS,
            base_max_hp,
        });
        self.gimmicks
            .events
            .push(GimmickEvent::Dynamaxed { combatant: c, max_hp });
    }

    fn end_dynamax(&mut self, c: usize) {
        let Some(Some(Form::Dynamax { base_max_hp, .. })) = self.gimmicks.form.get(c).cloned() else {
            return;
        };
        self.gimmicks.form[c] = None;
        let cb = &mut self.combatants[c];
        let dynamax_max = u64::from(cb.max_hp.max(1));
        // Rounded up (WikiDex).
        let hp = (u64::from(cb.hp) * u64::from(base_max_hp)).div_ceil(dynamax_max);
        cb.hp = (hp as u32).min(base_max_hp);
        cb.max_hp = base_max_hp;
        self.gimmicks
            .events
            .push(GimmickEvent::DynamaxEnded { combatant: c });
    }

    fn terastallize(&mut self, c: usize) {
        let Some(tera_type) = self.tera_type(c) else {
            return;
        };
        let cb = &mut self.combatants[c];
        let original = cb.species.types.clone();
        cb.species.types = [tera_type.clone(), tera_type.clone()];
        *grow(&mut self.gimmicks.form, c) = Some(Form::Tera {
            tera_type: tera_type.clone(),
            original,
        });
        self.gimmicks.events.push(GimmickEvent::Terastallized {
            combatant: c,
            tera_type,
        });
    }

    /// Replace the move of one action for this turn (Z-Move, Max Move / Max Guard, Tera power
    /// and STAB). The original values are kept in a patch and restored by
    /// `restore_gimmick_patches`.
    fn patch_action(&mut self, action: Action) {
        let actor = action.actor;
        let z = self.gimmicks.z_now.contains(&actor);
        let dynamaxed = self.dynamax_turns(actor).is_some();
        let tera = self.terastallized(actor).map(str::to_owned);
        if !z && !dynamaxed && tera.is_none() {
            return;
        }
        let Some(c) = self.combatants.get(actor) else {
            return;
        };
        let Some(slot) = c.moves.get(action.move_slot) else {
            return;
        };
        let data = slot.data.clone();
        let multi_hit = self
            .effects
            .as_ref()
            .and_then(|t| t.get(&data.name))
            .is_some_and(|f| f.max_hits > 1);
        let max_guard_priority = self
            .effects
            .as_ref()
            .and_then(|t| t.get("max-guard"))
            .map_or(4, |f| f.priority);
        let (attack, special_attack) = (c.attack, c.special_attack);
        self.gimmicks.patches.push(SlotPatch {
            actor,
            slot: action.move_slot,
            data: data.clone(),
            priority: slot.priority,
            power_q12: c.power_modifier_q12,
            damage_q12: c.damage_modifier_q12,
        });
        let damaging = data.category != "Status";
        if dynamaxed {
            self.gimmicks.dynamax_acted.push(actor);
        }
        if z && damaging {
            let Some(names) = type_moves(&data.move_type) else {
                return;
            };
            let name = format!("{}--{}", names.1, data.category.to_ascii_lowercase());
            let power = z_move_power(&data.name, data.power, multi_hit);
            let m = &mut self.combatants[actor].moves[action.move_slot];
            m.data.name = name.clone();
            m.data.power = power;
            m.data.accuracy = 101;
            self.gimmicks.piercing.push(actor);
            self.gimmicks
                .events
                .push(GimmickEvent::ZMove { actor, name, power });
        } else if dynamaxed && !damaging {
            let m = &mut self.combatants[actor].moves[action.move_slot];
            m.data.name = "max-guard".into();
            m.data.move_type = "Normal".into();
            m.data.power = 0;
            m.data.accuracy = 101;
            m.priority = max_guard_priority;
            self.gimmicks.max_guard.push(actor);
            self.gimmicks.events.push(GimmickEvent::MaxGuard { actor });
        } else if dynamaxed {
            let Some(names) = type_moves(&data.move_type) else {
                return;
            };
            let power = max_move_power(&data.move_type, data.power);
            let m = &mut self.combatants[actor].moves[action.move_slot];
            m.data.name = names.2.to_owned();
            m.data.power = power;
            m.data.accuracy = 101;
            self.gimmicks.piercing.push(actor);
            self.gimmicks
                .max_moves
                .push((actor, data.move_type.clone()));
            self.gimmicks.events.push(GimmickEvent::MaxMove {
                actor,
                name: names.2.to_owned(),
                power,
            });
        } else if let (Some(tera), true) = (tera, damaging) {
            let original = match self.form_of(actor) {
                Some(Form::Tera { original, .. }) => original.clone(),
                _ => return,
            };
            let m = &mut self.combatants[actor].moves[action.move_slot];
            if data.name == "tera-blast" {
                m.data.move_type = tera.clone();
                if attack > special_attack {
                    m.data.category = "Physical".into();
                }
            }
            let move_type = m.data.move_type.clone();
            let is_tera = move_type == tera;
            if is_tera
                && m.data.power > 0
                && m.data.power < TERA_POWER_FLOOR
                && !multi_hit
                && m.priority <= 0
            {
                m.data.power = TERA_POWER_FLOOR;
            }
            let in_original = original.contains(&move_type);
            let factor = match (is_tera, in_original) {
                (true, true) => TERA_MATCH_EXTRA_Q12,
                // The engine no longer sees the original type, so its STAB is put back here.
                (false, true) => ENGINE_STAB_Q12,
                _ => 4096,
            };
            let c = &mut self.combatants[actor];
            c.power_modifier_q12 = q12_mul(u32::from(c.power_modifier_q12), factor)
                .min(u32::from(u16::MAX)) as u16;
        }
    }

    fn restore_gimmick_patches(&mut self) {
        for p in std::mem::take(&mut self.gimmicks.patches).into_iter().rev() {
            let Some(c) = self.combatants.get_mut(p.actor) else {
                continue;
            };
            c.power_modifier_q12 = p.power_q12;
            c.damage_modifier_q12 = p.damage_q12;
            if let Some(m) = c.moves.get_mut(p.slot) {
                // PP stays as spent: the gimmick move costs the base move's PP.
                m.data = p.data;
                m.priority = p.priority;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::BattleData;
    use crate::battle_engine::effects::MoveEffectsTable;
    use crate::battle_engine::statuses::StatusRules;
    use crate::battle_engine::{Combatant, XorShift32};
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }
    fn combatant(d: &BattleData, species: &str, moves: &[&str], level: u8) -> Combatant {
        let s = d.species_row(species).unwrap().unwrap();
        let m = moves.iter().map(|n| d.move_row(n).unwrap().unwrap()).collect();
        Combatant::from_base(s, level, m)
    }
    fn battle(a: Combatant, b: Combatant) -> Battle {
        let d = BattleData::load(root()).unwrap();
        Battle::new(vec![a, b], vec![vec![0], vec![1]], d.type_chart().clone()).unwrap()
    }
    fn act(actor: usize, move_slot: usize) -> Action {
        Action { actor, move_slot }
    }
    fn req(actor: usize, gimmick: Gimmick) -> GimmickRequest {
        GimmickRequest { actor, gimmick }
    }

    #[test]
    fn power_tables_match_the_sources() {
        assert_eq!(z_move_power("thunderbolt", 90, false), 175);
        assert_eq!(z_move_power("ember", 40, false), 100);
        assert_eq!(z_move_power("bullet-seed", 25, true), 140);
        assert_eq!(z_move_power("hex", 65, false), 160);
        assert_eq!(z_move_power("close-combat", 120, false), 190);
        assert_eq!(z_move_power("giga-impact", 150, false), 200);
        assert_eq!(max_move_power("Electric", 90), 130);
        assert_eq!(max_move_power("Fire", 40), 90);
        assert_eq!(max_move_power("Fighting", 120), 95);
        assert_eq!(max_move_power("Poison", 40), 70);
        assert_eq!(max_move_power("Normal", 150), 150);
        for f in MEGA_FORMS.iter() {
            assert_eq!(f.delta.iter().sum::<i16>(), 100, "{}", f.form);
            assert_eq!(f.delta[0], 0, "{}", f.form);
        }
    }

    #[test]
    fn only_one_gimmick_kind_per_battle_and_once_per_side() {
        let d = BattleData::load(root()).unwrap();
        let mut b = battle(
            combatant(&d, "pikachu", &["thunderbolt"], 5),
            combatant(&d, "gengar", &["thunderbolt"], 50),
        );
        b.enable_gimmicks(None);
        let mut rng = XorShift32(7);
        b.play_turn_with_gimmicks(&[act(0, 0)], &[req(0, Gimmick::Terastal)], &mut rng);
        assert_eq!(b.battle_gimmick(), Some(Gimmick::Terastal));
        assert_eq!(b.gimmick_refusal(1, Gimmick::Dynamax, Some(0)), Some(Refusal::OtherGimmickThisBattle(Gimmick::Terastal)));
        assert_eq!(b.gimmick_refusal(0, Gimmick::Terastal, Some(0)), Some(Refusal::AlreadyUsedBySide));
        assert_eq!(b.gimmick_refusal(1, Gimmick::Terastal, Some(0)), None);
        b.take_gimmick_events();
        b.begin_gimmick_turn(&[act(1, 0)], &[req(1, Gimmick::Dynamax)]);
        assert!(b.take_gimmick_events().iter().any(|e| matches!(
            e,
            GimmickEvent::Refused { actor: 1, reason: Refusal::OtherGimmickThisBattle(Gimmick::Terastal), .. }
        )));
        assert_eq!(b.dynamax_turns(1), None);
    }

    #[test]
    fn disabled_battles_refuse_everything_and_play_normally() {
        let d = BattleData::load(root()).unwrap();
        let mut b = battle(
            combatant(&d, "pikachu", &["thunderbolt"], 30),
            combatant(&d, "gengar", &["thunderbolt"], 30),
        );
        assert_eq!(b.gimmick_refusal(0, Gimmick::Dynamax, None), Some(Refusal::Disabled));
        let mut plain = b.clone();
        let e1 = b.play_turn_with_gimmicks(&[act(0, 0)], &[req(0, Gimmick::Dynamax)], &mut XorShift32(3));
        let e2 = plain.play_turn(&[act(0, 0)], &mut XorShift32(3));
        assert_eq!(e1, e2);
        assert_eq!(b.combatants[0].max_hp, plain.combatants[0].max_hp);
    }

    #[test]
    fn mega_evolution_raises_stats_changes_types_and_reverts_at_the_end() {
        let d = BattleData::load(root()).unwrap();
        let mut b = battle(
            combatant(&d, "lopunny", &["quick-attack"], 50),
            combatant(&d, "pikachu", &["thunderbolt"], 5),
        );
        let before = b.combatants[0].clone();
        b.enable_gimmicks(Some(Gimmick::Mega));
        assert_eq!(b.gimmick_refusal(0, Gimmick::Mega, None), Some(Refusal::NoMegaStone));
        b.give_mega_stone(0);
        b.begin_gimmick_turn(&[act(0, 0)], &[req(0, Gimmick::Mega)]);
        assert!(b.is_mega(0));
        let m = &b.combatants[0];
        assert_eq!(m.species.attack, 136);
        assert_eq!(m.species.speed, 135);
        assert_eq!(m.species.types, ["Normal".to_string(), "Fighting".to_string()]);
        assert!(m.attack > before.attack && m.speed > before.speed);
        assert_eq!(m.special_attack, before.special_attack);
        assert_eq!(m.max_hp, before.max_hp);
        b.end_gimmick_turn(&[]);
        b.ended = true;
        b.end_gimmick_turn(&[]);
        assert!(!b.is_mega(0));
        assert_eq!(b.combatants[0].attack, before.attack);
        assert_eq!(b.combatants[0].species.types, before.species.types);
    }

    #[test]
    fn z_move_needs_a_matching_crystal_and_restores_the_slot() {
        let d = BattleData::load(root()).unwrap();
        let mut b = battle(
            combatant(&d, "pikachu", &["thunderbolt", "thunder-wave"], 30),
            combatant(&d, "gengar", &["thunderbolt"], 30),
        );
        b.enable_gimmicks(Some(Gimmick::ZMove));
        assert_eq!(b.gimmick_refusal(0, Gimmick::ZMove, Some(0)), Some(Refusal::NoZCrystal));
        b.give_z_crystal(0, "Fire");
        assert_eq!(b.gimmick_refusal(0, Gimmick::ZMove, Some(0)), Some(Refusal::CrystalTypeMismatch));
        b.give_z_crystal(0, "Electric");
        assert_eq!(b.gimmick_refusal(0, Gimmick::ZMove, Some(1)), Some(Refusal::StatusZMove));
        let pp = b.combatants[0].moves[0].pp;
        let events = b.play_turn_with_gimmicks(&[act(0, 0)], &[req(0, Gimmick::ZMove)], &mut XorShift32(9));
        assert!(events.iter().any(|e| matches!(e, Event::UsedMove { name, .. } if name == "gigavolt-havoc--special")));
        assert!(b.take_gimmick_events().iter().any(|e| matches!(e, GimmickEvent::ZMove { power: 160, .. })));
        let slot = &b.combatants[0].moves[0];
        assert_eq!((slot.data.name.as_str(), slot.data.power), ("thunderbolt", 80));
        assert_eq!(slot.pp, pp - 1);
        assert!(b.side_used_gimmick(0));
    }

    #[test]
    fn z_move_hits_through_protect_for_a_quarter() {
        let d = BattleData::load(root()).unwrap();
        let mk = || {
            let mut b = battle(
                combatant(&d, "pikachu", &["thunderbolt"], 40),
                combatant(&d, "gengar", &["protect"], 40),
            );
            b.combatants[1].speed = 999;
            b.set_move_effects(MoveEffectsTable::load(root()).unwrap());
            b.set_status_rules(StatusRules::game_rules());
            b.enable_gimmicks(Some(Gimmick::ZMove));
            b.give_z_crystal(0, "Electric");
            b
        };
        let (mut open, mut guarded) = (mk(), mk());
        let hit = |b: &mut Battle, protect: bool| {
            let mut actions = vec![act(0, 0)];
            if protect {
                actions.push(act(1, 0));
            }
            let events = b.play_turn_with_gimmicks(&actions, &[req(0, Gimmick::ZMove)], &mut XorShift32(11));
            events
                .iter()
                .find_map(|e| match e {
                    Event::UsedMove { actor: 0, damage, .. } => Some(*damage),
                    _ => None,
                })
                .unwrap()
        };
        let full = hit(&mut open, false);
        let quarter = hit(&mut guarded, true);
        assert!(quarter < full, "{quarter} vs {full}");
        assert!(guarded
            .take_gimmick_events()
            .iter()
            .any(|e| matches!(e, GimmickEvent::ProtectPierced { attacker: 0, target: 1 })));
        assert_eq!(guarded.combatants[0].damage_modifier_q12, 4096);
    }

    #[test]
    fn dynamax_scales_hp_uses_max_moves_and_ends_after_three_actions() {
        let d = BattleData::load(root()).unwrap();
        let mut b = battle(
            combatant(&d, "pikachu", &["ember", "thunder-wave"], 50),
            combatant(&d, "garchomp", &["thunderbolt"], 100),
        );
        b.enable_gimmicks(Some(Gimmick::Dynamax));
        let base = b.combatants[0].max_hp;
        let mut rng = XorShift32(5);
        b.play_turn_with_gimmicks(&[act(0, 0)], &[req(0, Gimmick::Dynamax)], &mut rng);
        assert_eq!(b.combatants[0].max_hp, base * 2);
        assert_eq!(b.dynamax_turns(0), Some(2));
        assert_eq!(b.weather, BattleWeather::Sun, "Max Flare sets sun");
        let events = b.take_gimmick_events();
        assert!(events.iter().any(|e| matches!(e, GimmickEvent::MaxMove { power: 90, .. })));
        b.play_turn_with_gimmicks(&[act(0, 1)], &[], &mut rng);
        assert!(b.take_gimmick_events().iter().any(|e| matches!(e, GimmickEvent::MaxGuard { actor: 0 })));
        assert_eq!(b.combatants[0].moves[1].data.name, "thunder-wave");
        b.combatants[0].hp = b.combatants[0].max_hp - 1;
        b.play_turn_with_gimmicks(&[act(0, 0)], &[], &mut rng);
        assert_eq!(b.dynamax_turns(0), None);
        assert_eq!(b.combatants[0].max_hp, base);
        assert_eq!(b.combatants[0].hp, base, "odd HP rounds up");
    }

    #[test]
    fn dynamax_ends_when_switching_out() {
        let d = BattleData::load(root()).unwrap();
        let mons = vec![
            combatant(&d, "pikachu", &["ember"], 30),
            combatant(&d, "lucario", &["ember"], 30),
            combatant(&d, "gengar", &["thunderbolt"], 30),
        ];
        let mut b = Battle::new(mons, vec![vec![0, 1], vec![2]], d.type_chart().clone()).unwrap();
        b.enable_gimmicks(None);
        let base = b.combatants[0].max_hp;
        b.play_turn_with_gimmicks(&[act(0, 0)], &[req(0, Gimmick::Dynamax)], &mut XorShift32(2));
        b.switch(0, 1).unwrap();
        assert_eq!(b.dynamax_turns(0), None);
        assert_eq!(b.combatants[0].max_hp, base);
    }

    #[test]
    fn terastallization_changes_types_floors_power_and_keeps_stab() {
        let d = BattleData::load(root()).unwrap();
        let mut b = battle(
            combatant(&d, "pikachu", &["ember", "thunderbolt", "quick-attack"], 30),
            combatant(&d, "gengar", &["thunderbolt"], 30),
        );
        b.enable_gimmicks(None);
        b.set_tera_type(0, "Fire");
        b.begin_gimmick_turn(&[act(0, 0)], &[req(0, Gimmick::Terastal)]);
        assert_eq!(b.terastallized(0), Some("Fire"));
        assert_eq!(b.combatants[0].species.types, ["Fire".to_string(), "Fire".to_string()]);
        assert_eq!(b.combatants[0].moves[0].data.power, TERA_POWER_FLOOR, "ember 40 -> 60");
        assert_eq!(b.combatants[0].power_modifier_q12, 4096, "new Tera type: engine STAB only");
        b.end_gimmick_turn(&[]);
        assert_eq!(b.combatants[0].moves[0].data.power, 40);
        b.begin_gimmick_turn(&[act(0, 1)], &[]);
        assert_eq!(b.combatants[0].power_modifier_q12, ENGINE_STAB_Q12 as u16, "original type keeps STAB");
        b.end_gimmick_turn(&[]);
        b.begin_gimmick_turn(&[act(0, 2)], &[]);
        assert_eq!(b.combatants[0].power_modifier_q12, 4096, "quick-attack: no STAB");
        b.end_gimmick_turn(&[]);
        assert_eq!(b.combatants[0].power_modifier_q12, 4096);
    }

    #[test]
    fn tera_matching_an_original_type_boosts_stab() {
        let d = BattleData::load(root()).unwrap();
        let mut b = battle(
            combatant(&d, "pikachu", &["thunderbolt"], 30),
            combatant(&d, "gengar", &["thunderbolt"], 30),
        );
        b.enable_gimmicks(Some(Gimmick::Terastal));
        b.begin_gimmick_turn(&[act(0, 0)], &[req(0, Gimmick::Terastal)]);
        assert_eq!(b.terastallized(0), Some("Electric"));
        assert_eq!(b.combatants[0].power_modifier_q12, TERA_MATCH_EXTRA_Q12 as u16);
        b.end_gimmick_turn(&[]);
    }
}
