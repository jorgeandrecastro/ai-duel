// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Jorge Andre Castro <georgeandrec@gmail.com>

//! A [Bevy](https://bevyengine.org) plugin for player vs. AI duels.
//!
//! Two fighters (or more, with multiple enemies), health, factions that
//! determine who can hurt whom, and a simple AI that rushes toward the player
//! and attacks when in range.
//!
//! # What the plugin handles
//!
//! - AI behavior;
//! - animations (running / attacking);
//! - attack hitboxes, damage, and invulnerability after a hit;
//! - fighter death.
//!
//! # What it does not handle
//!
//! - asset loading: you must construct the [`FighterAssets`] resource;
//! - player input: call [`start_attack`] from your own system,
//!   scheduled `.before(`[`AiDuelSet`]`)`.
//!
//! # Code organization
//!
//! | Module       | Contents                                                   |
//! |--------------|------------------------------------------------------------|
//! | `components` | configuration, assets, components                          |
//! | `systems`    | systems run by the plugin                                  |
//! | root (`lib`) | plugin, [`AiDuelSet`], [`spawn_fighter`], [`start_attack`] |
//!
//! # Minimal example
//!
//! ```no_run
//! use bevy::prelude::*;
//! use ai_duel::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(AiDuelPlugin)
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(
//!     mut commands: Commands,
//!     asset_server: Res<AssetServer>,
//!     mut layouts: ResMut<Assets<TextureAtlasLayout>>,
//!     config: Res<AiDuelConfig>,
//! ) {
//!     commands.spawn(Camera2d);
//!
//!     let assets = FighterAssets {
//!         run_sheet: asset_server.load("run.png"),
//!         run_layout: layouts.add(TextureAtlasLayout::from_grid(
//!             UVec2::new(96, 80), 8, 1, None, None)),
//!         attack_sheet: asset_server.load("attack.png"),
//!         attack_layout: layouts.add(TextureAtlasLayout::from_grid(
//!             UVec2::new(96, 80), 6, 1, None, None)),
//!     };
//!
//!     // the player, on the left
//!     spawn_fighter(&mut commands, &assets, &config,
//!         Vec3::new(-250.0, 0.0, 0.0), Faction::Player, None);
//!
//!     // an AI-controlled enemy, on the right
//!     spawn_fighter(&mut commands, &assets, &config,
//!         Vec3::new(250.0, 0.0, 0.0), Faction::Enemy,
//!         Some(AiController::default()));
//!
//!     commands.insert_resource(assets);
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod components;
pub mod systems;

pub use components::*;

use bevy::prelude::*;

/// Convenient import: `use ai_duel::prelude::*;`.
pub mod prelude {
    pub use crate::components::*;
    pub use crate::{spawn_fighter, start_attack, AiDuelPlugin, AiDuelSet};
}

/// Duel systems, executed in order:
/// cooldowns → AI → animation → hitbox → collisions → health → death.
///
/// Use this to order your own systems relative to the plugin:
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use ai_duel::{AiDuelPlugin, AiDuelSet};
/// # fn player_input() {}
/// # fn main() {
/// App::new()
///     .add_plugins(AiDuelPlugin)
///     .add_systems(Update, player_input.before(AiDuelSet));
/// # }
/// ```
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AiDuelSet;

/// Add after `DefaultPlugins`. Registers the default configuration
/// ([`AiDuelConfig`]) and all duel systems.
///
/// Systems are chained (`.chain()`) because each step depends on the result
/// of the previous one.
pub struct AiDuelPlugin;

impl Plugin for AiDuelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AiDuelConfig>().add_systems(
            Update,
            (
                systems::tick_cooldowns,
                systems::ai_behavior,
                systems::animate_fighters,
                systems::update_hitboxes,
                systems::resolve_hits,
                systems::tick_health_and_flash,
                systems::check_death,
            )
                .chain()
                .in_set(AiDuelSet),
        );
    }
}

/// Switches a fighter to attacking: changes the animation and resets the
/// frame counter. Does nothing if already attacking (attacks cannot be
/// interrupted).
///
/// Call this from your input system to control the player.
pub fn start_attack(
    state: &mut CurrentState,
    anim: &mut AnimationTimer,
    sprite: &mut Sprite,
    assets: &FighterAssets,
    config: &AiDuelConfig,
) {
    if state.0 == FighterState::Attacking {
        return;
    }
    state.0 = FighterState::Attacking;
    anim.frame_index = 0;
    anim.max_frames = config.attack_frames.max(1);
    anim.timer = Timer::from_seconds(config.attack_frame_time, TimerMode::Repeating);

    if let Some(atlas) = &mut sprite.texture_atlas {
        atlas.layout = assets.attack_layout.clone();
        atlas.index = 0;
    }
    sprite.image = assets.attack_sheet.clone();
}

/// Spawns a complete fighter (sprite, health, hurtbox, cooldown, state...) and
/// its initially inactive hitbox as a child.
///
/// Pass `ai = Some(...)` for an automatically controlled enemy, or `None`
/// for a fighter you control yourself (typically the player).
/// Returns the fighter entity.
pub fn spawn_fighter(
    commands: &mut Commands,
    assets: &FighterAssets,
    config: &AiDuelConfig,
    pos: Vec3,
    faction: Faction,
    ai: Option<AiController>,
) -> Entity {
    let mut fighter = commands.spawn((
        Sprite::from_atlas_image(
            assets.run_sheet.clone(),
            TextureAtlas {
                layout: assets.run_layout.clone(),
                index: 0,
            },
        ),
        Transform::from_translation(pos).with_scale(Vec3::splat(config.sprite_scale)),
        CurrentState(FighterState::Running),
        AnimationTimer {
            timer: Timer::from_seconds(config.run_frame_time, TimerMode::Repeating),
            frame_index: 0,
            max_frames: config.run_frames.max(1),
        },
        faction,
        Health::full(config.default_max_health),
        Hurtbox { half_size: config.hurtbox_half_size },
        AttackCooldown(0.0),
        WantsToMove(false),
    ));

    if faction == Faction::Player {
        fighter.insert(Player);
    }
    if let Some(ai) = ai {
        fighter.insert(ai);
    }
    let parent = fighter.id();

    // The offset is in world units; the local transform is divided by the
    // parent's scale, which multiplies it (and automatically flips it when
    // the sprite faces left).
    let local_offset = config.hitbox_offset / config.sprite_scale.max(f32::EPSILON);

    let mut hitbox = commands.spawn((
        Transform::from_xyz(local_offset.x, local_offset.y, 0.5),
        Hitbox {
            owner: parent,
            faction,
            half_size: config.hitbox_half_size,
            damage: config.attack_damage,
            active: false,
        },
    ));
    if config.debug_hitboxes {
        hitbox.insert(Sprite {
            color: Color::srgba(1.0, 0.2, 0.2, 0.35),
            custom_size: Some(config.hitbox_half_size * 2.0 / config.sprite_scale),
            ..default()
        });
    }
    let hitbox = hitbox.id();

    commands.entity(parent).add_child(hitbox);
    parent
}