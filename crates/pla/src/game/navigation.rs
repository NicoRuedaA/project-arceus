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
    /// If neither is walkable, clamps to nearest valid point or stays in place.
    pub fn try_move_with_slide(&self, curr: Vec2, delta: Vec2) -> Vec2 {
        let target = curr + delta;
        if self.is_walkable(target.x, target.y) {
            return target;
        }

        // Try sliding along X axis
        let target_x = Vec2::new(target.x, curr.y);
        if delta.x.abs() > 1e-4 && self.is_walkable(target_x.x, target_x.y) {
            return target_x;
        }

        // Try sliding along Z axis
        let target_z = Vec2::new(curr.x, target.y);
        if delta.y.abs() > 1e-4 && self.is_walkable(target_z.x, target_z.y) {
            return target_z;
        }

        // If hitting corner or wall, check nearest walkable boundary
        if let Some(hit) = self.nearest(target.x, target.y, 1.2) {
            if hit.distance > 0.05 {
                // Return current position preventing boundary penetration
                return curr;
            }
        }

        curr
    }
}
