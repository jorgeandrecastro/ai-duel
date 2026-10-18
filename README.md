# ai-duel

[![Crates.io](https://img.shields.io/crates/v/ai-duel.svg)](https://crates.io/crates/ai-duel)
[![docs.rs](https://img.shields.io/docsrs/ai-duel)](https://docs.rs/ai-duel)
[![License: GPL v2+](https://img.shields.io/badge/license-GPL--2.0--or--later-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.html)
[![Bevy 0.20](https://img.shields.io/badge/bevy-0.19-orange.svg)](https://bevyengine.org)

Plugin [Bevy](https://bevyengine.org) pour monter rapidement un duel joueur
contre IA : santé, factions, hitbox d'attaque et une IA basique qui fonce sur
le joueur et frappe quand elle est à portée.

Né d'un prototype personnel, isolé dans une crate séparée car réutilisé d'un
projet à l'autre.

## Sommaire

- [Fonctionnalités](#fonctionnalités)
- [Limites](#limites)
- [Installation](#installation)
- [Démarrage rapide](#démarrage-rapide)
- [Configuration](#configuration)
- [Contrôler le joueur](#contrôler-le-joueur)
- [Débogage](#débogage)
- [Architecture](#architecture)
- [Choix de conception](#choix-de-conception)
- [Changelog](#changelog)
- [Licence](#licence)

## Fonctionnalités

- **Combattants** : vie, faction (`Player` / `Enemy`), animation de course et
  d'attaque.
- **IA** (`AiController`) : avance vers le joueur, s'arrête à une distance
  donnée, se tourne vers sa cible et attaque à portée.
- **Hitbox / hurtbox** : la hitbox est active sur une fenêtre de frames
  réglable, décalée devant le combattant et retournée avec le sprite. Une
  touche exige un recouvrement AABB avec la hurtbox de la cible.
- **Invulnérabilité** brève et flash rouge après chaque coup encaissé.
- **Mort** : un combattant à 0 PV est supprimé avec sa hitbox.
- **`AiDuelSet`** : ensemble de systèmes pour ordonner vos propres systèmes.

## Limites

- **Pas de chargement d'assets** : `FighterAssets` est construite par
  l'utilisateur avec ses propres spritesheets.
- **Pas d'input clavier** : à vous d'appeler `start_attack()` et de déplacer
  le `Transform` du joueur (voir [Contrôler le joueur](#contrôler-le-joueur)).
- **Pas d'animation d'idle ni de mort, pas de pathfinding** : l'IA va en ligne
  droite vers le joueur.
- **Collisions en O(n)** : pas de structure d'accélération. Largement
  suffisant pour un duel ou quelques ennemis, pas pour une horde.
- **Une seule hitbox par combattant**, créée au spawn.
- **Un seul joueur** : `ai_behavior` utilise `Query::single` ; avec plusieurs
  entités `Player`, l'IA ne fait rien.
- **Pas de réseau**, tout est local.

## Installation

```toml
[dependencies]
ai-duel = "0.2"
bevy = "0.20"
```

## Démarrage rapide

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

    // le joueur, à gauche
    spawn_fighter(&mut commands, &assets, &config,
        Vec3::new(-250.0, 0.0, 0.0), Faction::Player, None);

    // un ennemi piloté par l'IA, à droite
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

Les spritesheets sont attendues sous forme d'une seule ligne de frames (course
et attaque dans deux images séparées). Adaptez `TextureAtlasLayout` pour un
autre format.

> **Portée de l'IA.** Avec la config par défaut, une attaque touche jusqu'à
> environ 100 unités (décalage 45 + demi-hitbox 35 + demi-hurtbox 20).
> Gardez `aggression_range` en dessous, sinon l'IA frappe dans le vide.

## Configuration

Tous les réglages sont dans `AiDuelConfig`. Le plugin insère une config par
défaut ; remplacez-la après avoir ajouté le plugin :

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

| Champ                                 | Défaut        | Rôle                                         |
|---------------------------------------|---------------|----------------------------------------------|
| `attack_damage`                       | `10.0`        | Dégâts par coup                              |
| `attack_cooldown`                     | `0.60`        | Délai minimal entre deux attaques (s)        |
| `hit_cooldown`                        | `0.40`        | Invulnérabilité après un coup (s)            |
| `hitbox_active_from` / `_to`          | `2` / `4`     | Frames où la hitbox est active               |
| `hitbox_half_size`                    | `(35, 30)`    | Demi-taille de la hitbox                     |
| `hitbox_offset`                       | `(45, 0)`     | Décalage devant le combattant                |
| `hurtbox_half_size`                   | `(20, 35)`    | Demi-taille de la zone vulnérable            |
| `debug_hitboxes`                      | `false`       | Affiche les hitbox                           |
| `default_max_health`                  | `100.0`       | PV de départ                                 |
| `sprite_scale`                        | `1.5`         | Échelle des sprites                          |

Les distances sont en unités monde. Voir la documentation de `AiDuelConfig`
pour la liste complète.

## Contrôler le joueur

Le plugin ne touche pas au `Transform` du joueur. Planifiez votre système
`.before(AiDuelSet)` pour que l'attaque soit prise en compte dès la même frame :

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

    // pas de déplacement pendant l'attaque
    if state.0 == FighterState::Attacking {
        wants.0 = false;
        return;
    }

    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowRight) { dir.x += 1.0; }
    if keys.pressed(KeyCode::ArrowLeft)  { dir.x -= 1.0; }

    wants.0 = dir != Vec2::ZERO;
    if dir.x != 0.0 {
        // retourne le sprite (et donc la hitbox) selon la direction
        tf.scale.x = config.sprite_scale * dir.x.signum();
    }
    tf.translation += (dir.normalize_or_zero() * 200.0 * time.delta_secs()).extend(0.0);
}

// dans main() :
// .add_systems(Update, player_input.before(AiDuelSet))
```

## Débogage

Activez `debug_hitboxes: true` dans `AiDuelConfig` pour afficher les hitbox en
rouge translucide. Les touches et les morts sont tracées au niveau `debug`
(`RUST_LOG=ai_duel=debug`).

## Architecture

```
src/
├── lib.rs          plugin, AiDuelSet, spawn_fighter, start_attack, prelude
├── components.rs   AiDuelConfig, FighterAssets, composants (Health, Hitbox...)
└── systems.rs      systèmes du plugin (IA, animation, collisions, mort)
```

L'ordre d'exécution est : cooldowns → IA → animation → hitbox → collisions →
santé/flash → mort.

## Choix de conception

- **États figés (`Running` / `Attacking`)** plutôt qu'une machine à états
  générique : un duel n'a besoin que de deux états.
- **Hitbox en entité enfant** : le `GlobalTransform` hérité du parent suit la
  position et le retournement du combattant sans calcul manuel.
- **Fenêtre de frames actives** plutôt qu'une hitbox active pendant toute
  l'attaque : le coup ne fait pas mal avant que le bras ait bougé.
- **Hitbox + hurtbox (AABB)** plutôt qu'un test point-dans-boîte : la portée
  réelle d'une attaque dépend de la taille des deux zones.
- **Systèmes chaînés (`.chain()`)** : chaque étape dépend du résultat de la
  précédente ; les paralléliser créerait des incohérences d'une frame à l'autre.
- **Systèmes publics** : ils peuvent être remplacés ou ré-ordonnés depuis
  l'application hôte sans forker la crate.

## Changelog

Voir [CHANGELOG.md](CHANGELOG.md).

## Licence

GPL-2.0-or-later, voir le champ `license` de `Cargo.toml`.