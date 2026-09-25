// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Jorge Andre Castro <georgeandrec@gmail.com>

//! Plugin Bevy pour un duel joueur / IA.
//!
//! L'idée de base : deux combattants (ou plus, si vous souhaitez plusieurs
//! ennemis), une santé, une faction pour savoir qui peut taper qui, et une
//! IA toute simple qui fonce sur le joueur et l'attaque dès qu'elle est
//! assez proche.
//!
//! Ce que le plugin gère pour vous: le comportement de l'IA, les
//! animations (course / attaque), la hitbox d'attaque, les dégâts et la
//! mort des combattants. Ce qu'il ne gère PAS : le chargement des assets
//! (vous devez créer la ressource [`FighterAssets`] vous-mêmes) et l'input
//! clavier du joueur (rien ne vous empêche d'appeler [`start_attack`] depuis
//! votre propre système).
//!
//! Exemple minimal avec un joueur et un ennemi :
//!
//! ```no_run
//! use bevy::prelude::*;
//! use ai_duel::{
//!     AiDuelPlugin, AiDuelConfig, FighterAssets, Faction, AiController,
//!     spawn_fighter,
//! };
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
//!     // un ennemi contrôlé par l'IA, à droite
//!     spawn_fighter(&mut commands, &assets, &config,
//!         Vec3::new(250.0, 0.0, 0.0), Faction::Enemy,
//!         Some(AiController {
//!             aggression_range: 110.0,
//!             stop_range: 55.0,
//!             speed: 180.0,
//!         }));
//!
//!     commands.insert_resource(assets);
//! }
//! ```

use bevy::prelude::*;

// Config

/// Tous les réglages du duel au même endroit (cooldowns, dégâts, vitesse
/// d'animation...). Le plugin en insèrant une copie par défaut ; si vous voulez
/// changer un truc, insèrez la votre juste après avoir ajouté le plugin :
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
#[derive(Resource, Clone, Debug)]
pub struct AiDuelConfig {
    // animation de course
    pub run_frames: usize,
    pub run_frame_time: f32,

    // animation d'attaque
    pub attack_frames: usize,
    pub attack_frame_time: f32,

    /// Temps mini entre deux attaques d'un même combattant.
    pub attack_cooldown: f32,
    /// Combien de temps un combattant reste invulnérable après un coup
    /// (histoire d'éviter qu'il se fasse hit sur 3 frames d'affilée).
    pub hit_cooldown: f32,
    pub attack_damage: f32,

    /// La hitbox n'est active que sur cette plage de frames de l'anim
    /// d'attaque (sinon on prendrait des dégâts dès la première frame).
    pub hitbox_active_from: usize,
    pub hitbox_active_to: usize,
    /// Demi-taille de la hitbox (AABB), en unités monde.
    pub hitbox_half_size: Vec2,

    pub default_max_health: f32,
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
            default_max_health: 100.0,
            sprite_scale: 1.5,
        }
    }
}

// Assets

/// Les deux spritesheets sont partagées par tous les combattants (course et
/// attaque). Pas besoin d'une copie par entité, tout le monde pointe sur
/// les mêmes handles.
#[derive(Resource, Clone, Debug)]
pub struct FighterAssets {
    pub run_sheet: Handle<Image>,
    pub run_layout: Handle<TextureAtlasLayout>,
    pub attack_sheet: Handle<Image>,
    pub attack_layout: Handle<TextureAtlasLayout>,
}


// Composants

/// Marque le combattant du joueur. Optionnel côté logique du plugin (il
/// sert surtout à repérer "qui est le joueur" dans `ai_behavior`), mais
/// pratique aussi pour brancher votre  propre système d'input dessus.
#[derive(Component, Debug)]
pub struct Player;

/// Le camp d'un combattant. Deux entités de la même faction ne se font
/// jamais mal entre elles (voir `resolve_hits`).
#[derive(Component, PartialEq, Eq, Clone, Copy, Debug)]
pub enum Faction {
    Player,
    Enemy,
}

