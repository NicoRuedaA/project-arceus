//! Player facial expressions (Brief 22, expressions part).
//!
//! The player's face rig carries real eyelid, eyebrow, jaw and lip bones, and the game ships
//! real facial animation clips for it in `pc/dressup_motion.gfpak`:
//!
//! - `00011..00021_faceNN_loop` -> exported as `face_01`..`face_11`: eleven static face poses
//!   (each animates the mouth shape, the eyes, the brows and sometimes the jaw; the animation
//!   controller of the pack maps them to the `npc_face_mouth/mouth_shape/mouthNN` states).
//!   Three of them are byte-identical (`face_01` = `face_02` = `face_03`, VERIFIED) and
//!   `face_11` matches the blink pose; ten distinct poses remain.
//! - `00201_eye_close01` -> exported as `eye_close`: the closed-eyes pose (eyelid bones move
//!   24/54/40 mm up and 7/11/8 mm down, VERIFIED).
//!
//! What the extracted data does NOT contain is the mapping "expression name -> faceNN". The
//! names come from `player_face_config.bin` (`data/character_face.tsv`): normal, damage,
//! serious, rejoice, trouble, smile, sad, relief, anger, unique1, unique2. The mapping below was
//! classified from the bone movement of each clip (which bones move, how far, which way):
//!
//! | clip | measured pose (VERIFIED) | expression (this port) |
//! |------|--------------------------|------------------------|
//! | face_01 | most neutral pose | `Normal` (name PROBABLE: `face_default -> normal`) |
//! | face_02/03 | identical to face_01 | `Unique1` / `Unique2` (PROBABLE: distinct catalogue indexes, same pose) |
//! | face_04 | eyes shut (24/35/31 mm) + jaw open 18 deg | `Damage` (SUPUESTO: flinch) |
//! | face_05 | brows up 13 deg, mouth slightly open | `Smile` (SUPUESTO) |
//! | face_06 | brows down 10-11 deg | `Anger` (PROBABLE: frowning brows) |
//! | face_07 | jaw open 9/16 deg, mouth corners out 14 mm | `Trouble` (SUPUESTO: open-mouth wail) |
//! | face_08 | brows up 20 deg | `Rejoice` (SUPUESTO: raised brows) |
//! | face_09 | mouth corners 6 mm, nothing else | `Serious` (PROBABLE: tight mouth; `face_hide -> serious`) |
//! | face_10 | eyes half closed | `Sad` (SUPUESTO: downcast eyes) |
//! | face_11 / eye_close | eyes fully closed | blink pose (`Relief` unused) |
//!
//! Which expressions the port actually triggers is evidence-based:
//! - `face_hide -> serious` while the player is hidden (`game::player_skills::PlayerStealth`),
//! - `face_battle_result_win -> rejoice` / `face_battle_result_lose -> trouble` after a battle,
//!   with the decoded keep frames (`battle_result_win_keep_frame` 90, `..._lose_keep_frame` 120,
//!   frames of an assumed 30 Hz tick),
//! - `normal` the rest of the time.
//!
//! The expressions are drawn by playing the clip on the FACE piece's `AnimationPlayer` (the
//! face clips only write face bones, which no body clip touches, so both play together).

use bevy::animation::AnimationPlayer;
use bevy::animation::transition::AnimationTransitions;
use bevy::prelude::*;

use super::player_anim::PlayerAnimState;
use super::player_skills::PlayerStealth;
use super::save_wire::BattleTracker;
use super::PlayerAvatar;

/// Number of exported face clips.
pub const FACE_CLIP_SLOTS: usize = 12;
/// glTF animation names of the face clips, in slot order.
pub const FACE_CLIP_NAMES: [&str; FACE_CLIP_SLOTS] = [
    "face_01", "face_02", "face_03", "face_04", "face_05", "face_06", "face_07", "face_08",
    "face_09", "face_10", "face_11", "eye_close",
];

