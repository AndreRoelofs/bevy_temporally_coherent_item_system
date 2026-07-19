use bevy::prelude::*;

use crate::{
    Contains, CursorLocked, CursorSystems, EffectiveStats, ItemState, Player, StatModifierRegistry,
    StatsDirty,
};

/// Shorthand that will need to live in a more general place in the future
type EquippedGuns<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ItemState,
        &'static EffectiveStats,
        &'static mut Ammo,
        Option<&'static LastShotAt>,
    ),
    With<Firearm>,
>;

#[derive(Component, Clone, Default)]
pub struct Gun;

#[derive(Component, Clone)]
pub struct Firearm {
    pub base_cooldown_secs: f32,
    pub magazine_size: u32,
}

#[derive(Component, Clone)]
pub struct Ammo(pub u32);

#[derive(Component, Clone)]
pub struct LastShotAt(pub f32);

#[derive(Message)]
pub struct ShotFired {
    pub gun: Entity,
}

pub struct GunPlugin;

impl Plugin for GunPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShotFired>()
            .add_observer(stats_on_firearm)
            .add_observer(stats_on_dirty)
            .add_systems(Update, fire_equipped.before(CursorSystems));
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FireOutcome {
    Fired,
    Cooldown,
    Empty,
}

pub fn try_fire(
    now_secs: f32,
    stats: &EffectiveStats,
    ammo: &Ammo,
    last_shot: Option<&LastShotAt>,
) -> FireOutcome {
    if ammo.0 == 0 {
        return FireOutcome::Empty;
    }
    let cooling = last_shot.is_some_and(|last| now_secs - last.0 < stats.cooldown_secs);
    if cooling {
        return FireOutcome::Cooldown;
    }
    FireOutcome::Fired
}

fn fire_equipped(
    mouse: Res<ButtonInput<MouseButton>>,
    locked: Option<Res<CursorLocked>>,
    time: Res<Time>,
    player: Query<&Contains, With<Player>>,
    mut guns: EquippedGuns,
    mut shots: MessageWriter<ShotFired>,
    mut commands: Commands,
) {
    if !locked.is_some_and(|locked| locked.0) || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(contains) = player.single() else {
        return;
    };

    for held in contains.iter() {
        let Ok((gun, state, stats, mut ammo, last_shot)) = guns.get_mut(held) else {
            continue;
        };
        if !state.is_equipped() {
            continue;
        }
        if try_fire(time.elapsed_secs(), stats, &ammo, last_shot) == FireOutcome::Fired {
            ammo.0 -= 1;
            commands.entity(gun).insert(LastShotAt(time.elapsed_secs()));
            shots.write(ShotFired { gun });
        }
        return;
    }
}

/// A firearm's stats exist from the moment the fact does.
fn stats_on_firearm(
    insert: On<Insert, Firearm>,
    models: Query<EntityRef>,
    registry: Res<StatModifierRegistry>,
    commands: Commands,
) {
    recompute_stats(insert.event().entity, &models, &registry, commands);
}

/// A modifier component changed somewhere on this model; re-fold.
fn stats_on_dirty(
    insert: On<Insert, StatsDirty>,
    models: Query<EntityRef>,
    registry: Res<StatModifierRegistry>,
    commands: Commands,
) {
    recompute_stats(insert.event().entity, &models, &registry, commands);
}

fn recompute_stats(
    entity: Entity,
    models: &Query<EntityRef>,
    registry: &StatModifierRegistry,
    mut commands: Commands,
) {
    let Ok(model) = models.get(entity) else {
        return;
    };
    let Ok(mut entity_commands) = commands.get_entity(entity) else {
        return;
    };
    let Some(firearm) = model.get::<Firearm>() else {
        // Dirty on something without base stats: nothing to fold yet.
        entity_commands.remove::<StatsDirty>();
        return;
    };
    let fold = registry.fold(model);
    entity_commands
        .try_insert(EffectiveStats {
            cooldown_secs: firearm.base_cooldown_secs * fold.cooldown_mult,
        })
        .remove::<StatsDirty>();
}