/// Vie d'un combattant, avec une petite fenêtre d'invulnérabilité après
/// chaque coup encaissé.
#[derive(Component, Debug)]
pub struct Health {
    pub current: f32,
    pub max: f32,
    /// Temps restant avant de pouvoir reprendre un coup, en secondes.
    pub hit_cooldown: f32,
}

impl Health {
    pub fn full(max: f32) -> Self {
        Self { current: max, max, hit_cooldown: 0.0 }
    }
}

/// Temps restant avant la prochaine attaque possible.
#[derive(Component, Debug)]
pub struct AttackCooldown(pub f32);

/// La zone qui fait mal, spawnée en enfant du combattant pour suivre sa
/// position automatiquement via le `GlobalTransform`. Elle n'est active
/// que sur quelques frames de l'anim d'attaque (voir `update_hitboxes`).
#[derive(Component, Debug)]
pub struct Hitbox {
    /// L'entité qui possède cette hitbox (celle qui frappe).
    pub owner: Entity,
    pub faction: Faction,
    pub half_size: Vec2,
    pub damage: f32,
    pub active: bool,
}

/// Comportement de l'IA : avance vers le joueur, s'arrête à distance et
/// attaque quand c'est assez proche. Volontairement basique,pour une version 0.1.
#[derive(Component, Debug)]
pub struct AiController {
    /// En dessous de cette distance, l'IA lance une attaque.
    pub aggression_range: f32,
    /// En dessous de cette distance, l'IA arrête d'avancer (évite qu'elle
    /// colle le joueur).
    pub stop_range: f32,
    pub speed: f32,
}

/// Flag interne pour savoir si le combattant est en train de se déplacer
/// cette frame, utilisé par l'anim pour choisir entre course et pose fixe.
#[derive(Component, Debug)]
pub struct WantsToMove(pub bool);

#[derive(Component, Debug)]
pub struct AnimationTimer {
    pub timer: Timer,
    pub frame_index: usize,
    pub max_frames: usize,
}

/// Les deux seuls états possibles d'un combattant pour l'instant.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum FighterState {
    Running,
    Attacking,
}

#[derive(Component, Debug)]
pub struct CurrentState(pub FighterState);

// Plugin

/// À ajouter après `DefaultPlugins`. Enregistre la config par défaut et
/// tous les systèmes du duel, dans l'ordre où ils doivent tourner (l'IA
/// avant l'anim, l'anim avant la hitbox, etc. d'où le `.chain()`).
///
/// Ce que le plugin ne fait pas à votre place : fournir les assets
/// ([`FighterAssets`]), spawner les combattants (utilise [`spawn_fighter`])
/// et gérer l'input clavier du joueur.
pub struct AiDuelPlugin;

impl Plugin for AiDuelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AiDuelConfig>()
            .add_systems(
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
                    .chain(),
            );
    }
}


// API publique

/// Bascule un combattant en attaque : change l'anim, remet le compteur de
/// frame à zéro. Ne fait rien s'il attaque déjà (pas d'interruption
/// d'attaque possible pour l'instant). C'est la fonction à appeler depuis
/// ton système d'input si tu veux gérer le joueur toi-même.
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
    anim.max_frames = config.attack_frames;
    anim.timer = Timer::from_seconds(config.attack_frame_time, TimerMode::Repeating);

    if let Some(atlas) = &mut sprite.texture_atlas {
        atlas.layout = assets.attack_layout.clone();
        atlas.index = 0;
    }
    sprite.image = assets.attack_sheet.clone();
}

