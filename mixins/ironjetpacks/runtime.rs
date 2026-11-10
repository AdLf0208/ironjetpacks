// The pure flight model of one Iron Jetpacks tier. This file is both the
// standalone Cargo library and the injected `crate::game::ironjetpacks`
// module; it has no game dependency, so the rules are unit-tested without
// staging Horizon.
//
// Upstream: `PlayerTickHandler.onPlayerTick` (26.1 branch), lines 15-100. The
// iron tier's numbers are `ModJetpacks.java:30` (`IRON.setStats(...)`).

/// The flight numbers of one jetpack tier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tier {
    /// Energy stored at full (`capacity`, FE).
    pub capacity: u32,
    /// Energy one thrusting tick spends (`usage`, FE/t).
    pub usage: u32,
    /// `speedVert`: the ascent cap in blocks a tick.
    pub speed_vert: f64,
    /// `accelVert`: the per-tick acceleration.
    pub accel_vert: f64,
    /// `speedHoverAscend`: the hover-mode ascent cap.
    pub speed_hover_ascend: f64,
    /// `speedHoverDescend`: the fast hover fall speed.
    pub speed_hover_descend: f64,
    /// `speedHoverSlow`: the slow hover fall speed.
    pub speed_hover_slow: f64,
    /// `sprintSpeedVert`: the ascent multiplier while sprinting.
    pub sprint_speed_vert: f64,
    /// `sprintFuel`: the fuel-usage multiplier while sprinting.
    pub sprint_fuel: f64,
}

/// Iron Jetpacks' iron tier, `ModJetpacks.java:30`:
/// `IRON.setStats(800000, 120, 0.41, 0.12, 0.14, 0.27, 0.25, 0.075, 1.1, 1.05, 2.1)`.
pub const IRON: Tier = Tier {
    capacity: 800_000,
    usage: 120,
    speed_vert: 0.41,
    accel_vert: 0.12,
    speed_hover_ascend: 0.27,
    speed_hover_descend: 0.25,
    speed_hover_slow: 0.075,
    sprint_speed_vert: 1.05,
    sprint_fuel: 2.1,
};

/// What the player's input and body say this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input {
    /// `InputHandler.isHoldingUp`: the jump/ascend key is held.
    pub up: bool,
    /// `InputHandler.isHoldingDown`: the descend key (sneak) is held.
    pub down: bool,
    /// `InputHandler.isHoldingSprint` or `Player.isSprinting`.
    pub sprint: bool,
    pub on_ground: bool,
    pub in_water: bool,
}

/// The jetpack's per-item state for one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    /// The engine component; upstream defaults it off, the slice keeps it on.
    pub engine_on: bool,
    /// The hover component; the slice has no toggle yet.
    pub hover: bool,
    /// A creative player spends no fuel and flies with an empty tank.
    pub creative: bool,
    /// `JetpackUtils.getThrottle`: 0.1 to 1.0.
    pub throttle: f64,
    /// Energy left at the start of the tick.
    pub fuel: u32,
}

/// What one tick does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// Energy left after this tick's extraction.
    pub fuel: u32,
    /// The energy this tick takes out of the tank (upstream's `extractEnergy`
    /// result): 0 on an idle tick, the clamped remainder on a short tank.
    pub spent: u32,
    /// The vertical velocity to set, when the jetpack thrusts this tick.
    pub velocity_y: Option<f64>,
}

