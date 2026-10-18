// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Jorge Andre Castro <georgeandrec@gmail.com>

//! Ressources, composants et configuration du plugin.

use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Tous les réglages du duel au même endroit (cooldowns, dégâts, vitesse
/// d'animation, hitbox...).
///
/// Le plugin insère une copie par défaut ; pour modifier un réglage,
/// insérez la vôtre juste après avoir ajouté le plugin :
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
/// Les distances sont exprimées en **unités monde** (donc après application
/// de [`sprite_scale`](Self::sprite_scale)).
#[derive(Resource, Clone, Debug)]
pub struct AiDuelConfig {
    /// Nombre de frames de l'animation de course.
    pub run_frames: usize,
    /// Durée d'une frame de course, en secondes.
    pub run_frame_time: f32,

    /// Nombre de frames de l'animation d'attaque.
    pub attack_frames: usize,
    /// Durée d'une frame d'attaque, en secondes.
    pub attack_frame_time: f32,

    /// Temps minimal entre deux attaques d'un même combattant (secondes).
    pub attack_cooldown: f32,
    /// Durée d'invulnérabilité après un coup encaissé (secondes), pour
    /// éviter d'être touché plusieurs fois par la même attaque.
    pub hit_cooldown: f32,
    /// Dégâts infligés par une attaque.
    pub attack_damage: f32,

    /// Première frame de l'animation d'attaque où la hitbox est active.
    pub hitbox_active_from: usize,
    /// Dernière frame (incluse) où la hitbox est active.
    pub hitbox_active_to: usize,
    /// Demi-taille de la hitbox d'attaque (AABB).
    pub hitbox_half_size: Vec2,
    /// Décalage de la hitbox par rapport au centre du combattant, pour un
    /// combattant tourné vers la droite. Le décalage en X est
    /// automatiquement inversé quand le sprite est retourné.
    pub hitbox_offset: Vec2,
    /// Demi-taille de la zone où un combattant peut être touché (AABB).
    pub hurtbox_half_size: Vec2,
    /// Affiche les hitbox à l'écran (débogage).
    pub debug_hitboxes: bool,

    /// Points de vie d'un combattant à sa création.
    pub default_max_health: f32,
    /// Échelle appliquée aux sprites des combattants.
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

/// Les spritesheets (course et attaque), partagées par tous les combattants.
///
/// Chaque entité référence les mêmes handles : aucune copie par combattant.
/// Cette ressource doit être insérée par l'application hôte.
#[derive(Resource, Clone, Debug)]
pub struct FighterAssets {
    /// Image de l'animation de course.
    pub run_sheet: Handle<Image>,
    /// Découpage de l'animation de course.
    pub run_layout: Handle<TextureAtlasLayout>,
    /// Image de l'animation d'attaque.
    pub attack_sheet: Handle<Image>,
    /// Découpage de l'animation d'attaque.
    pub attack_layout: Handle<TextureAtlasLayout>,
}

// ---------------------------------------------------------------------------
// Composants
// ---------------------------------------------------------------------------

/// Marque le combattant du joueur.
///
/// Utilisé par l'IA pour trouver sa cible, et pratique pour brancher votre
/// propre système d'input.
#[derive(Component, Debug)]
pub struct Player;

/// Le camp d'un combattant. Deux entités de la même faction ne se blessent
/// jamais entre elles.
#[derive(Component, PartialEq, Eq, Clone, Copy, Debug)]
pub enum Faction {
    /// Camp du joueur.
    Player,
    /// Camp des ennemis.
    Enemy,
}

/// Points de vie d'un combattant, avec une fenêtre d'invulnérabilité après
/// chaque coup encaissé.
#[derive(Component, Debug)]
pub struct Health {
    /// Points de vie actuels.
    pub current: f32,
    /// Points de vie maximum.
    pub max: f32,
    /// Temps restant avant de pouvoir encaisser un nouveau coup (secondes).
    pub hit_cooldown: f32,
}

impl Health {
    /// Crée une santé pleine.
    pub fn full(max: f32) -> Self {
        Self { current: max, max, hit_cooldown: 0.0 }
    }

    /// `true` si le combattant n'a plus de points de vie.
    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    /// Fraction de vie restante, entre 0.0 et 1.0 (utile pour une barre de vie).
    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            0.0
        } else {
            (self.current / self.max).clamp(0.0, 1.0)
        }
    }

    /// Tente d'encaisser un coup. Retourne `true` si les dégâts ont été
    /// appliqués, `false` si le combattant est mort ou invulnérable.
    pub fn try_hit(&mut self, damage: f32, invulnerability: f32) -> bool {
        if self.is_dead() || self.hit_cooldown > 0.0 {
            return false;
        }
        self.current = (self.current - damage).max(0.0);
        self.hit_cooldown = invulnerability;
        true
    }
}

/// Temps restant avant la prochaine attaque possible (secondes).
#[derive(Component, Debug)]
pub struct AttackCooldown(pub f32);

/// Zone vulnérable d'un combattant (AABB centrée sur son `GlobalTransform`).
#[derive(Component, Debug)]
pub struct Hurtbox {
    /// Demi-taille de la zone, en unités monde.
    pub half_size: Vec2,
}

/// Zone qui inflige des dégâts.
///
/// Spawnée en enfant du combattant pour suivre sa position (et son
/// retournement) via le `GlobalTransform`. Elle n'est active que sur quelques
/// frames de l'animation d'attaque.
#[derive(Component, Debug)]
pub struct Hitbox {
    /// L'entité qui possède cette hitbox (celle qui frappe).
    pub owner: Entity,
    /// Faction du propriétaire : les cibles de même faction sont ignorées.
    pub faction: Faction,
    /// Demi-taille de la hitbox, en unités monde.
    pub half_size: Vec2,
    /// Dégâts infligés en cas de touche.
    pub damage: f32,
    /// `true` uniquement pendant la fenêtre de frames actives.
    pub active: bool,
}

/// Comportement de l'IA : avance vers le joueur, s'arrête à distance et
/// attaque quand elle est assez proche. Volontairement basique.
#[derive(Component, Clone, Debug)]
pub struct AiController {
    /// En dessous de cette distance, l'IA lance une attaque.
    pub aggression_range: f32,
    /// En dessous de cette distance, l'IA arrête d'avancer.
    pub stop_range: f32,
    /// Vitesse de déplacement (unités monde par seconde).
    pub speed: f32,
}

impl Default for AiController {
    fn default() -> Self {
        Self { aggression_range: 90.0, stop_range: 55.0, speed: 180.0 }
    }
}

/// Indique si le combattant se déplace cette frame ; l'animation s'en sert
/// pour choisir entre course et pose fixe.
#[derive(Component, Debug)]
pub struct WantsToMove(pub bool);

/// État d'animation d'un combattant.
#[derive(Component, Debug)]
pub struct AnimationTimer {
    /// Timer de changement de frame.
    pub timer: Timer,
    /// Frame courante dans l'animation en cours.
    pub frame_index: usize,
    /// Nombre de frames de l'animation en cours.
    pub max_frames: usize,
}

/// Les états possibles d'un combattant.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum FighterState {
    /// Course / déplacement (ou pose fixe).
    Running,
    /// Attaque en cours (non interruptible).
    Attacking,
}

/// État courant d'un combattant.
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
        // encore invulnérable
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