/// Spawn un combattant complet (sprite, vie, cooldown, état...) et sa
/// hitbox en enfant, inactive au départ. Passe `ai = Some(...)` pour un
/// ennemi piloté automatiquement, ou `None` pour un combattant que vous
/// contrôlerez par vous memes(typiquement le joueur).
pub fn spawn_fighter(
    commands: &mut Commands,
    assets: &FighterAssets,
    config: &AiDuelConfig,
    pos: Vec3,
    faction: Faction,
    ai: Option<AiController>,
) -> Entity {
    let mut ec = commands.spawn((
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
            max_frames: config.run_frames,
        },
        faction,
        Health::full(config.default_max_health),
        AttackCooldown(0.0),
        WantsToMove(false),
    ));

    if faction == Faction::Player {
        ec.insert(Player);
    }
    if let Some(ai) = ai {
        ec.insert(ai);
    }

    let parent = ec.id();

    // hitbox transparente au départ, on ne la voit jamais (l'alpha est à 0) :
    // custom_size sert juste pour du debug visuel si besoin de la réactiver
    let hitbox = commands
        .spawn((
            Sprite {
                color: Color::srgba(1.0, 0.2, 0.2, 0.0),
                custom_size: Some(config.hitbox_half_size * 2.0),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 0.5),
            Hitbox {
                owner: parent,
                faction,
                half_size: config.hitbox_half_size,
                damage: config.attack_damage,
                active: false,
            },
        ))
        .id();

    commands.entity(parent).add_child(hitbox);

    parent
}

// Systèmes

pub mod systems {
    //! Les systèmes ajoutés par [`crate::AiDuelPlugin`]. Publics pour que
    //! vous  puissiez les ré-ordonnancer ou les remplacer dans votre `App` si
    //! besoin, mais dans 90% des cas vous n'aurez rien à y toucher.

    use super::*;

    /// Fait descendre les cooldowns d'attaque à chaque frame.
    pub fn tick_cooldowns(time: Res<Time>, mut q: Query<&mut AttackCooldown>) {
        for mut cd in &mut q {
            cd.0 = (cd.0 - time.delta_secs()).max(0.0);
        }
    }

    /// Le cerveau de l'IA : fonce sur le joueur, s'arrête à `stop_range`,
    /// attaque dès qu'elle passe sous `aggression_range` (si le cooldown le
    /// permet). Rien de très malin, mais ça fait le job pour un duel.
    pub fn ai_behavior(
        time: Res<Time>,
        assets: Res<FighterAssets>,
        config: Res<AiDuelConfig>,
        mut ai_q: Query<
            (
                &mut Transform,
                &mut CurrentState,
                &mut AnimationTimer,
                &mut Sprite,
                &mut WantsToMove,
                &mut AttackCooldown,
                &AiController,
            ),
            Without<Player>,
        >,
        player_q: Query<&Transform, With<Player>>,
    ) {
        // pas de joueur sur la carte -> rien à poursuivre
        let Ok(player_tf) = player_q.single() else {
            return;
        };

        for (mut tf, mut state, mut anim, mut sprite, mut wants, mut cd, ai) in &mut ai_q {
            if state.0 == FighterState::Attacking {
                // on ne bouge pas pendant l'animation d'attaque
                wants.0 = false;
                continue;
            }

            let to_player = player_tf.translation.truncate() - tf.translation.truncate();
            let dist = to_player.length();

            if dist < ai.aggression_range && cd.0 <= 0.0 {
                crate::start_attack(&mut state, &mut anim, &mut sprite, &assets, &config);
                cd.0 = config.attack_cooldown;
                wants.0 = false;
                continue;
            }

            if dist > ai.stop_range {
                let dir = to_player.normalize_or_zero();
                tf.translation.x += dir.x * ai.speed * time.delta_secs();
                tf.translation.y += dir.y * ai.speed * time.delta_secs();
                wants.0 = true;

                // on retourne le sprite selon la direction pour éviter
                // qu'il coure "à l'envers"
                if dir.x.abs() > 0.1 {
                    tf.scale.x = config.sprite_scale * dir.x.signum();
                }
            } else {
                wants.0 = false;
            }
        }
    }

