# bevy_temporally_coherent_item_system

# The following explanation is still WIP

## Purpose
This project is loosely inspired by the way Unreal Engine Lyra project handles the items and mutates their state between being on the ground, in inventory or equipped by the player and how a similar system of fragments can be implemented better in Bevy ECS. Below is a breakdown of how this project works and motivations for the design decisions that I made.

## States of an item

Every item exists in one of the following 3 states

```rust
// Marks items lying on the ground
#[derive(Component)]
pub struct OnGround;

// Marks items equipped by a player in hand
#[derive(Component)]
pub struct EquippedBy(pub Entity);

// Marks items stored in a container or player's inventory
#[derive(Component)]
pub struct StoredIn(pub Entity);
```

An item can only have one of these states. The state decides how the item is displayed to the player. Let's say you want to equip an item to the player. You can do this by simply adding `EquippedBy` with a link to the player's entity, which then gets processed by the following observer

```rust
fn force_item_state_invariants<S: ItemStateMarker>(
    insert: On<Insert, S>,
    markers: Res<ItemStateMarkers>,
    models: Query<EntityRef>,
    mut commands: Commands,
);
```

`force_item_state_invariants` removes every other item state that existed on the item before you decided to insert the new `EquippedBy` component. This includes any previous instances of `OnGround`, `EquippedBy` or `StoredIn` as well as any third party item states that can be added by mods. The reason why this works is that every `Component` that you want to explicitly define as an item state invariant can just be registered via

```rust
#[derive(Resource)]
struct ItemStateMarkers(Vec<...>);
```


## The MVP
The purpose of this project is to create a reusable item system that can be integrated into any type of game. The game you will find here is a simple 3D First Person Shooter. The benefits of the proposed item system are expressed via the `Rusty` component that degrades a gun's performance if the gun has been laying on the ground for a total of 5 seconds.

The two most important components are

```rust
#[derive(Component, Default)]
pub struct GroundedSecs(pub f32); // stores the number of seconds an entity spent on the ground

#[derive(Component, Clone, Default)]
pub struct Rusty;
```

The application of `Rusty` is handled by a simple function you can find below:

```rust
// ... app.add_systems(Update, rust_grounded_items); ...

fn rust_grounded_items(
    time: Res<Time>,
    // Make sure we never insert Rusty twice by querying only the items that don't have Rusty
    mut items: Query<(Entity, &mut GroundedSecs), Without<Rusty>>,
    mut commands: Commands,
) {
    for (item, mut grounded) in &mut items {
        grounded.0 += time.delta_secs();
        if grounded.0 >= 5.0 {
            commands.entity(item).insert(Rusty);
        }
    }
}
```

Now that we have the basics out the way, let's see what else this architecture has to offer.



## Old Old Explanation



Now we have a problem already. An item can receive `Rusty` only if it's `OnGround`. But it's performance can only be degraded if it's `Equipped`. We need to have some kind of way to preserve the components between the items states. For the purposes of this demo a gun is displayed as a 3D rectangle when on the ground, as a 2D image when in player's inventory and a 3D sphere when in hand, mimicking potentially different meshes that an item might have in all 3 different states.

If the item is `Rusty`, it also needs to convey that information to the player visually. In all 3 states, a `Rusty` item gets a brown tint. Brand new items just have a white texture. `Entity` persistence is something that we receive for free in ECS systems. OOP systems like Unreal Engine and Unity can solve the persistence issue to some degree by using a unified representation and adding key-value pairs to that representation in order to save data between state transitions.

## Model/View split

Every item is just an instance of `Entity` at the end of the day. With the `Rusty` component being in play, we already want to split the representation of an item from how this item behaves in different states.



# Old Explanation

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

### Advanced OOP solution

The architecture used by mature games looks different but has the same idea. Every item is split into layers:

1. An immutable definition shared by every item - `UItemDefinition`
2. A shared persistent item. This is what is shared across ground, equipped and inventory states.
3. A view of the item. 3D model for when it is on the ground or equipped, 2D image when in inventory.

