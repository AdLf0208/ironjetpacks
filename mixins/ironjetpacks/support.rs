// Game-side glue for Iron Jetpacks slice 2: the item-backed energy tank, the
// flight tick, the engine toggle and the guest input flags.
//
// Injected into `crate::game` as `ironjetpacks_state`; it needs the game, so
// it is not part of the standalone library (only `runtime.rs` is).
//
// The pin's `player_tick` runs at the end of `ServerPlayer.tick`, after the
// server's travel. A velocity written there is sent to the player's own
// client as `SetEntityMotion` on the next tick (`health.sync_velocity`), which
// is what drives the local player: the server copy follows the client's moves
// and cannot move it by itself (LAN guests have no server-side player write at
// this pin, recorded in `notes.md`). `fall_distance` is zeroed on the same
// ticks, upstream's fall protection (`PlayerTickHandler.java:90-96`).
//
// Slice 2 moves the tank from a per-player map onto the **item**: the jetpack
// declares an energy buffer (`native_item_energy_buffer`, API 307) and the
// energy lives in the stack's `minecraft:custom_data`, so it is saved and sent
// with the item. The engine switch is the mod's own item component
// (`native_component_types`, API 65), upstream's `JETPACK_ENGINE`.

use std::collections::BTreeMap;
use std::sync::Mutex;

use horizon_client::items::ItemStack;
use horizon_client::mod_api::ItemEnergyBuffer;
use horizon_client::text::Text;
use horizon_core::geometry::Vec3;
use horizon_world::nbt::Tag;

use crate::game::ironjetpacks::{Action, Flags, IRON, Input, State, input_from_flags, parse, step};
use crate::game::native_server::{NativeCommand, NativeServerPlayer};

/// The single 26.1 jetpack item; the slice fixes its tier to iron.
const ITEM: &str = "ironjetpacks:jetpack";
/// `Player.armor` is head, chest, legs, feet.
const CHEST: usize = 1;
/// The mod's own item component: the engine switch (`JETPACK_ENGINE`).
const ENGINE: &str = "ironjetpacks:engine";

#[derive(Default)]
struct Store {
    /// Space held in the window's own input, by the window's player name.
    jump_held: BTreeMap<String, bool>,
    /// The last `PlayerInput` flags a guest sent (`native_player_input`).
    guest_input: BTreeMap<String, Flags>,
}

static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn store() -> std::sync::MutexGuard<'static, Option<Store>> {
    STORE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// The window's tick: sample the local player's Space key (the vanilla jump
/// mapping). `native_tick` runs in **every** `Game` — the window's and the
/// single-player server thread's (`native_hooks.rs:2311-2321`) — and the
/// server's has no window keys, so only the window may write this; otherwise
/// the server's empty `keys` clears the sample on the same tick. The value is
/// read in `on_player_tick`.
pub(super) fn on_client_tick(game: &mut crate::game::Game<'_>) {
    if !game.native_window() {
        return;
    }
    let space = game.keys.raw(&winit::keyboard::KeyCode::Space);
    let name = game.options.player_name();
    let mut guard = store();
    let store = guard.get_or_insert_with(Store::default);
    store.jump_held.insert(name, space);
}

/// `native_player_input` (API 343): a guest sent the flags the server applied.
/// The host sends none, so it keeps the window key.
pub(super) fn on_player_input(_player: u128, name: &str, flags: i32) {
    let mut guard = store();
    let store = guard.get_or_insert_with(Store::default);
    store.guest_input.insert(name.to_owned(), input_from_flags(flags));
}

/// `native_item_energy_buffer` (API 307): the jetpack is a battery of its
/// tier's capacity; one call may move a whole tank.
pub(super) fn on_item_energy_buffer(item: &str, buffer: &mut Option<ItemEnergyBuffer>) {
    if item == ITEM {
        *buffer = Some(ItemEnergyBuffer {
            capacity: IRON.capacity as i32,
            max_insert: IRON.capacity as i32,
            max_extract: IRON.capacity as i32,
        });
    }
}

/// `native_component_types` (API 65): before content loads, register the
/// engine switch the stacks carry.
pub(super) fn register_component_types(types: &mut Vec<String>) {
    types.push(ENGINE.to_owned());
}

/// `native_commands` (API 35): the chat box knows the mod's command, so
/// `/ironjetpacks ...` is coloured, completed and listed.
pub(super) fn on_commands(commands: &mut Vec<&'static str>) {
    commands.push("ironjetpacks");
}

/// The energy one jetpack item holds (`native_item_energy`, API 307).
fn tank(game: &crate::game::Game<'_>, stack: &ItemStack) -> i32 {
    game.native_item_energy(stack).map_or(0, |held| held.amount)
}

/// The engine switch on one jetpack item; upstream defaults it off.
fn engine_on(game: &crate::game::Game<'_>, stack: &ItemStack) -> bool {
    matches!(
        game.native_component(stack, ENGINE).ok().flatten(),
        Some(Tag::Byte(1))
    )
}

/// The same item with the engine switch set.
fn with_engine(
    game: &crate::game::Game<'_>,
    stack: &ItemStack,
    on: bool,
) -> Option<ItemStack> {
    let mut one = stack.clone();
    one.count = 1;
    game.native_set_component(&mut one, ENGINE, Tag::Byte(i8::from(on)))
        .ok()
        .map(|()| one)
}

