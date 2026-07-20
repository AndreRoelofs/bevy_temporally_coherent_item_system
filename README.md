# bevy_temporally_coherent_item_system

## Problem

In many games, an item is a different object in each of its states: the gun
lying on the ground, the gun in the player's hand, and the gun in a chest are
separate spawns that merely share a mesh. Transitions then work by despawning
one object and spawning another, and anything that accumulated on the old
object - wear, enchantments, ownership, history - is lost unless it is
manually copied across.

While games written in traditional engines can and do preserve the accumulated components
between the item states, the more interesting question is what architecture
makes the persistence guarantee structural instead of a per-field copying
discipline.

## Solution: model/view split over three decomposed axes

Each item is one persistent model entity that is spawned once and kept track off through
different states (Equipped, OnGround, Stored). During normal gameplay, arbitrary
components are accumulated on the entity. When the item transitions to a different state,
the accumulated components are preserved. They can be removed only by separate systems.
This setup is especially useful for enabling rich mod support as third party code
can easily add completely new mechanics to the game that affect existing systems and items.

The state transitions do require their own components. That is where the view in the model/view
architecture comes in. More on this below.

## Architecture

An item consists of two distinct parts: the base entity and the view.
The base entity is the one that accumulates various components and
persists them through state transition. The view of the item is what
changes between the transitions. For example, in this game the gun is
displayed as a rectangle when on the ground and as a sphere when
the player equips it. When the player has it stored in their inventory
\- there is no display. This allows you full control over the representation
of the gun. If you wanted to have an inventory system, you could display
the gun as a 2D image in your inventory.

The accumulated components interact with these views as well as other
components. For example, if a gun was out on the ground for too long,
it will receive the `Rusty` component which decreases the rate of
fire. The gun will receive the `Rusty` component if it had been left
out on the ground for too long (5 seconds). The `Rusty` component knows
what the state of the item is and applies effects accordingly. The
cooldown calculation is affected when the gun is shot.

Now that I am reading it the explanation is quite abstract without the 2D
inventory system. Hmm. I will add it and then finish the architecture
explanation.

## Controls

- Click to capture the mouse, `Esc` to release
- `WASD` + mouse to move and look
- Walk over a grounded item to stow it in the bag
- `Q` - equip the first stowed item (the current weapon slides into the bag)
- `G` - drop the equipped item just beyond pickup range

## Points of improvement

- The scenes in `src/item/views/gun.rs` are still inlined `bsn!` blocks; once
  the official `.bsn` file loader lands, the `ItemRegistry` can map keys to
  scene assets loaded from files instead of patches built in code and item
  appearance becomes fully data-driven.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