/// Face-clip slot of each expression (see the module table).
fn clip_slot(expression: FaceExpression) -> usize {
    use FaceExpression::*;
    match expression {
        Normal => 0,
        Unique1 => 1,
        Unique2 => 2,
        Damage => 3,
        Smile => 4,
        Anger => 5,
        Trouble => 6,
        Rejoice => 7,
        Serious => 8,
        Sad => 9,
        Relief => 10,
    }
}

/// The expression names decoded from `player_face_config.bin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FaceExpression {
    #[default]
    Normal,
    Damage,
    Serious,
    Rejoice,
    Trouble,
    Smile,
    Sad,
    Relief,
    Anger,
    Unique1,
    Unique2,
}

/// Frames an expression is held before falling back to `normal` (decoded keep frames,
/// `data/character_face.tsv`; the 30 Hz tick rate is ASSUMED, like the blink intervals).
pub const WIN_KEEP_FRAMES: f32 = 90.0;
pub const LOSE_KEEP_FRAMES: f32 = 120.0;
pub const HOLD_TICK_HZ: f32 = 30.0;

/// Current expression and remaining hold time (seconds).
#[derive(Resource, Debug, Default)]
pub struct FaceExpressionState {
    pub current: FaceExpression,
    pub hold: f32,
    /// Debug/test override (`--face serious`): while set, this expression wins over the live
    /// game state. `None` in normal play.
    pub forced: Option<FaceExpression>,
}

impl FaceExpression {
    /// Parse the decoded expression names (`player_face_config.bin`, `data/character_face.tsv`).
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "normal" => Self::Normal,
            "damage" => Self::Damage,
            "serious" => Self::Serious,
            "rejoice" => Self::Rejoice,
            "trouble" => Self::Trouble,
            "smile" => Self::Smile,
            "sad" => Self::Sad,
            "relief" => Self::Relief,
            "anger" => Self::Anger,
            "unique1" => Self::Unique1,
            "unique2" => Self::Unique2,
            _ => return None,
        })
    }
}

/// The expression the game state asks for right now, before the hold timer is applied.
/// Pure, unit-tested.
pub fn wanted_expression(hidden: bool, battle: Option<bool>) -> FaceExpression {
    if hidden {
        // `face_hide -> serious` (decoded mapping).
        return FaceExpression::Serious;
    }
    match battle {
        // `face_battle_result_win -> rejoice`, `face_battle_result_lose -> trouble`.
        Some(true) => FaceExpression::Rejoice,
        Some(false) => FaceExpression::Trouble,
        None => FaceExpression::Normal,
    }
}

/// Pick the expression for this frame: a battle result holds for its decoded keep frames, the
/// hide state applies while it lasts, otherwise `normal`. Pure, unit-tested.
pub fn select_expression(state: &FaceExpressionState, hidden: bool, fresh_result: Option<bool>, dt: f32) -> (FaceExpression, f32) {
    let hold = (state.hold - dt).max(0.0);
    if let Some(won) = fresh_result {
        let frames = if won { WIN_KEEP_FRAMES } else { LOSE_KEEP_FRAMES };
        return (wanted_expression(hidden, Some(won)), frames / HOLD_TICK_HZ);
    }
    if hold > 0.0 {
        return (state.current, hold);
    }
    (wanted_expression(hidden, None), 0.0)
}

fn drive_face_expression(
    time: Res<Time>,
    mut state: ResMut<FaceExpressionState>,
    stealth: Option<Res<PlayerStealth>>,
    battle: Option<Res<BattleTracker>>,
    mut last_result: Local<Option<bool>>,
    mut avatars: Query<&mut PlayerAnimState, With<PlayerAvatar>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let hidden = stealth.is_some_and(|s| s.hidden);
    // A battle result is "fresh" the first frame it appears in the tracker.
    let result = battle.as_deref().and_then(|b| b.last.as_ref()).map(|o| o.won);
    let fresh = match (result, *last_result) {
        (Some(won), last) if last != Some(won) => Some(won),
        _ => None,
    };
    *last_result = result;
    let dt = time.delta_secs();
    let (want, hold) = match state.forced {
        Some(f) => (f, 0.0),
        None => select_expression(&state, hidden, fresh, dt),
    };
    if want == state.current && hold == state.hold && state.forced.is_none() {
        return;
    }
    state.current = want;
    state.hold = hold;
    let node_slot = clip_slot(want);
    for mut anim in &mut avatars {
        if anim.face_current == Some(node_slot) {
            continue;
        }
        let mut played = false;
        for scene in &anim.scenes {
            let Some(node) = scene.face[node_slot] else { continue };
            if let Ok((mut player, mut tr)) = players.get_mut(scene.player) {
                // Face poses are static loops: play them looping; cross-fade keeps it soft.
                tr.play(&mut player, node, std::time::Duration::from_millis(200)).repeat();
                played = true;
            }
        }
        if played {
            anim.face_current = Some(node_slot);
        }
    }
}

