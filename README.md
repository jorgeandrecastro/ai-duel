# ai-duel

[![Crates.io](https://img.shields.io/crates/v/ai-duel.svg)](https://crates.io/crates/ai-duel)
[![docs.rs](https://img.shields.io/docsrs/ai-duel)](https://docs.rs/ai-duel)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20or%20Apache--2.0-blue.svg)](https://opensource.org/licenses/MIT)
[![Bevy 0.20](https://img.shields.io/badge/bevy-0.19-orange.svg)](https://bevyengine.org)

A [Bevy](https://bevyengine.org) plugin for quickly setting up a player vs. AI
duel: health, factions, attack hitboxes, and a basic AI that rushes toward the
player and strikes when in range.

Originally part of a personal prototype, it was extracted into a separate crate
for reuse across projects.

## Contents

- [Features](#features)
- [Limitations](#limitations)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Configuration](#configuration)
- [Controlling the player](#controlling-the-player)
- [Debugging](#debugging)
- [Architecture](#architecture)
- [Design choices](#design-choices)
- [Changelog](#changelog)
- [License](#️-license)
- [About](#-about)

## Features

- **Fighters**: health, faction (`Player` / `Enemy`), running and attack
  animations.
- **AI** (`AiController`): moves toward the player, stops at a specified
  distance, faces its target, and attacks in range.
- **Hitbox / hurtbox**: the hitbox is active during a configurable frame
  window, offset in front of the fighter and flipped with the sprite. A hit
  requires AABB overlap with the target's hurtbox.
- Brief **invulnerability** and a red flash after each hit.
- **Death**: a fighter at 0 HP is removed along with its hitbox.
- **`AiDuelSet`**: a system set for ordering your own systems.

## Limitations

- **No asset loading**: the user constructs `FighterAssets` with their own
  spritesheets.
- **No keyboard input**: you must call `start_attack()` and move the player's
  `Transform` yourself (see [Controlling the player](#controlling-the-player)).
- **No idle or death animation, no pathfinding**: the AI moves in a straight
  line toward the player.
- **O(n) collisions**: no acceleration structure. More than enough for a duel
  or a few enemies, but not for a horde.
- **One hitbox per fighter**, created on spawn.
- **One player only**: `ai_behavior` uses `Query::single`; with multiple
  `Player` entities, the AI does nothing.
- **No networking**: everything is local.

## Installation

```toml
[dependencies]
ai-duel = "0.3"
bevy = "0.20"
```

## Quick start

```rust
use bevy::prelude::*;
use ai_duel::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(AiDuelPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    config: Res<AiDuelConfig>,
) {
    commands.spawn(Camera2d);

    let assets = FighterAssets {
        run_sheet: asset_server.load("run.png"),
        run_layout: layouts.add(TextureAtlasLayout::from_grid(
            UVec2::new(96, 80), 8, 1, None, None)),
        attack_sheet: asset_server.load("attack.png"),
        attack_layout: layouts.add(TextureAtlasLayout::from_grid(
            UVec2::new(96, 80), 6, 1, None, None)),
    };

    // the player, on the left
    spawn_fighter(&mut commands, &assets, &config,
        Vec3::new(-250.0, 0.0, 0.0), Faction::Player, None);

    // an AI-controlled enemy, on the right
    spawn_fighter(&mut commands, &assets, &config,
        Vec3::new(250.0, 0.0, 0.0), Faction::Enemy,
        Some(AiController {
            aggression_range: 90.0,
            stop_range: 55.0,
            speed: 180.0,
        }));

    commands.insert_resource(assets);
}
```

Spritesheets are expected to contain a single row of frames (running and
attacking in separate images). Adjust `TextureAtlasLayout` for other formats.

> **AI range.** With the default config, an attack can hit up to approximately
> 100 units away (offset 45 + hitbox half-size 35 + hurtbox half-size 20).
> Keep `aggression_range` below that, or the AI will strike at empty space.

## Configuration

All settings are in `AiDuelConfig`. The plugin inserts a default config;
replace it after adding the plugin:

```rust
App::new()
    .add_plugins(AiDuelPlugin)
    .insert_resource(AiDuelConfig {
        attack_damage: 25.0,
        attack_cooldown: 0.4,
        ..default()
    })
    .run();
```

| Field                                | Default       | Purpose                                      |
|--------------------------------------|---------------|----------------------------------------------|
| `attack_damage`                      | `10.0`        | Damage per hit                               |
| `attack_cooldown`                    | `0.60`        | Minimum delay between attacks (s)            |
| `hit_cooldown`                       | `0.40`        | Invulnerability after a hit (s)              |
| `hitbox_active_from` / `_to`         | `2` / `4`     | Frames in which the hitbox is active         |
| `hitbox_half_size`                   | `(35, 30)`    | Hitbox half-size                             |
| `hitbox_offset`                      | `(45, 0)`     | Offset in front of the fighter               |
| `hurtbox_half_size`                  | `(20, 35)`    | Vulnerable area half-size                    |
| `debug_hitboxes`                     | `false`       | Displays hitboxes                            |
| `default_max_health`                 | `100.0`       | Starting HP                                  |
| `sprite_scale`                       | `1.5`         | Sprite scale                                 |

Distances are in world units. See the `AiDuelConfig` documentation for the
complete list.

## Controlling the player

The plugin does not change the player's `Transform`. Schedule your system
`.before(AiDuelSet)` so the attack is processed in the same frame:

```rust
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    assets: Res<FighterAssets>,
    config: Res<AiDuelConfig>,
    mut q: Query<
        (&mut Transform, &mut CurrentState, &mut AnimationTimer, &mut Sprite, &mut WantsToMove),
        With<Player>,
    >,
) {
    let Ok((mut tf, mut state, mut anim, mut sprite, mut wants)) = q.single_mut() else {
        return;
    };

    if keys.just_pressed(KeyCode::Space) {
        start_attack(&mut state, &mut anim, &mut sprite, &assets, &config);
        wants.0 = false;
        return;
    }

    // no movement during an attack
    if state.0 == FighterState::Attacking {
        wants.0 = false;
        return;
    }

    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowRight) { dir.x += 1.0; }
    if keys.pressed(KeyCode::ArrowLeft)  { dir.x -= 1.0; }

    wants.0 = dir != Vec2::ZERO;
    if dir.x != 0.0 {
        // flip the sprite (and therefore the hitbox) according to direction
        tf.scale.x = config.sprite_scale * dir.x.signum();
    }
    tf.translation += (dir.normalize_or_zero() * 200.0 * time.delta_secs()).extend(0.0);
}

// in main():
// .add_systems(Update, player_input.before(AiDuelSet))
```

## Debugging

Set `debug_hitboxes: true` in `AiDuelConfig` to display hitboxes in translucent
red. Hits and deaths are logged at the `debug` level
(`RUST_LOG=ai_duel=debug`).

## Architecture

```
src/
├── lib.rs          plugin, AiDuelSet, spawn_fighter, start_attack, prelude
├── components.rs   AiDuelConfig, FighterAssets, components (Health, Hitbox...)
└── systems.rs      plugin systems (AI, animation, collisions, death)
```

The execution order is: cooldowns → AI → animation → hitbox → collisions →
health/flash → death.

## Design choices

- **Fixed states (`Running` / `Attacking`)** rather than a generic state
  machine: a duel only needs two states.
- **Hitbox as a child entity**: the inherited `GlobalTransform` follows the
  fighter's position and flip without manual calculations.
- **Active frame window** rather than a hitbox active throughout the attack:
  the strike does not deal damage before the arm moves.
- **Hitbox + hurtbox (AABB)** rather than a point-in-box test: the actual
  attack range depends on the size of both areas.
- **Chained systems (`.chain()`)**: each step depends on the result of the
  previous one; running them in parallel would cause frame-to-frame
  inconsistencies.
- **Public systems**: they can be replaced or reordered by the host
  application without forking the crate.

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## 🛡️ License

Licensed under either of:

    MIT license (http://opensource.org/licenses/MIT)

    Apache License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)

at your option.

## 🦅 About

Developed by Jorge Andre Castro.