/// The one-item result of moving `amount` into or out of the stack's tank.
fn move_energy(
    game: &crate::game::Game<'_>,
    stack: &ItemStack,
    amount: i32,
) -> Option<ItemStack> {
    if amount == 0 {
        return Some(stack.clone());
    }
    let moved = if amount > 0 {
        game.native_item_energy_insert(stack, amount)
    } else {
        game.native_item_energy_extract(stack, -amount)
    };
    moved.map(|(_, one)| one)
}

/// One player's server tick end: thrust while the iron jetpack is worn in the
/// chest slot, its engine is on and the ascend key is held.
pub(super) fn on_player_tick(game: &mut crate::game::Game<'_>) {
    let Some(player) = game.native_acting_player() else {
        return;
    };
    if player.game_mode == 3 {
        return;
    }
    let Some(stack) = game.armor[CHEST].clone() else {
        return;
    };
    if &*stack.item != ITEM {
        return;
    }
    let Some((motion_y, on_ground, in_water, sprinting)) = game.player.as_ref().map(|motion| {
        (
            motion.velocity.y,
            motion.ground.on_ground,
            motion.fluids.was_touching_water,
            motion.sprinting,
        )
    }) else {
        return;
    };
    // A guest's own flags, else the host's window key and vanilla state.
    let (up, down, sprint) = {
        let guard = store();
        match guard
            .as_ref()
            .and_then(|store| store.guest_input.get(&player.name))
            .copied()
        {
            Some(flags) => (flags.up, flags.down, flags.sprint || sprinting),
            None => {
                let space = guard
                    .as_ref()
                    .and_then(|store| store.jump_held.get(&player.name))
                    .copied()
                    .unwrap_or(false);
                (space, false, sprinting)
            }
        }
    };
    let before = State {
        engine_on: engine_on(&*game, &stack),
        hover: false,
        creative: player.game_mode == 1,
        throttle: 1.0,
        fuel: tank(&*game, &stack).max(0) as u32,
    };
    let input = Input {
        up,
        down,
        sprint,
        on_ground,
        in_water,
    };
    let stepped = step(&IRON, before, input, motion_y);
    // Upstream extracts inside the thrust branch; `spent` is that extraction.
    // `move_energy` reads the sign (negative extracts), so the amount taken
    // out of the tank goes in negative: passing it positive would insert it
    // back into a full tank and the item would never drain.
    if stepped.spent > 0
        && let Some(one) = move_energy(&*game, &stack, -(stepped.spent as i32))
    {
        game.armor[CHEST] = Some(one);
    }
    let Some(velocity_y) = stepped.velocity_y else {
        return;
    };
    if let Some(motion) = game.player.as_mut() {
        motion.velocity = Vec3::new(motion.velocity.x, velocity_y, motion.velocity.z);
        // The client is told on the next tick; it is the body that moves.
        motion.health.sync_velocity = true;
        // `PlayerTickHandler.java:91`: fall protection on every thrust tick.
        motion.fall_distance = 0.0;
    }
}

/// Declare the mod's commands. `fuel` is the slice's charge control, `engine`
/// the switch upstream keeps in the item component.
pub(super) fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op("ironjetpacks fuel", 2));
    commands.push(NativeCommand::op("ironjetpacks fuel <amount:integer>", 2));
    commands.push(NativeCommand::op("ironjetpacks engine", 2));
    commands.push(NativeCommand::op("ironjetpacks engine <on:boolean>", 2));
}

/// The worn jetpack the command acts on, or the feedback saying there is none.
fn worn(game: &crate::game::Game<'_>) -> Option<ItemStack> {
    let stack = game.armor[CHEST].clone()?;
    (&*stack.item == ITEM).then_some(stack)
}

/// `/ironjetpacks fuel|engine [...]`: charge or drain the worn jetpack's tank,
/// or flip its engine.
pub(super) fn on_perform_command(
    game: &mut crate::game::Game<'_>,
    _player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let action = parse(command);
    if action == Action::Unknown {
        return;
    }
    *ran = true;
    let Some(stack) = worn(&*game) else {
        game.native_feedback(Text::literal(
            "Iron Jetpacks: wear the jetpack in the chest slot",
        ));
        return;
    };
    match action {
        Action::Unknown => {}
        Action::Bad => game.native_feedback(Text::literal(
            "Usage: /ironjetpacks fuel [amount] | /ironjetpacks engine [on|off]",
        )),
        Action::Query => {
            let amount = tank(&*game, &stack);
            let engine = if engine_on(&*game, &stack) {
                "on"
            } else {
                "off"
            };
            game.native_feedback(Text::literal(format!(
                "Iron Jetpacks: {amount} / {} FE, engine {engine}",
                IRON.capacity
            )));
        }
        Action::Set(amount) => {
            let held = tank(&*game, &stack);
            let change = amount as i32 - held;
            let Some(one) = move_energy(&*game, &stack, change) else {
                game.native_feedback(Text::literal("Iron Jetpacks: the jetpack holds no tank"));
                return;
            };
            let after = tank(&*game, &one);
            game.armor[CHEST] = Some(one);
            game.native_feedback(Text::literal(format!(
                "Iron Jetpacks: fuel set to {after} / {}",
                IRON.capacity
            )));
        }
        Action::ToggleEngine | Action::SetEngine(_) => {
            let on = match action {
                Action::SetEngine(on) => on,
                _ => !engine_on(&*game, &stack),
            };
            let Some(one) = with_engine(&*game, &stack, on) else {
                game.native_feedback(Text::literal("Iron Jetpacks: the engine could not be set"));
                return;
            };
            game.armor[CHEST] = Some(one);
            game.native_feedback(Text::literal(format!(
                "Iron Jetpacks: engine {}",
                if on { "on" } else { "off" }
            )));
        }
    }
}
