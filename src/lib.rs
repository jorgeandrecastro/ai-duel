// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Jorge Andre Castro <georgeandrec@gmail.com>

//! Plugin [Bevy](https://bevyengine.org) pour un duel joueur / IA.
//!
//! Deux combattants (ou plus, avec plusieurs ennemis), de la santé, des
//! factions pour savoir qui peut blesser qui, et une IA simple qui fonce sur
//! le joueur et attaque dès qu'elle est à portée.
//!
//! # Ce que le plugin gère
//!
//! - le comportement de l'IA ;
//! - les animations (course / attaque) ;
//! - la hitbox d'attaque, les dégâts et l'invulnérabilité après un coup ;
//! - la mort des combattants.
//!
//! # Ce qu'il ne gère pas
//!
//! - le chargement des assets : vous devez construire la ressource
//!   [`FighterAssets`] ;
//! - l'input du joueur : appelez [`start_attack`] depuis votre propre système,
//!   planifié `.before(`[`AiDuelSet`]`)`.
//!
//! # Organisation du code
//!
//! | Module         | Contenu                                         |
//! |----------------|-------------------------------------------------|
//! | `components`   | configuration, assets, composants               |
//! | `systems`      | systèmes exécutés par le plugin                 |
//! | racine (`lib`) | plugin, [`AiDuelSet`], [`spawn_fighter`], [`start_attack`] |
//!
//! # Exemple minimal
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
//!     // le joueur, à gauche
//!     spawn_fighter(&mut commands, &assets, &config,
//!         Vec3::new(-250.0, 0.0, 0.0), Faction::Player, None);
//!
//!     // un ennemi piloté par l'IA, à droite
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

/// Import pratique : `use ai_duel::prelude::*;`.
pub mod prelude {
    pub use crate::components::*;
    pub use crate::{spawn_fighter, start_attack, AiDuelPlugin, AiDuelSet};
}

/// Ensemble de systèmes du duel, exécutés dans l'ordre
/// cooldowns → IA → animation → hitbox → collisions → santé → mort.
///
/// Utilisez-le pour ordonner vos propres systèmes par rapport au plugin :
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

/// À ajouter après `DefaultPlugins`. Enregistre la configuration par défaut
/// ([`AiDuelConfig`]) et tous les systèmes du duel.
///
/// Les systèmes sont chaînés (`.chain()`) car chaque étape dépend du résultat
/// de la précédente.
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

/// Bascule un combattant en attaque : change l'animation et remet le compteur
/// de frame à zéro. Ne fait rien s'il attaque déjà (une attaque n'est pas
/// interruptible).
///
/// À appeler depuis votre système d'input pour piloter le joueur.
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

/// Spawne un combattant complet (sprite, vie, hurtbox, cooldown, état...) et
/// sa hitbox en enfant, inactive au départ.
///
/// Passez `ai = Some(...)` pour un ennemi piloté automatiquement, ou `None`
/// pour un combattant que vous contrôlez vous-même (typiquement le joueur).
/// Retourne l'entité du combattant.
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

    // Le décalage est exprimé en unités monde ; la transformation locale est
    // divisée par l'échelle du parent, qui la multiplie (et la retourne
    // automatiquement quand le sprite est tourné vers la gauche).
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