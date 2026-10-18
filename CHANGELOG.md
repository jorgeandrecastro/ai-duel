# Changelog

Toutes les modifications notables de ce projet sont documentées ici.

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et le
projet respecte le [versionnage sémantique](https://semver.org/lang/fr/).

## [0.2.0] - 2026-10-18

### Ajouté

- `AiDuelSet` : ensemble de systèmes pour ordonner ses propres systèmes par
  rapport au plugin (ex. `player_input.before(AiDuelSet)`).
- `prelude` : `use ai_duel::prelude::*;` importe l'essentiel.
- Composant `Hurtbox` : zone vulnérable des combattants, désormais testée
  contre la hitbox en AABB contre AABB.
- Champs de `AiDuelConfig` : `hitbox_offset`, `hurtbox_half_size` et
  `debug_hitboxes`.
- `Health::is_dead()`, `Health::fraction()` et `Health::try_hit()`.
- `AiController` implémente `Default`.
- `systems::aabb_overlap()`, fonction pure de test de recouvrement.
- Tests unitaires pour `Health` et les collisions AABB.
- `#![forbid(unsafe_code)]` et `#![warn(missing_docs)]`.

### Modifié

- **Code découpé en trois fichiers** : `lib.rs` (plugin et API publique),
  `components.rs` (config, assets, composants) et `systems.rs` (systèmes).
  Les types restent accessibles à la racine de la crate (`ai_duel::Health`...).
- La hitbox est décalée devant le combattant (`hitbox_offset`) et suit le
  retournement du sprite, au lieu d'être centrée sur lui.
- L'IA se tourne vers sa cible avant d'attaquer, et plus seulement quand
  elle se déplace.
- Les logs « touché » et « combattant tombé » passent de `info!` à `debug!`.
- Documentation relue et corrigée (fautes, formulations), README restructuré
  avec sommaire, tableau de configuration et section architecture.
- Valeurs d'exemple : `aggression_range` passe de 110 à 90 pour rester
  dans la portée réelle d'une attaque.

### Corrigé

- `hit_cooldown` était codé en dur à `0.4` dans `resolve_hits` ; la valeur de
  `AiDuelConfig::hit_cooldown` est maintenant respectée.
- L'IA attaquait à `aggression_range` (110) alors que la hitbox ne portait
  qu'à 35 unités : les coups partaient souvent dans le vide.
- Risque de division / modulo par zéro si `run_frames` ou `attack_frames`
  valait 0.
- Le sprite de debug de la hitbox est désormais créé uniquement si
  `debug_hitboxes` est activé, au lieu d'un sprite invisible permanent.

### Compatibilité

- Changement de comportement : la portée des attaques augmente (voir
  `hitbox_offset` et `hurtbox_half_size`). Pour retrouver l'ancien
  comportement, mettre `hitbox_offset` à `Vec2::ZERO` et
  `hurtbox_half_size` à `Vec2::ZERO`.
- `spawn_fighter` ajoute maintenant un composant `Hurtbox` ; tout code qui
  construit ses combattants à la main doit l'ajouter pour être touchable.

## [0.1.0]

### Ajouté

- Version initiale : combattants avec santé et factions, animations de course
  et d'attaque, hitbox active sur une fenêtre de frames, IA basique,
  invulnérabilité avec flash rouge, mort par despawn.

[0.2.0]: https://crates.io/crates/ai-duel/0.2.0
[0.1.0]: https://crates.io/crates/ai-duel/0.1.0