/// Adds the facial-expression systems. The clips come from the face piece of the dress-up
/// export; with the merged fallback model (no face clips) nothing plays and the state stays
/// `normal`.
pub struct FaceExpressionPlugin;

impl Plugin for FaceExpressionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FaceExpressionState>();
        app.add_systems(Update, drive_face_expression.after(super::player_anim::PlayerAnimSystems));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_player_is_serious() {
        assert_eq!(wanted_expression(true, None), FaceExpression::Serious);
        // Hiding beats a battle result (both decoded; hide is the live situation).
        assert_eq!(wanted_expression(true, Some(true)), FaceExpression::Serious);
    }

    #[test]
    fn battle_results_map_to_rejoice_and_trouble() {
        assert_eq!(wanted_expression(false, Some(true)), FaceExpression::Rejoice);
        assert_eq!(wanted_expression(false, Some(false)), FaceExpression::Trouble);
        assert_eq!(wanted_expression(false, None), FaceExpression::Normal);
    }

    #[test]
    fn a_fresh_result_holds_for_its_decoded_keep_frames() {
        let state = FaceExpressionState::default();
        let (expr, hold) = select_expression(&state, false, Some(true), 1.0 / 60.0);
        assert_eq!(expr, FaceExpression::Rejoice);
        assert!((hold - WIN_KEEP_FRAMES / HOLD_TICK_HZ).abs() < 1e-6);
        let (expr, hold) = select_expression(&state, false, Some(false), 1.0 / 60.0);
        assert_eq!(expr, FaceExpression::Trouble);
        assert!((hold - LOSE_KEEP_FRAMES / HOLD_TICK_HZ).abs() < 1e-6);
        // While the hold runs, the expression stays even if the tracker no longer changes.
        let held = FaceExpressionState { current: FaceExpression::Rejoice, hold: 1.0, forced: None };
        let (expr, hold) = select_expression(&held, false, None, 0.5);
        assert_eq!(expr, FaceExpression::Rejoice);
        assert!((hold - 0.5).abs() < 1e-6);
        // Once it expires the live state takes over again.
        let (expr, hold) = select_expression(&held, false, None, 2.0);
        assert_eq!(expr, FaceExpression::Normal);
        assert_eq!(hold, 0.0);
    }

    #[test]
    fn every_expression_maps_to_a_distinct_clip_slot() {
        let all = [
            FaceExpression::Normal,
            FaceExpression::Damage,
            FaceExpression::Serious,
            FaceExpression::Rejoice,
            FaceExpression::Trouble,
            FaceExpression::Smile,
            FaceExpression::Sad,
            FaceExpression::Relief,
            FaceExpression::Anger,
            FaceExpression::Unique1,
            FaceExpression::Unique2,
        ];
        let mut slots: Vec<usize> = all.iter().map(|e| clip_slot(*e)).collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), all.len(), "every expression needs its own clip");
        for s in slots {
            assert!(s < FACE_CLIP_SLOTS);
        }
    }

    #[test]
    fn face_clip_names_match_the_export() {
        assert_eq!(FACE_CLIP_NAMES.len(), FACE_CLIP_SLOTS);
        assert!(FACE_CLIP_NAMES.contains(&"eye_close"));
        assert_eq!(FACE_CLIP_NAMES[0], "face_01");
        assert_eq!(FACE_CLIP_NAMES[10], "face_11");
    }
}
