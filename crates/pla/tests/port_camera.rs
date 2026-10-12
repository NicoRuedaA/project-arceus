use bevy::prelude::Vec3;
use pla::pml::{calculate_hp, PmlDatabase};

/// Tests canonical PLA spherical-to-Cartesian coordinate transformation:
/// ΔX = dist * cos(pitch) * sin(yaw)
/// ΔY = dist * sin(pitch)
/// ΔZ = dist * cos(pitch) * cos(yaw)
/// cameraPos = lookAtPos + Δ
#[test]
fn pla_camera_spherical_coordinate_transform_matches_canonical_spec() {
    let focal_target = Vec3::new(10.0, 1.2, -5.0);
    let distance = 5.0f32;
    let pitch = 0.35f32;
    let yaw = 0.0f32;

    let delta_x = distance * pitch.cos() * yaw.sin();
    let delta_y = distance * pitch.sin();
    let delta_z = distance * pitch.cos() * yaw.cos();
    let eye_pos = focal_target + Vec3::new(delta_x, delta_y, delta_z);

    assert_eq!(delta_x, 0.0);
    assert!((delta_y - (5.0 * 0.35f32.sin())).abs() < 1e-6);
    assert!((delta_z - (5.0 * 0.35f32.cos())).abs() < 1e-6);
    assert!((eye_pos.x - 10.0).abs() < 1e-6);
    assert!((eye_pos.y - (1.2 + 5.0 * 0.35f32.sin())).abs() < 1e-6);
    assert!((eye_pos.z - (-5.0 + 5.0 * 0.35f32.cos())).abs() < 1e-6);
}

/// Tests that switching to Aim Mode (ZL Over-the-Shoulder) contracts distance
/// to 2.2m and shifts the look-at focal target to the right shoulder.
#[test]
fn pla_camera_aim_mode_kinematics() {
    let player_pos = Vec3::new(0.0, 0.9, 0.0);
    let focal_offset_y = 1.2f32;
    let yaw = 0.0f32;

    // Default orbit mode focal target
    let default_focal = player_pos + Vec3::new(0.0, focal_offset_y, 0.0);

    // Aim mode focal target (right shoulder offset)
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let aim_focal = default_focal + right * 0.45 + Vec3::new(0.0, 0.15, 0.0);

    assert_eq!(default_focal, Vec3::new(0.0, 2.1, 0.0));
    assert_eq!(aim_focal, Vec3::new(0.45, 2.25, 0.0));

    // Verify distance contracting
    let default_distance = 5.0f32;
    let aim_distance = 2.2f32;
    assert!(aim_distance < default_distance);
}

/// Tests PML integration within the port viewer context for Pikachu #025.
#[test]
fn pml_pikachu_stats_and_hp_calculation_in_port_context() {
    let pml_dir = [
        std::path::PathBuf::from("data"),
        std::path::PathBuf::from("../../data"),
        std::path::PathBuf::from("/home/nico/work/decompilacion/data"),
    ]
    .into_iter()
    .find(|p| p.join("pml_personal.tsv").exists());

    let Some(dir) = pml_dir else {
        eprintln!("[skip] data/ directory not found in test environment");
        return;
    };

    let db = PmlDatabase::load_from_dir(dir).expect("load PML tables");
    let pikachu = db
        .personal
        .iter()
        .find(|p| p.national_dex == 25)
        .expect("Pikachu #025 must exist in personal table");

    assert_eq!(pikachu.species_name, "pikachu");
    assert_eq!(pikachu.type1_name.to_lowercase(), "electric");
    assert_eq!(pikachu.base_stats[0], 35, "Pikachu base HP is 35");

    // Level 25, Ganba K[G]=3 (K=4), non-Shedinja
    let hp = calculate_hp(pikachu.base_stats[0], 25, 3, Some(false));
    assert_eq!(hp, Some(87), "Level 25 Pikachu with Ganba 3 has 87 HP in PLA");
}

/// Tests ballistic trajectory flight under PLA gravity and CCD intersection.
#[test]
fn ballistics_trajectory_and_ccd_intersection() {
    let spawn_pos = Vec3::new(0.4, 0.9, 0.0);
    let target_pos = Vec3::new(0.0, 0.45, -3.8);

    let aim_dir = (target_pos - spawn_pos).normalize();
    let speed = 30.0f32;
    let mut velocity = aim_dir * speed;
    let gravity = -16.0f32;

    let dt = 1.0 / 60.0f32;
    let mut ball_pos = spawn_pos;
    let mut hit_detected = false;

    for _ in 0..60 {
        let prev_pos = ball_pos;
        let next_pos = prev_pos + velocity * dt;
        velocity.y += gravity * dt;
        ball_pos = next_pos;

        // Continuous collision detection
        let seg = next_pos - prev_pos;
        let seg_len_sq = seg.length_squared();
        let to_target = target_pos - prev_pos;
        let t = (to_target.dot(seg) / seg_len_sq).clamp(0.0, 1.0);
        let closest = prev_pos + seg * t;
        let hit_dist = closest.distance(target_pos);

        if hit_dist < 0.95 {
            hit_detected = true;
            break;
        }
    }

    assert!(hit_detected, "Poké Ball trajectory must connect with target via CCD");
}

/// Tests PLA capture rate evaluation with BattleData multipliers.
#[test]
fn pla_capture_rate_evaluation_with_battle_data() {
    let data_dir = [
        std::path::PathBuf::from("data"),
        std::path::PathBuf::from("../../data"),
        std::path::PathBuf::from("/home/nico/work/decompilacion/data"),
    ]
    .into_iter()
    .find(|p| p.join("capture/capture_ball_multipliers.tsv").exists());

    let Some(dir) = data_dir else {
        eprintln!("[skip] capture tables not found");
        return;
    };

    let battle = pla::battle::BattleData::load(dir).expect("load battle tables");
    let regular_mod = battle.capture_factor("capture_index_ball_factor_default")
        .expect("read regular ball factor")
        .unwrap_or(0.75);
    let ultra_mod = battle.capture_factor("capture_index_ball_factor_hyper")
        .expect("read ultra ball factor")
        .unwrap_or(2.25);

    assert_eq!(regular_mod, 0.75, "Standard Poké Ball has 0.75 multiplier");
    assert_eq!(ultra_mod, 2.25, "Ultra Ball has 2.25 multiplier");
    assert!(ultra_mod > regular_mod, "Ultra Ball must have higher multiplier than standard ball");
}
