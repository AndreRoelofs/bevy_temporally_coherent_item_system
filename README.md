# bevy_temporally_coherent_item_system

## 19 July 2026 State

The player now has a bag next to the weapon in hand: walking over a weapon
puts it in the bag, `Q` takes one out to equip it (the current weapon goes
back in the bag), and `G` drops the equipped one. If whoever is carrying
items dies, everything they had falls to the ground where they stood.
Internally, an item's whereabouts is no longer one big value but three
simple parts - what state it is in, who has it, and where it is - and all
changes go through one small set of functions, which keeps the parts from
ever disagreeing.

## 09 July 2026 State

The game implements the basic ideas described in the problem/solution
statement using the model/view split. A `Rusty` component on the model
drives the weapon's color, and `GroundedSecs` accumulates while a weapon
lies on the ground, inserting `Rusty` once it has been on the floor for too long.

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

## Solution: model/view split over three decomposed axes

Each item is **one persistent model entity** that is spawned once and never
rebuilt. Everything durable lives on it as plain components; because no code
ever strips or regenerates the model, arbitrary accumulated components
survive every transition *by construction* - there is no whitelist to
maintain and no copying step to forget.

The model's location is decomposed onto three axes, each with the mutation
semantics its quantity actually has:

| Axis | Component | Semantics |
|---|---|---|
| kind | `ItemState` (`OnGround \| Equipped \| Stored`) | immutable; re-insert *is* the transition, firing `On<Insert, ItemState>` exactly once |
| reference | `ContainedBy(entity)` → `Contains` | a real relationship: O(1) reverse queries, automatic cleanup |
| position | `Transform` | freely mutable - moving a grounded item is plain mutation, no view rebuild |

Both writable axes are **sealed**: `ItemState` and `ContainedBy` can be read
anywhere but constructed only inside `src/item/`, so the `ItemTransitions`
trait (`equip_to` / `store_in` / `drop_at`) is the only door. The
load-bearing orderings live in that one module, policy included: equipping a
new weapon automatically stows the old one, and an item first enters the
world by being *dropped* into it. A dev-build watchdog reports any axis
contradiction in-module code could still create.

What changes with state is the **view**: a separate, disposable entity built
from a `bsn!` scene and linked to its model with a one-to-one relationship
(`ViewOf`/`View`, `linked_spawn`). The view is a pure function of the model:

```text
view = f(model)
```

- A grounded view is a child of its model, placed by the model's `Transform`
  through ordinary propagation; an equipped view is a child of the holder; a
  stored item has no view at all.
- Scene functions receive the model as an `EntityRef`, so a view can react
  to *any* model component: the gun's material turns rust-brown when the
  model has `Rusty`.
- When an entity holding items despawns, everything it carried - equipped
  and stowed alike - drops at its death position: `Despawn` observers run
  before the dying entity's components are stripped, so its `Transform` and
  `Contains` list are still readable. A second observer repairs links lost
  outside the sanctioned paths by re-grounding the item where it last lay.

Because the reference axis is one relationship and the *how* lives on the
kind axis, a character can equip a sword and carry a gun in the bag at the
same time - both are `ContainedBy(player)`, and stow/draw between them is a
pure state flip that never touches the relationship.

The demo makes the guarantees visible: leave a gun on the ground a few
seconds and `Rusty` appears on the model (it browns). The HUD shows each
model keeping its entity id and components across every transition, each
view's entity id changing per transition, and the player's inventory read
straight off `Contains`.

## Controls

- Click to capture the mouse, `Esc` to release
- `WASD` + mouse to move and look
- Walk over a grounded item to stow it in the bag
- `Q` - equip the first stowed item (the current weapon slides into the bag)
- `G` - drop the equipped item just beyond pickup range

## Points of improvement

- The scenes in `src/item/scenes.rs` are still inlined `bsn!` blocks; once
  the official `.bsn` file loader lands, the `ItemRegistry` can map keys to
  scene assets instead of functions and item appearance becomes fully
  data-driven.
- The state ladder, for future scale: the sealed enum + relationship shape
  implemented here is the second rung. If per-state iteration ever gets hot
  (many items, per-frame systems that only touch grounded ones), the next
  rung is marker *index* components (`OnGround`/`Equipped`/`Stored`)
  maintained by the transition API alongside the enum - archetype-level
  filtering without giving up exhaustive matching. Per-state components as
  the *truth* is a rung that should never be climbed: it trades away
  exhaustiveness and one-state-representation for nothing the index doesn't
  already provide.
- `Stored` currently renders nothing regardless of container. Differentiating
  a holstered weapon on a player from a gun in a chest would grow the view
  function from `f(model)` to `f(model, container)` - worth doing only when
  a real need appears.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