/// One `PlayerTickEvent.Pre` for a worn jetpack. `motion_y` is the player's
/// current vertical velocity.
pub fn step(tier: &Tier, state: State, input: Input, motion_y: f64) -> Step {
    let idle = Step {
        fuel: state.fuel,
        spent: 0,
        velocity_y: None,
    };
    if !state.engine_on {
        return idle;
    }
    // `PlayerTickHandler.java:27`: the ascend key, or hover while airborne.
    if !(input.up || (state.hover && !input.on_ground)) {
        return idle;
    }
    // `PlayerTickHandler.java:35`: a sprinting player burns the multiplier.
    let usage = if input.sprint {
        (f64::from(tier.usage) * tier.sprint_fuel) as u32
    } else {
        tier.usage
    };
    // `PlayerTickHandler.java:40-45`: creative players extract nothing.
    let fuel = if state.creative {
        state.fuel
    } else {
        state.fuel.saturating_sub(usage)
    };
    let spent = state.fuel - fuel;
    // `PlayerTickHandler.java:51`: the tank is read after the extraction, so
    // the tick that empties it does not thrust.
    if fuel == 0 && !state.creative {
        return Step {
            fuel,
            spent,
            velocity_y: None,
        };
    }
    // `PlayerTickHandler.java:32`: stronger acceleration from rest/falling.
    let accel = tier.accel_vert * if motion_y < 0.3 { 2.5 } else { 1.0 };
    // `PlayerTickHandler.java:53`: only a rising or level sprint gets it.
    let sprint = if motion_y >= 0.0 && input.sprint {
        tier.sprint_speed_vert
    } else {
        1.0
    };
    let velocity_y = match (input.up, state.hover) {
        (true, false) => {
            // `PlayerTickHandler.java:33`: water quarters the vertical cap.
            let cap = tier.speed_vert * if input.in_water { 0.4 } else { 1.0 };
            (motion_y + accel).min(cap) * state.throttle * sprint
        }
        (true, true) => {
            if input.down {
                // `PlayerTickHandler.java:60`.
                (motion_y + accel).min(-tier.speed_hover_slow)
            } else {
                // `PlayerTickHandler.java:62`.
                (motion_y + accel).min(tier.speed_hover_ascend) * state.throttle * sprint
            }
        }
        (false, true) => {
            // `PlayerTickHandler.java:31,66`: the slow fall, or the descend
            // speed while the descend key is held.
            let hover_speed = if input.down {
                tier.speed_hover_descend
            } else {
                tier.speed_hover_slow
            };
            (motion_y + accel).min(-hover_speed)
        }
        (false, false) => unreachable!("the trigger above returned"),
    };
    Step {
        fuel,
        spent,
        velocity_y: Some(velocity_y),
    }
}

/// `PlayerInput`'s flag bits (`horizon-protocol/src/packets/play.rs`), as
/// `native_player_input` hands them to the server for a guest.
pub const FLAG_JUMP: i32 = 16;
pub const FLAG_SHIFT: i32 = 32;
pub const FLAG_SPRINT: i32 = 64;

/// The input a guest's flags carry. Forward/back/left/right are other bits of
/// the same field; only the jetpack's three keys are read here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    /// Jump: upstream's ascend key.
    pub up: bool,
    /// Shift: upstream's descend key (used by the hover rules).
    pub down: bool,
    /// Sprint: the vertical and fuel multiplier.
    pub sprint: bool,
}

/// Decode a guest's `PlayerInput` flags.
pub fn input_from_flags(flags: i32) -> Flags {
    Flags {
        up: flags & FLAG_JUMP != 0,
        down: flags & FLAG_SHIFT != 0,
        sprint: flags & FLAG_SPRINT != 0,
    }
}

/// A command the mod owns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Not an `ironjetpacks` command.
    Unknown,
    /// `ironjetpacks fuel`: report the worn jetpack's tank.
    Query,
    /// `ironjetpacks fuel <amount>`: fill or drain the worn jetpack's tank.
    Set(u32),
    /// `ironjetpacks engine`: flip the engine.
    ToggleEngine,
    /// `ironjetpacks engine on|off`: set the engine.
    SetEngine(bool),
    /// An `ironjetpacks` command with a bad argument.
    Bad,
}

