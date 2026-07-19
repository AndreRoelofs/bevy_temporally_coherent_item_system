# bevy_temporally_coherent_item_system

## 09 July 2026 State

The game implements basic ideas described in the problem/solution statement. The game has Rusty component to define the color of the weapon and a way to add the Rusty component if the weapon was laying on the ground too long.

## Problem

In many games, an item is a different *object* in each of its states: the gun
lying on the ground, the gun in the player's hand, and the gun in a chest are
separate spawns that merely share a mesh. Transitions then work by despawning
one object and spawning another, and anything that accumulated on the old
object - wear, enchantments, ownership, history - is lost unless it is
manually copied across. (This is a data-modeling choice, not something any
particular paradigm forces; engines like Unity and Unreal can keep one object
across states too. The interesting question is what architecture makes the
persistent-item guarantee *structural* instead of a per-field copying
discipline.)

## Solution: model/view split

Each item is **one persistent model entity** that is spawned once and never
rebuilt. Everything durable lives on it as plain components: `Item` (key,
label), `ItemState`, `Gun`, `GroundedSecs`, `Rusty`, and whatever else other
systems decide to attach. Because no code ever strips or regenerates the
model, arbitrary accumulated components survive every transition *by
construction* - there is no whitelist to maintain and no copying step to
forget.

What changes with state is the **view**: a separate, disposable entity
holding the renderable components, built from a `bsn!` scene and linked to
its model with a relationship pair (`ViewOf` on the view, `View` on the
model, with `linked_spawn` so the view dies with the model).

The view is a pure function of the model:

```text
view = f(model)
```

- `ItemState` is an **immutable component**: the only way to transition is to
  re-insert it, which fires `On<Insert, ItemState>` exactly once per
  transition - no per-frame polling.
- The observer despawns the old view, asks the `ItemRegistry` (string key →
  scene function) for a new scene, spawns it with `Commands::spawn_scene`,
  and parents it to the holder when equipped. No exclusive systems anywhere.
- Scene functions receive the model as an `EntityRef`, so a view can react to
  *any* model component: the gun's material turns rust-brown when the model
  has `Rusty`.

The demo makes the guarantee visible: leave the gun on the ground for a few
seconds and `Rusty` appears on the model (the gun browns). Pick it up (`E`),
drop it (`G`) - the HUD shows the model keeping the same entity id and all
its components across every transition, while the view line shows a fresh
entity id each time.

## Controls

- Click to capture the mouse, `Esc` to release
- `WASD` + mouse to move and look
- Walk over a grounded item to pick it up
- `G` - drop the equipped item (it lands just beyond pickup range)

## Points of improvement

- The scenes in `src/item/scenes.rs` are still inlined `bsn!` blocks; once
  the official `.bsn` file loader lands, the `ItemRegistry` can map keys to
  scene assets instead of functions and item appearance becomes fully
  data-driven.
- `ItemState` is deliberately a single enum: exactly one state exists at any
  instant by construction, and a transition is one `insert`. The cost is that
  `EquippedBy`/`StoredIn` hold raw `Entity` ids, so reverse queries ("what is
  this player holding?") are O(items) scans, and a dead holder leaves a
  dangling id — covered here by the `ItemHolder` despawn guard, which
  re-grounds stranded items through the ordinary transition path. If reverse
  queries become hot, the scaling shape is to decompose the axes: keep a slim
  location enum (`OnGround(Vec3) | Equipped | Stored`) as the view trigger
  and move the entity reference into a real relationship pair (`ContainedBy`
  / `Contains`) for O(1) reverse queries and automatic cleanup — trading away
  the enum's by-construction exclusivity for it. Splitting each state into
  its own relationship component is *not* recommended: sibling-state cleanup
  is deferred through commands, so observers can see two states at once
  mid-transition.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
