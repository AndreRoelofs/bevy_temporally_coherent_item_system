# bevy_temporally_coherent_item_system

## Problem

Let's say you want to have a gun in your game that can lay on the ground, be equipped by the player or exist in a player's inventory. In a naive implementation of the item system in classical engines like Unreal Engine and Unity, you will model the item as 3 separate objects to represent the 3 separate states: an `AGunPickup` actor on the ground, an `AGunWeapon` actor in the hand, and a `UGunInventoryItem` object in the bag. When the object goes from the ground to the player's inventory, you would destroy the `AGunPickup` actor and create a `UGunInventoryItem`.

In order to pick up the gun from the ground you would have something like this:

```cpp
void AMyCharacter::PickUp(AGunPickup* Pickup)
{
    UGunInventoryItem* Item = NewObject<UGunInventoryItem>(Inventory);

    Item->FireCooldown = Pickup->FireCooldown; // How many seconds until we shoot again.

    Inventory->Items.Add(Item);
    Pickup->Destroy();
}
```

Notice that we are copying every single field by hand. Make sure you don't forget one or else!

Let's equip the gun now:
```cpp
void AMyCharacter::Equip(UGunInventoryItem* Item)
{
    AGunWeapon* Weapon = GetWorld()->SpawnActor<AGunWeapon>(Item->WeaponClass);

    Weapon->FireCooldown = Item->FireCooldown;

    Inventory->Items.Remove(Item);
}
```

Here is how this setup invites data loss and bugs. Let's say we want to add the rusting mechanic to our guns. The `Rusty` status effect should:

1. Add a brown tint to the 3D model in the hand
2. Add a brown tint to the 2D image in the inventory
3. Double the FireCooldown

A gun should become `Rusty` after a total of 5 seconds on the ground. Let's say the gun stays on the ground for 3 seconds before a player picks it up. If the player then drops the gun again, it should become `Rusty` in just 2 seconds. Let's model this addition:

```cpp
void AMyCharacter::PickUp(AGunPickup* Pickup)
{
    UGunInventoryItem* Item = NewObject<UGunInventoryItem>(Inventory);

    Item->FireCooldown    = Pickup->FireCooldown;

    // Two new fields
    Item->SecondsOnGround = Pickup->SecondsOnGround;
    Item->bIsRusty        = Pickup->bIsRusty;

    Inventory->Items.Add(Item);
    Pickup->Destroy();
}

void AMyCharacter::Equip(UGunInventoryItem* Item)
{
    AGunWeapon* Weapon = GetWorld()->SpawnActor<AGunWeapon>(Item->WeaponClass);

    Weapon->FireCooldown    = Item->FireCooldown;

    // For a total of four extra lines of code
    Weapon->SecondsOnGround = Item->SecondsOnGround;
    Weapon->bIsRusty        = Item->bIsRusty;

    Inventory->Items.Remove(Item);
}
```

With the gun equipped the player wants to fire it. Let's handle the cooldown and `IsRusty` field:

```cpp
void AGunWeapon::Fire()
{
    float Cooldown = FireCooldown;
    if (bIsRusty)
    {
        Cooldown *= 2.0f;
    }

    SpawnProjectile();
}
```

## OLD EXPLANATION

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
persists them through state transitions. The view of the item is what
changes between the transitions. In this game the gun is displayed as a
rectangle when on the ground, as a sphere when the player equips it, and
as a white 2D image on the backpack grid when it is stored. All three go
through the same registry: an item key maps each state to a chrome
scene, and a single observer rebuilds the view whenever the state
changes. A state without chrome — or a `Stored` item whose container has
no inventory panel — simply has no view.

The inventory is spatial, survival-game style. The container carries an
`InventoryGrid` (the player's backpack is 12×8 cells), each item carries
an `ItemFootprint` (the pistol is 4×4, the rifle 8×4), and the occupied
spot lives on the model as `PackedAt`. Because `PackedAt` accumulates on
the model like everything else, an item that is dropped and picked back
up returns to its remembered spot if it is still free. Occupancy is
never cached; it is always derived from the `Contains` relationship plus
these components. Packing runs as a world command so two items stowed in
the same frame cannot race for the same cells, and a bag without room
re-grounds the item at the holder — the repair-net philosophy again:
never leave a stored item without a valid spot. Dragging an icon across
the grid ("packing the backpack") commits through the same validation
and snaps back when the drop is invalid.

The accumulated components interact with these views as well as other
components. If a gun lies on the ground for too long (5 seconds), it
receives the `Rusty` component, which doubles the shot cooldown through
the stat fold. Cosmetically, `Rusty` never learns how views are drawn:
it inserts a `ViewTint` on whatever view currently exists, and
per-medium appliers translate that into a material swap for the 3D
meshes or a background-color swap for the 2D icon. The same rust brown
follows the gun from the ground to the hand to the backpack, and
removing the rust restores the original look in every medium.

## Controls

- Click to capture the mouse, `Esc` to release
- `WASD` + mouse to move and look
- Walk over a grounded item to stow it in the bag (if its footprint fits)
- `Tab` - open/close the backpack; drag the white item images to repack it
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