The item definition is composed of various fragments like so:
```cpp
class UItemDefinition : public UObject
{
public:
    FText DisplayName;
    TArray<UItemFragment*> Fragments; // This is what contains various unique aspects of an item
};

// Allows the item to be wielded by the player
class UFragment_Equippable : public UItemFragment
{
public:
    TSubclassOf<AGunWeapon> WeaponClass;
};

// Allows the item to shoot on a cooldown
class UFragment_GunStats : public UItemFragment
{
public:
    float FireCooldown;
};
```

The shared persistent item is created when the gun is first spawned - in our case on the ground. Now the item can be persisted through the pickup and equip phases - we can just store the `Rusty` component in `StatTags`.
```cpp
class UItemInstance : public UObject
{
public:
    TSubclassOf<UItemDefinition> ItemDef;

    template <typename FragmentT>
    const FragmentT* FindFragmentByClass() const;

    void  AddStatTagStack(FGameplayTag Tag, int32 Count);
    int32 GetStatTagStackCount(FGameplayTag Tag) const;

private:
    FGameplayTagStackContainer StatTags;
};
```

Picking up an item

```cpp
void AMyCharacter::PickUp(AGunPickup* Pickup)
{
    InventoryManager->AddItemInstance(Pickup->Item); // the instance moves, nothing is copied
    Pickup->Destroy();                               // only the 3D representation dies
}
```

Equipping goes through one more layer. The item never becomes an actor itself: an equipment component reads the equippable fragment, spawns the visible weapon, and links it back to the item it stands for. The instance itself never leaves the inventory:

```cpp
void AMyCharacter::Equip(UItemInstance* Item)
{
    const UFragment_Equippable* Equippable = Item->FindFragmentByClass<UFragment_Equippable>();

    // The equipment instance lives only while the item is equipped.
    // It spawns the AGunWeapon actor and attaches it to the hand.
    UEquipmentInstance* Equipment = EquipmentManager->EquipItem(Equippable);

    // The spawned weapon can find its way back to the one true item
    Equipment->SetInstigator(Item);
}
```

The rusting mechanic now touches one object no matter the state. The pickup actor accrues ground time onto the instance, and anything that cares about rust reads it from the same place:

```cpp
void AGunPickup::AccrueRust()
{
    Item->AddStatTagStack(TAG_SecondsOnGround, 1);

    if (Item->GetStatTagStackCount(TAG_SecondsOnGround) >= 5)
    {
        Item->AddStatTagStack(TAG_Rusty, 1);
    }
}
```


Let's fire the gun and double the cooldown if the gun is `Rusty`

```cpp
void AGunWeapon::Fire()
{
    UItemInstance* Item = Equipment->GetInstigator();

    const UFragment_GunStats* Stats = Item->FindFragmentByClass<UFragment_GunStats>();

    float Cooldown = Stats->FireCooldown;
    if (Item->GetStatTagStackCount(TAG_Rusty) > 0)
    {
        Cooldown *= 2.0f;
    }

    NextFireTime = GetWorld()->GetTimeSeconds() + Cooldown;
    SpawnProjectile();
}
```

## Advanced Bevy solution

In Entity Component System an item is a persistent `Entity`. What this entity does depends completely on what `Component`s are present on it.

The string-keyed tags from the advanced example above become Bevy `Component`s

```rust
#[derive(Component, Default)]
pub struct GroundedSecs(pub f32); // stores the number of seconds an entity spent on the ground

#[derive(Component, Clone, Default)]
pub struct Rusty;
```

We can keep track of their application via the following function

```rust
// ... app.add_systems(Update, rust_grounded_items); ...

fn rust_grounded_items(
    time: Res<Time>,
    // Make sure we never insert Rusty twice by querying only the items that don't have Rusty
    mut items: Query<(Entity, &mut GroundedSecs), Without<Rusty>>,
    mut commands: Commands,
) {
    for (item, mut grounded) in &mut items {
        grounded.0 += time.delta_secs();
        if grounded.0 >= 5.0 {
            commands.entity(item).insert(Rusty);
        }
    }
}
```

So far so good. We just converted the basics of the advanced example to Bevy API. Let's now replicate the process of not only firing the gun, but also increasing it's cooldown based on whether it's Rusty or not before doing so.

```rust

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
