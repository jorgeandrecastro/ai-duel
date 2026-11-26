// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Jorge Andre Castro <georgeandrec@gmail.com>

//! Resources, components, and configuration for the plugin.

use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// All duel settings in one place (cooldowns, damage, animation speed,
/// hitboxes...).
///
/// The plugin inserts a default copy; to change a setting, insert your own
/// just after adding the plugin:
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use ai_duel::{AiDuelPlugin, AiDuelConfig};
/// # fn main() {
/// App::new()
///     .add_plugins(AiDuelPlugin)
///     .insert_resource(AiDuelConfig {
///         attack_damage: 25.0,
///         attack_cooldown: 0.4,
///         ..default()
///     })
///     .run();
/// # }
/// ```
///
/// Distances are expressed in **world units** (after applying
/// [`sprite_scale`](Self::sprite_scale)).
#[derive(Resource, Clone, Debug)]
pub struct AiDuelConfig {
    /// Number of frames in the running animation.
    pub run_frames: usize,
    /// Duration of a running frame, in seconds.
    pub run_frame_time: f32,

    /// Number of frames in the attack animation.
    pub attack_frames: usize,
    /// Duration of an attack frame, in seconds.
    pub attack_frame_time: f32,

    /// Minimum time between attacks by the same fighter (seconds).
    pub attack_cooldown: f32,
    /// Invulnerability duration after a hit (seconds), to prevent being hit
    /// multiple times by the same attack.
    pub hit_cooldown: f32,
    /// Damage dealt by an attack.
    pub attack_damage: f32,

    /// First attack animation frame in which the hitbox is active.
    pub hitbox_active_from: usize,
    /// Last frame (inclusive) in which the hitbox is active.
    pub hitbox_active_to: usize,
    /// Half-size of the attack hitbox (AABB).
    pub hitbox_half_size: Vec2,
    /// Hitbox offset from the fighter's center when facing right. The X offset
    /// is automatically reversed when the sprite is flipped.
    pub hitbox_offset: Vec2,
    /// Half-size of the area where a fighter can be hit (AABB).
    pub hurtbox_half_size: Vec2,
    /// Displays hitboxes on screen (debugging).
    pub debug_hitboxes: bool,

    /// A fighter's health points at creation.
    pub default_max_health: f32,
    /// Scale applied to fighter sprites.
    pub sprite_scale: f32,
}