/// Parse `/ironjetpacks fuel [amount]` or `/ironjetpacks engine [on|off]`
/// (without the leading slash).
pub fn parse(command: &str) -> Action {
    let mut words = command.split_whitespace();
    match (words.next(), words.next(), words.next()) {
        (Some("ironjetpacks"), Some("fuel"), None) => Action::Query,
        (Some("ironjetpacks"), Some("fuel"), Some(amount)) if words.next().is_none() => {
            match amount.parse::<u32>() {
                Ok(amount) => Action::Set(amount.min(IRON.capacity)),
                Err(_) => Action::Bad,
            }
        }
        (Some("ironjetpacks"), Some("engine"), None) => Action::ToggleEngine,
        (Some("ironjetpacks"), Some("engine"), Some(on)) if words.next().is_none() => match on {
            "on" => Action::SetEngine(true),
            "off" => Action::SetEngine(false),
            _ => Action::Bad,
        },
        (Some("ironjetpacks"), _, _) => Action::Bad,
        _ => Action::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(up: bool) -> Input {
        Input {
            up,
            down: false,
            sprint: false,
            on_ground: true,
            in_water: false,
        }
    }

    fn state(fuel: u32) -> State {
        State {
            engine_on: true,
            hover: false,
            creative: false,
            throttle: 1.0,
            fuel,
        }
    }

    fn velocity(step: Step) -> f64 {
        step.velocity_y.expect("the jetpack thrusts")
    }

    #[test]
    fn engine_off_does_nothing() {
        let mut state = state(1_000);
        state.engine_on = false;
        let step = step(&IRON, state, input(true), 0.0);
        assert_eq!(step.fuel, 1_000);
        assert_eq!(step.velocity_y, None);
    }

    #[test]
    fn no_up_input_without_hover_does_nothing() {
        let step = step(&IRON, state(1_000), input(false), 0.0);
        assert_eq!(step.fuel, 1_000);
        assert_eq!(step.velocity_y, None);
    }

    #[test]
    fn ascent_accelerates_below_the_point_three_threshold() {
        // accelVert 0.12 x 2.5 while the player is slower than 0.3/tick.
        let step = step(&IRON, state(1_000), input(true), 0.0);
        assert_eq!(velocity(step), 0.3);
        assert_eq!(step.fuel, 880);
    }

    #[test]
    fn ascent_uses_the_plain_acceleration_at_or_above_point_three() {
        let step = step(&IRON, state(1_000), input(true), 0.35);
        assert_eq!(velocity(step), 0.41);
    }

    #[test]
    fn ascent_caps_at_the_vertical_speed() {
        let step = step(&IRON, state(1_000), input(true), 0.41);
        assert_eq!(velocity(step), 0.41);
    }

    #[test]
    fn water_quarters_the_vertical_cap() {
        let mut input = input(true);
        input.in_water = true;
        let step = step(&IRON, state(1_000), input, 0.0);
        assert_eq!(velocity(step), 0.164);
    }

    #[test]
    fn sprint_multiplies_the_ascent() {
        let mut input = input(true);
        input.sprint = true;
        let step = step(&IRON, state(1_000), input, 0.0);
        // min(0.3, 0.41) x 1.05.
        assert_eq!(velocity(step), 0.315);
        // 120 x 2.1, truncated as upstream's (int) cast.
        assert_eq!(step.fuel, 748);
    }

    #[test]
    fn sprint_does_not_multiply_a_fall() {
        let mut input = input(true);
        input.sprint = true;
        let step = step(&IRON, state(1_000), input, -0.2);
        // min(-0.2 + 0.3, 0.41) x 1.0: the multiplier needs motionY >= 0.
        assert!((velocity(step) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn throttle_scales_the_ascent() {
        let mut state = state(1_000);
        state.throttle = 0.5;
        let step = step(&IRON, state, input(true), 0.0);
        assert_eq!(velocity(step), 0.15);
    }

    #[test]
    fn the_last_fuel_tick_still_thrusts() {
        let step = step(&IRON, state(200), input(true), 0.0);
        assert_eq!(step.fuel, 80);
        assert_eq!(velocity(step), 0.3);
    }

    #[test]
    fn the_tick_that_empties_the_tank_does_not_thrust() {
        // Upstream extracts first, then asks whether any energy is left.
        let step = step(&IRON, state(100), input(true), 0.0);
        assert_eq!(step.fuel, 0);
        assert_eq!(step.velocity_y, None);
    }

    #[test]
    fn an_empty_tank_does_not_thrust() {
        let step = step(&IRON, state(0), input(true), 0.0);
        assert_eq!(step.fuel, 0);
        assert_eq!(step.velocity_y, None);
    }

    #[test]
    fn creative_does_not_drain_and_flies_empty() {
        let mut state = state(0);
        state.creative = true;
        let step = step(&IRON, state, input(true), 0.0);
        assert_eq!(step.fuel, 0);
        assert_eq!(velocity(step), 0.3);
    }

    #[test]
    fn hover_without_up_descends_at_the_slow_speed() {
        let mut state = state(1_000);
        state.hover = true;
        let mut input = input(false);
        input.on_ground = false;
        let step = step(&IRON, state, input, 0.0);
        // min(0 + 0.3, -0.075).
        assert_eq!(velocity(step), -0.075);
    }

    #[test]
    fn hover_with_down_descends_at_the_descend_speed() {
        let mut state = state(1_000);
        state.hover = true;
        let mut input = input(false);
        input.on_ground = false;
        input.down = true;
        let step = step(&IRON, state, input, 0.0);
        assert_eq!(velocity(step), -0.25);
    }

    #[test]
    fn hover_on_the_ground_without_up_does_nothing() {
        let mut state = state(1_000);
        state.hover = true;
        let step = step(&IRON, state, input(false), 0.0);
        assert_eq!(step.velocity_y, None);
        assert_eq!(step.fuel, 1_000);
    }

    #[test]
    fn hover_ascends_at_the_hover_cap() {
        let mut state = state(1_000);
        state.hover = true;
        let mut input = input(true);
        input.on_ground = false;
        let step = step(&IRON, state, input, 0.0);
        assert_eq!(velocity(step), 0.27);
    }

    #[test]
    fn hover_down_while_ascending_uses_the_slow_fall_speed() {
        // Upstream's `-jetpack.speedHoverSlow` in the hover + up + down branch.
        let mut state = state(1_000);
        state.hover = true;
        let mut input = input(true);
        input.on_ground = false;
        input.down = true;
        let step = step(&IRON, state, input, 0.0);
        assert_eq!(velocity(step), -0.075);
    }

    #[test]
    fn parses_the_fuel_query() {
        assert_eq!(parse("ironjetpacks fuel"), Action::Query);
    }

    #[test]
    fn parses_a_fuel_amount() {
        assert_eq!(parse("ironjetpacks fuel 0"), Action::Set(0));
        assert_eq!(parse("ironjetpacks fuel 1234"), Action::Set(1234));
    }

    #[test]
    fn clamps_a_fuel_amount_to_the_tier_capacity() {
        assert_eq!(parse("ironjetpacks fuel 9999999"), Action::Set(800_000));
    }

    #[test]
    fn rejects_other_and_bad_commands() {
        assert_eq!(parse("time set day"), Action::Unknown);
        assert_eq!(parse("ironjetpacks"), Action::Bad);
        assert_eq!(parse("ironjetpacks fuel -1"), Action::Bad);
        assert_eq!(parse("ironjetpacks fuel lots"), Action::Bad);
        assert_eq!(parse("ironjetpacks fuel 1 2"), Action::Bad);
    }

    #[test]
    fn the_spent_energy_is_the_usage_of_a_thrusting_tick() {
        let step = step(&IRON, state(1_000), input(true), 0.0);
        assert_eq!(step.spent, 120);
        assert_eq!(step.fuel, 880);
    }

    #[test]
    fn the_spent_energy_is_the_sprint_usage_when_sprinting() {
        let mut input = input(true);
        input.sprint = true;
        let step = step(&IRON, state(1_000), input, 0.0);
        assert_eq!(step.spent, 252);
        assert_eq!(step.fuel, 748);
    }

    #[test]
    fn an_idle_tick_spends_nothing() {
        assert_eq!(step(&IRON, state(1_000), input(false), 0.0).spent, 0);
        let mut engine_off = state(1_000);
        engine_off.engine_on = false;
        assert_eq!(step(&IRON, engine_off, input(true), 0.0).spent, 0);
    }

    #[test]
    fn the_tick_that_empties_the_tank_spends_what_is_left() {
        // Upstream's extract is clamped by the tank, so a short tank spends
        // its remaining energy and still does not thrust.
        let step = step(&IRON, state(100), input(true), 0.0);
        assert_eq!(step.spent, 100);
        assert_eq!(step.fuel, 0);
        assert_eq!(step.velocity_y, None);
    }

    #[test]
    fn a_creative_tick_spends_nothing() {
        let mut creative = state(0);
        creative.creative = true;
        let step = step(&IRON, creative, input(true), 0.0);
        assert_eq!(step.spent, 0);
        assert_eq!(step.velocity_y, Some(0.3));
    }

    #[test]
    fn the_guest_input_flags_carry_jump_shift_and_sprint() {
        assert_eq!(input_from_flags(0), Flags::default());
        assert_eq!(
            input_from_flags(FLAG_JUMP),
            Flags {
                up: true,
                ..Flags::default()
            }
        );
        assert_eq!(
            input_from_flags(FLAG_SHIFT),
            Flags {
                down: true,
                ..Flags::default()
            }
        );
        assert_eq!(
            input_from_flags(FLAG_SPRINT),
            Flags {
                sprint: true,
                ..Flags::default()
            }
        );
        // Forward/back/left/right are other bits of the same field and are
        // not this port's input.
        assert_eq!(
            input_from_flags(1 | 2 | 4 | 8 | FLAG_JUMP | FLAG_SHIFT | FLAG_SPRINT),
            Flags {
                up: true,
                down: true,
                sprint: true,
            }
        );
    }

    #[test]
    fn parses_the_engine_toggle() {
        assert_eq!(parse("ironjetpacks engine"), Action::ToggleEngine);
        assert_eq!(parse("ironjetpacks engine on"), Action::SetEngine(true));
        assert_eq!(parse("ironjetpacks engine off"), Action::SetEngine(false));
        assert_eq!(parse("ironjetpacks engine maybe"), Action::Bad);
        assert_eq!(parse("ironjetpacks engine on now"), Action::Bad);
    }
}
