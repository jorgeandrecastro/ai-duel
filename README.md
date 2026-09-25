# ai-duel

[![Crates.io](https://img.shields.io/crates/v/ai-duel.svg)](https://crates.io/crates/ai-duel)
[![docs.rs](https://img.shields.io/docsrs/ai-duel)](https://docs.rs/ai-duel)
[![License: GPL v2+](https://img.shields.io/badge/license-GPL--2.0--or--later-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.html)
[![Bevy 0.19](https://img.shields.io/badge/bevy-0.19-orange.svg)](https://bevyengine.org)

Petit plugin [Bevy](https://bevyengine.org) pour monter rapidement un duel
joueur contre IA : santé, factions, hitbox d'attaque, et une IA basique qui
fonce sur le joueur et tape quand elle est à portée.

Fait à la base pour un prototype perso, mis en crate séparée parce qu'elle
est réutilisée d'un projet à l'autre.

## Ce que ça fait

- Un combattant a une vie, une faction (`Player` / `Enemy`), une anim de
  course et une anim d'attaque.
  
- L'IA (`AiController`) avance vers le joueur, s'arrête à une distance
  donnée, et déclenche une attaque quand elle est assez proche.

- L'attaque active une hitbox pendant quelques frames de l'anim (réglable),
  qui inflige des dégâts au premier combattant de faction adverse qu'elle
  touche.

- Petite invulnérabilité + flash rouge après chaque coup encaissé, pour
  éviter de perdre toute sa vie en une seule frame de contact.

- Le combattant à 0 PV est despawn.

## Ce que ça ne fait pas / limites

- **Pas de chargement d'assets** : la ressource `FighterAssets` doit être
  construite par l'utilisateur avec ses propres spritesheets.

- **Pas d'input clavier** : il faut brancher un système qui appelle
  `start_attack()` (ou qui bouge le `Transform` du joueur) voir plus bas.

- **Pas d'anim d'idle séparée**, pas d'anim de mort, pas de pathfinding 
  l'IA va en ligne droite vers le joueur, rien de plus.

- **Collisions en O(n)** : chaque hitbox active est comparée à tous les
  combattants adverses, sans structure d'accélération. Largement
  suffisant pour un duel 1v1 ou quelques ennemis, pas pensé pour une horde.

- **Une seule hitbox par combattant**, créée à la construction et jamais
  redimensionnée dynamiquement (pas d'attaques avec des portées
  différentes sur un même personnage, par exemple).

- **Un seul joueur supporté** : `ai_behavior` prend le premier (et seul)
  combattant marqué `Player` via `Query::single`. Avec plusieurs joueurs
  sur la carte, le système paniquera ou ignorera la requête selon la
  version de Bevy.

- **Pas de réseau / multijoueur**, tout est local et synchrone.

## Choix de conception

Quelques décisions volontaires, pour éviter les mauvaises surprises :

- **États figés (`Running` / `Attacking`) plutôt qu'une machine à états
  générique.** Un duel simple n'a besoin que de deux états ; ajouter un
  système de state machine générique aurait complexifié le code pour un
  gain nul à cette échelle.

- **Hitbox en entité enfant plutôt qu'un champ sur le combattant.**
  Profiter du `GlobalTransform` hérité du parent évite de recalculer la
  position de la hitbox à la main à chaque frame.

- Fenêtre de frames actives (`hitbox_active_from` / `_to`) plutôt qu'une
  hitbox active sur toute la durée de l'attaque. Sans ça, un coup ferait
  mal dès la toute première frame de l'anim, avant même que le sprite ait
  commencé à bouger le bras.

- **Systèmes chaînés (`.chain()`) plutôt qu'en parallèle.** L'ordre
  IA → anim → hitbox → collisions → dégâts → mort dépend directement du
  résultat de l'étape précédente ; les paralléliser introduirait des
  incohérences d'une frame à l'autre.

- **Tout est public dans `systems`.** Cela permet de réordonnancer ou de
  remplacer un système individuel depuis l'application hôte sans devoir
  forker la crate.

## Installation

```toml
[dependencies]
ai-duel = "0.1"
bevy = "0.19"
```

## Utilisation

```rust
use bevy::prelude::*;
use ai_duel::{
    AiDuelPlugin, AiDuelConfig, FighterAssets, Faction, AiController,
    spawn_fighter,
};

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
            aggression_range: 110.0,
            stop_range: 55.0,
            speed: 180.0,
        }));

    commands.insert_resource(assets);
}
```

Les spritesheets sont attendues sous forme d'une seule ligne de frames
(course et attaque dans deux images séparées). Il suffit d'adapter
`TextureAtlasLayout` si le format utilisé est différent.

## Configurer le comportement

Tous les réglages (dégâts, cooldowns, vitesse d'anim, taille de la
hitbox...) sont dans `AiDuelConfig`. Le plugin insère une config par
défaut ; il suffit de la remplacer après l'avoir ajoutée pour changer un
réglage :

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

## Contrôler le joueur

Le plugin ne touche pas au `Transform` d'une entité marquée `Player` en
dehors des collisions. Un système minimal pour le déplacement et
l'attaque :

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
        ai_duel::start_attack(&mut state, &mut anim, &mut sprite, &assets, &config);
        return;
    }

    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowRight) { dir.x += 1.0; }
    if keys.pressed(KeyCode::ArrowLeft)  { dir.x -= 1.0; }

    wants.0 = dir != Vec2::ZERO;
    tf.translation += (dir.normalize_or_zero() * 200.0 * time.delta_secs()).extend(0.0);
}
```

À ajouter dans `Update`, avant les systèmes du plugin si l'attaque du
joueur doit être prise en compte dès la même frame.

## Licence

GPL-2.0-or-later, voir le fichier `Cargo.toml`.

