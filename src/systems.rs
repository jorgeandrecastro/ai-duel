// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Jorge Andre Castro <georgeandrec@gmail.com>

//! The systems added by [`AiDuelPlugin`](crate::AiDuelPlugin).
//!
//! They are public so you can reorder or replace them in your `App`, but in
//! most cases you will not need to change them.

use bevy::prelude::*;

use crate::components::*;
use crate::start_attack;

/// Tests overlap between two AABBs defined by their centers and half-sizes.
pub fn aabb_overlap(center_a: Vec2, half_a: Vec2, center_b: Vec2, half_b: Vec2) -> bool {
    (center_a.x - center_b.x).abs() < half_a.x + half_b.x
        && (center_a.y - center_b.y).abs() < half_a.y + half_b.y
}

/// Decreases attack cooldowns each frame.
pub fn tick_cooldowns(time: Res<Time>, mut q: Query<&mut AttackCooldown>) {
    for mut cd in &mut q {
        cd.0 = (cd.0 - time.delta_secs()).max(0.0);
    }
}

/// The AI's brain: rushes toward the player, stops at `stop_range`, and
/// attacks below `aggression_range` (if the cooldown allows it).
/// Without a player on the map, the AI does nothing.
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
    let Ok(player_tf) = player_q.single() else {
        return;
    };
    let player_pos = player_tf.translation.truncate();

    for (mut tf, mut state, mut anim, mut sprite, mut wants, mut cd, ai) in &mut ai_q {
        // No movement during the attack animation.
        if state.0 == FighterState::Attacking {
            wants.0 = false;
            continue;
        }

        let to_player = player_pos - tf.translation.truncate();
        let dist = to_player.length();
        let dir = to_player.normalize_or_zero();

        // Always face the target, even when stopped or before an attack
        // (otherwise the hitbox would appear on the wrong side).
        if dir.x.abs() > 0.1 {
            tf.scale.x = config.sprite_scale * dir.x.signum();
        }

        if dist < ai.aggression_range && cd.0 <= 0.0 {
            start_attack(&mut state, &mut anim, &mut sprite, &assets, &config);
            cd.0 = config.attack_cooldown;
            wants.0 = false;
            continue;
        }

        if dist > ai.stop_range {
            let step = dir * ai.speed * time.delta_secs();
            tf.translation.x += step.x;
            tf.translation.y += step.y;
            wants.0 = true;
        } else {
            wants.0 = false;
        }
    }
}

/// Advances animation timers and changes frames when needed. Also returns to
/// the running state once the attack is finished.
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
                // Attack finished: return to running.
                state.0 = FighterState::Running;
                anim.frame_index = 0;
                anim.max_frames = config.run_frames.max(1);
                anim.timer = Timer::from_seconds(config.run_frame_time, TimerMode::Repeating);

                if let Some(atlas) = &mut sprite.texture_atlas {
                    atlas.layout = assets.run_layout.clone();
                    atlas.index = 0;
                }
                sprite.image = assets.run_sheet.clone();
            } else if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = anim.frame_index;
            }
        } else if wants.0 {
            anim.frame_index %= anim.max_frames.max(1);
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = anim.frame_index;
            }
        }
        // Stationary fighter: keep the current frame (no idle animation).
    }
}

/// Activates the hitbox only during the frame window defined in the config,
/// so the attack does not hit at the start of the animation.
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
            && (config.hitbox_active_from..=config.hitbox_active_to).contains(&anim.frame_index);
    }
}

/// AABB vs. AABB collision: for each active hitbox, applies damage to
/// opposing fighters whose hurtboxes overlap it.
///
/// Complexity O(active hitboxes × fighters): more than enough for a duel or
/// a few enemies, but not designed for a horde.
pub fn resolve_hits(
    config: Res<AiDuelConfig>,
    hitbox_q: Query<(&Hitbox, &GlobalTransform)>,
    mut target_q: Query<(&GlobalTransform, &Faction, &Hurtbox, &mut Health)>,
) {
    for (hb, hb_tf) in &hitbox_q {
        if !hb.active {
            continue;
        }
        let center = hb_tf.translation().truncate();

        for (tf, faction, hurtbox, mut hp) in &mut target_q {
            if *faction == hb.faction {
                continue; // no friendly damage
            }
            let target = tf.translation().truncate();
            if aabb_overlap(center, hb.half_size, target, hurtbox.half_size)
                && hp.try_hit(hb.damage, config.hit_cooldown)
            {
                debug!("Hit! Health remaining: {:.0}/{:.0}", hp.current, hp.max);
            }
        }
    }
}

/// Counts down invulnerability after a hit and tints the sprite red during
/// that time, making the hit visible on screen.
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

/// Removes fighters at 0 health (and their hitbox, a child of the entity).
/// There is no death animation yet.
pub fn check_death(mut commands: Commands, q: Query<(Entity, &Health)>) {
    for (entity, hp) in &q {
        if hp.is_dead() {
            debug!("A fighter has fallen!");
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_boxes_are_detected() {
        assert!(aabb_overlap(Vec2::ZERO, Vec2::splat(10.0), Vec2::new(15.0, 0.0), Vec2::splat(10.0)));
    }

    #[test]
    fn distant_boxes_do_not_overlap() {
        assert!(!aabb_overlap(Vec2::ZERO, Vec2::splat(10.0), Vec2::new(25.0, 0.0), Vec2::splat(10.0)));
        assert!(!aabb_overlap(Vec2::ZERO, Vec2::splat(10.0), Vec2::new(0.0, 25.0), Vec2::splat(10.0)));
    }

    #[test]
    fn touching_edges_do_not_count() {
        assert!(!aabb_overlap(Vec2::ZERO, Vec2::splat(10.0), Vec2::new(20.0, 0.0), Vec2::splat(10.0)));
    }
}