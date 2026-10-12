//! Navigation mesh resource and spatial collision system.

use std::path::Path;
use std::sync::Arc;
use bevy::prelude::*;
use crate::assets::navmesh::{AreaNavigation, NavHit};

#[derive(Resource, Clone)]
pub struct GameNavmeshResource {
    pub nav: Arc<AreaNavigation>,
}

impl GameNavmeshResource {
    pub fn load(bin_root: impl AsRef<Path>, area: u8) -> Result<Self, String> {
        let nav = AreaNavigation::load(bin_root, area)?;
        Ok(Self {
            nav: Arc::new(nav),
        })
    }

    pub fn is_walkable(&self, x: f32, z: f32) -> bool {
        self.nav.is_walkable(x, z)
    }

    pub fn height_at(&self, x: f32, z: f32) -> Option<f32> {
        self.nav.height_at(x, z)
    }

    pub fn nearest(&self, x: f32, z: f32, max_dist: f32) -> Option<NavHit> {
        self.nav.nearest(x, z, max_dist)
    }

    /// Try moving from (curr_x, curr_z) along move_vec (dx, dz).
    /// If target (curr_x + dx, curr_z + dz) is walkable, returns target.
    /// If not walkable, tries sliding along x-only or z-only axis.
    /// If neither is walkable, tries the move rotated by up to 90 deg, else stays in place.
    pub fn try_move_with_slide(&self, curr: Vec2, delta: Vec2) -> Vec2 {
        crate::game::spawn_point::slide_move(curr, delta, |p| self.is_walkable(p.x, p.y))
    }
}