    /// Avance les timers d'anim et change de frame quand il le faut. Gère
    /// aussi le retour à l'état "course" une fois l'anim d'attaque finie.
    pub fn animate_fighters(
        time: Res<Time>,
        assets: Res<FighterAssets>,
        config: Res<AiDuelConfig>,
        mut q: Query<(
            &mut AnimationTimer,
            &mut Sprite,
            &mut CurrentState,
            &WantsToMove,
        )>,
    ) {
        for (mut anim, mut sprite, mut state, wants) in &mut q {
            anim.timer.tick(time.delta());

            if !anim.timer.just_finished() {
                continue;
            }
            anim.frame_index += 1;

            if state.0 == FighterState::Attacking {
                if anim.frame_index >= anim.max_frames {
                    // attaque terminée, retour à la course
                    state.0 = FighterState::Running;
                    anim.frame_index = 0;
                    anim.max_frames = config.run_frames;
                    anim.timer =
                        Timer::from_seconds(config.run_frame_time, TimerMode::Repeating);

                    if let Some(atlas) = &mut sprite.texture_atlas {
                        atlas.layout = assets.run_layout.clone();
                        atlas.index = 0;
                    }
                    sprite.image = assets.run_sheet.clone();
                } else if let Some(atlas) = &mut sprite.texture_atlas {
                    atlas.index = anim.frame_index;
                }
            } else if wants.0 {
                anim.frame_index %= anim.max_frames;
                if let Some(atlas) = &mut sprite.texture_atlas {
                    atlas.index = anim.frame_index;
                }
            }
            // si le combattant ne bouge pas, on reste sur la frame actuelle
            // (pas d'anim d'idle pour l'instant)
        }
    }

    /// Active la hitbox uniquement sur la fenêtre de frames définie dans
    /// la config, pour que le coup ne touche pas dès le tout début de
    /// l'anim d'attaque.
    pub fn update_hitboxes(
        config: Res<AiDuelConfig>,
        mut hitbox_q: Query<&mut Hitbox>,
        fighter_q: Query<(&CurrentState, &AnimationTimer)>,
    ) {
        for mut hb in &mut hitbox_q {
            let Ok((state, anim)) = fighter_q.get(hb.owner) else {
                continue;
            };
            hb.active = state.0 == FighterState::Attacking
                && anim.frame_index >= config.hitbox_active_from
                && anim.frame_index <= config.hitbox_active_to;
        }
    }

    /// Collision toute bête entre AABB : pour chaque hitbox active, on
    /// regarde qui est dedans et n'est pas de la même faction. Suffisant
    /// pour un duel 1v1, à revoir si tu veux gérer beaucoup d'entités.
    pub fn resolve_hits(
        hitbox_q: Query<(&Hitbox, &GlobalTransform)>,
        mut target_q: Query<(&GlobalTransform, &Faction, &mut Health)>,
    ) {
        for (hb, hb_tf) in &hitbox_q {
            if !hb.active {
                continue;
            }

            let center = hb_tf.translation().truncate();
            let half = hb.half_size;

            for (tf, faction, mut hp) in &mut target_q {
                if *faction == hb.faction {
                    continue; // pas de dégâts alliés
                }
                if hp.current <= 0.0 || hp.hit_cooldown > 0.0 {
                    continue; // déjà mort ou encore invulnérable
                }

                let p = tf.translation().truncate();
                let overlap =
                    (p.x - center.x).abs() < half.x && (p.y - center.y).abs() < half.y;

                if overlap {
                    hp.current = (hp.current - hb.damage).max(0.0);
                    hp.hit_cooldown = 0.4;
                    info!(
                        "Touché ! HP restants : {:.0}/{:.0}",
                        hp.current, hp.max
                    );
                }
            }
        }
    }

    /// Décompte l'invulnérabilité après un coup et fait clignoter le
    /// sprite en rouge pendant ce temps, juste pour que ce soit visible
    /// à l'écran qu'on vient de se prendre un coup.
    pub fn tick_health_and_flash(time: Res<Time>, mut q: Query<(&mut Health, &mut Sprite)>) {
        for (mut hp, mut sprite) in &mut q {
            if hp.hit_cooldown > 0.0 {
                hp.hit_cooldown = (hp.hit_cooldown - time.delta_secs()).max(0.0);
                sprite.color = Color::srgba(1.0, 0.3, 0.3, 0.85);
            } else {
                sprite.color = Color::WHITE;
            }
        }
    }

    /// Supprime les combattants à 0 PV. Pas d'anim de mort pour l'instant,
    /// l'entité disparaît directement.
    pub fn check_death(mut commands: Commands, q: Query<(Entity, &Health)>) {
        for (e, hp) in &q {
            if hp.current <= 0.0 {
                info!("Un combattant est tombé !");
                commands.entity(e).despawn();
            }
        }
    }
}