use bevy::prelude::*;

use crate::{Contains, CooldownModifiers, CursorLocked, CursorSystems, ItemState, Player};

/// Shorthand that will need to live in a more general place in the future
type EquippedGuns<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ItemState,
        &'static Firearm,
        Option<&'static CooldownModifiers>,
        &'static mut Ammo,
        Option<&'static LastShotAt>,
    ),
>;

#[derive(Component, Clone)]
pub struct Firearm {
    pub base_cooldown_secs: f32,
    pub magazine_size: u32,
}

impl Firearm {
    /// The one home of the cooldown fold: base through the staged modifier
    /// fold, clamped — "no negative cooldowns" is this stat owner's policy.
    pub fn cooldown_secs(&self, modifiers: Option<&CooldownModifiers>) -> f32 {
        modifiers
            .map_or(self.base_cooldown_secs, |modifiers| {
                modifiers.apply_to(self.base_cooldown_secs)
            })
            .max(0.0)
    }
}

#[derive(Component, Clone)]
pub struct Ammo(pub u32);

#[derive(Component, Clone)]
pub struct LastShotAt(pub f32);

#[derive(Message)]
pub struct ShotFired {
    pub gun: Entity,
}

pub struct FirearmPlugin;

impl Plugin for FirearmPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ShotFired>()
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
    cooldown_secs: f32,
    ammo: &Ammo,
    last_shot: Option<&LastShotAt>,
) -> FireOutcome {
    if ammo.0 == 0 {
        return FireOutcome::Empty;
    }
    let cooling = last_shot.is_some_and(|last| now_secs - last.0 < cooldown_secs);
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
        let Ok((gun, state, firearm, modifiers, mut ammo, last_shot)) = guns.get_mut(held) else {
            continue;
        };
        if !state.is_equipped() {
            continue;
        }
        let cooldown_secs = firearm.cooldown_secs(modifiers);
        if try_fire(time.elapsed_secs(), cooldown_secs, &ammo, last_shot) == FireOutcome::Fired {
            ammo.0 -= 1;
            commands.entity(gun).insert(LastShotAt(time.elapsed_secs()));
            shots.write(ShotFired { gun });
        }
        return;
    }
}