impl Default for AiDuelConfig {
    fn default() -> Self {
        Self {
            run_frames: 8,
            run_frame_time: 0.10,
            attack_frames: 6,
            attack_frame_time: 0.08,
            attack_cooldown: 0.60,
            hit_cooldown: 0.40,
            attack_damage: 10.0,
            hitbox_active_from: 2,
            hitbox_active_to: 4,
            hitbox_half_size: Vec2::new(35.0, 30.0),
            hitbox_offset: Vec2::new(45.0, 0.0),
            hurtbox_half_size: Vec2::new(20.0, 35.0),
            debug_hitboxes: false,
            default_max_health: 100.0,
            sprite_scale: 1.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Assets
// ---------------------------------------------------------------------------

/// The spritesheets (running and attacking), shared by all fighters.
///
/// Every entity references the same handles: no copy per fighter.
/// This resource must be inserted by the host application.
#[derive(Resource, Clone, Debug)]
pub struct FighterAssets {
    /// Running animation image.
    pub run_sheet: Handle<Image>,
    /// Running animation atlas layout.
    pub run_layout: Handle<TextureAtlasLayout>,
    /// Attack animation image.
    pub attack_sheet: Handle<Image>,
    /// Attack animation atlas layout.
    pub attack_layout: Handle<TextureAtlasLayout>,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Marks the player's fighter.
///
/// Used by the AI to find its target and convenient for connecting your own
/// input system.
#[derive(Component, Debug)]
pub struct Player;

/// A fighter's faction. Two entities in the same faction never hurt each other.
#[derive(Component, PartialEq, Eq, Clone, Copy, Debug)]
pub enum Faction {
    /// The player's faction.
    Player,
    /// The enemies' faction.
    Enemy,
}

/// A fighter's health points, with an invulnerability window after each hit.
#[derive(Component, Debug)]
pub struct Health {
    /// Current health points.
    pub current: f32,
    /// Maximum health points.
    pub max: f32,
    /// Time remaining before another hit can be taken (seconds).
    pub hit_cooldown: f32,
}

impl Health {
    /// Creates full health.
    pub fn full(max: f32) -> Self {
        Self { current: max, max, hit_cooldown: 0.0 }
    }

    /// `true` if the fighter has no health points left.
    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    /// Remaining health fraction, between 0.0 and 1.0 (useful for a health bar).
    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            0.0
        } else {
            (self.current / self.max).clamp(0.0, 1.0)
        }
    }

    /// Attempts to take a hit. Returns `true` if damage was applied, or
    /// `false` if the fighter is dead or invulnerable.
    pub fn try_hit(&mut self, damage: f32, invulnerability: f32) -> bool {
        if self.is_dead() || self.hit_cooldown > 0.0 {
            return false;
        }
        self.current = (self.current - damage).max(0.0);
        self.hit_cooldown = invulnerability;
        true
    }
}

/// Time remaining until the next possible attack (seconds).
#[derive(Component, Debug)]
pub struct AttackCooldown(pub f32);

/// A fighter's vulnerable area (AABB centered on its `GlobalTransform`).
#[derive(Component, Debug)]
pub struct Hurtbox {
    /// Half-size of the area, in world units.
    pub half_size: Vec2,
}

/// An area that deals damage.
///
/// Spawned as a child of the fighter to follow its position (and flip) via
/// `GlobalTransform`. It is only active during a few attack animation frames.
#[derive(Component, Debug)]
pub struct Hitbox {
    /// The entity that owns this hitbox (the attacker).
    pub owner: Entity,
    /// Owner's faction: targets in the same faction are ignored.
    pub faction: Faction,
    /// Half-size of the hitbox, in world units.
    pub half_size: Vec2,
    /// Damage dealt on a hit.
    pub damage: f32,
    /// `true` only during the active frame window.
    pub active: bool,
}

/// AI behavior: moves toward the player, stops at a distance, and attacks
/// when close enough. Intentionally basic.
#[derive(Component, Clone, Debug)]
pub struct AiController {
    /// Below this distance, the AI launches an attack.
    pub aggression_range: f32,
    /// Below this distance, the AI stops moving forward.
    pub stop_range: f32,
    /// Movement speed (world units per second).
    pub speed: f32,
}

impl Default for AiController {
    fn default() -> Self {
        Self { aggression_range: 90.0, stop_range: 55.0, speed: 180.0 }
    }
}

/// Indicates whether the fighter is moving this frame; the animation uses it
/// to choose between running and a stationary pose.
#[derive(Component, Debug)]
pub struct WantsToMove(pub bool);

/// A fighter's animation state.
#[derive(Component, Debug)]
pub struct AnimationTimer {
    /// Frame change timer.
    pub timer: Timer,
    /// Current frame in the active animation.
    pub frame_index: usize,
    /// Number of frames in the active animation.
    pub max_frames: usize,
}

/// The possible states of a fighter.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum FighterState {
    /// Running / moving (or stationary pose).
    Running,
    /// Attack in progress (cannot be interrupted).
    Attacking,
}

/// A fighter's current state.
#[derive(Component, Debug)]
pub struct CurrentState(pub FighterState);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_starts_full() {
        let hp = Health::full(100.0);
        assert_eq!(hp.current, 100.0);
        assert!(!hp.is_dead());
        assert_eq!(hp.fraction(), 1.0);
    }

    #[test]
    fn try_hit_applies_damage_then_invulnerability() {
        let mut hp = Health::full(100.0);
        assert!(hp.try_hit(30.0, 0.4));
        assert_eq!(hp.current, 70.0);
        // still invulnerable
        assert!(!hp.try_hit(30.0, 0.4));
        assert_eq!(hp.current, 70.0);
    }

    #[test]
    fn health_never_goes_negative() {
        let mut hp = Health::full(10.0);
        assert!(hp.try_hit(50.0, 0.0));
        assert_eq!(hp.current, 0.0);
        assert!(hp.is_dead());
        assert!(!hp.try_hit(5.0, 0.0));
    }

    #[test]
    fn fraction_handles_zero_max() {
        assert_eq!(Health::full(0.0).fraction(), 0.0);
    }
}