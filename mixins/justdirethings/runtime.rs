// Pure machine logic for Just Dire Things slice 1: the Simple Coal Generator.
//
// This file is both the standalone Cargo library and the injected
// `crate::game::justdirethings` module. It has no game types: the hook glue in
// `support.rs` turns a block entity, its fuel slot and the engine's
// `minecraft:cooking_fuel` values into the small values used here.
//
// Numbers are upstream 1.5.7's defaults (`Config.generatorT1Config`); see
// `FINDINGS.md` for the derivation.

/// The generator's buffer (`GENERATOR_T1_MAX_FE`, default 1,000,000 FE).
pub const CAPACITY: i32 = 1_000_000;
/// FE made per burn tick of fuel (`GENERATOR_T1_FE_PER_FUEL_TICK`, default 15).
pub const FE_PER_FUEL_TICK: i32 = 15;
/// The burn-speed multiplier from the config (`GENERATOR_T1_BURN_SPEED_MULTIPLIER`,
/// default 4): the burn time is divided by it and the output multiplied by it.
pub const BURN_SPEED_MULTIPLIER: i32 = 4;

/// One fuel item as the engine describes it: its burn time in ticks and the
/// `speed_multiplier` of its `minecraft:cooking_fuel` component.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fuel {
    pub burn_time: u32,
    pub speed_multiplier: f32,
}

/// Energy clamped into a buffer.
#[must_use]
pub fn clamp_energy(energy: i32, capacity: i32) -> i32 {
    energy.clamp(0, capacity.max(0))
}

/// Upstream's `getBurnSpeedMultiplier`: the config's multiplier times the
/// fuel's own integer multiplier (vanilla fuel is 1, Primal Coal is 2).
#[must_use]
pub fn burn_speed(fuel_multiplier: i32) -> i32 {
    BURN_SPEED_MULTIPLIER.saturating_mul(fuel_multiplier.max(1))
}

/// The generator's saved state, keyed as upstream saves it (`energy`,
/// `feRemaining`, `burnRemaining`, `maxBurn`, `fuelBurnMultiplier`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generator {
    pub energy: i32,
    pub fe_remaining: i32,
    pub burn_remaining: i32,
    pub max_burn: i32,
    pub fuel_multiplier: i32,
}

impl Default for Generator {
    fn default() -> Self {
        Self {
            energy: 0,
            fe_remaining: 0,
            burn_remaining: 0,
            max_burn: 0,
            fuel_multiplier: 1,
        }
    }
}

impl Generator {
    /// The empty generator.
    #[must_use]
    #[allow(
        dead_code,
        reason = "pure API: the glue builds the state from NBT, and the game build does not compile the tests"
    )]
    pub fn new() -> Self {
        Self::default()
    }

    /// Upstream's gate: fuel burns only with some room in the buffer.
    #[must_use]
    pub fn room(&self) -> i32 {
        (CAPACITY - self.energy).max(0)
    }

    /// True while a fuel item is being burned (upstream's `feRemaining > 0`).
    #[must_use]
    pub fn burning(&self) -> bool {
        self.fe_remaining > 0
    }

    /// FE this tick would produce at the current fuel's speed.
    #[must_use]
    pub fn fe_per_tick(&self) -> i32 {
        FE_PER_FUEL_TICK.saturating_mul(burn_speed(self.fuel_multiplier))
    }

    /// Load one fuel item when no FE is left to burn. `next_fuel` takes the
    /// next item out of the slot and answers what it is worth, `None` when the
    /// slot is empty or holds something that does not burn.
    pub fn load_fuel(&mut self, next_fuel: &mut impl FnMut() -> Option<Fuel>) -> bool {
        if self.fe_remaining > 0 {
            return false;
        }
        self.max_burn = 0;
        self.burn_remaining = 0;
        if self.room() <= 0 {
            return false;
        }
        let Some(fuel) = next_fuel() else {
            return false;
        };
        if fuel.burn_time == 0 {
            return false;
        }
        // Upstream keeps an integer multiplier; the engine's value is a float
        // (vanilla 1.0, Primal Coal 2.0), so round it.
        self.fuel_multiplier = fuel.speed_multiplier.max(1.0).round() as i32;
        let speed = burn_speed(self.fuel_multiplier);
        self.fe_remaining =
            i32::try_from(i64::from(fuel.burn_time).saturating_mul(i64::from(FE_PER_FUEL_TICK)))
                .unwrap_or(i32::MAX);
        self.max_burn = (fuel.burn_time / u32::try_from(speed).unwrap_or(1)) as i32;
        self.burn_remaining = self.max_burn.saturating_add(1);
        true
    }

    /// One server tick: burn, then store what the buffer takes. Returns the FE
    /// stored this tick.
    pub fn tick(&mut self, enabled: bool, mut next_fuel: impl FnMut() -> Option<Fuel>) -> i32 {
        if !enabled {
            return 0;
        }
        if !self.burning() {
            self.load_fuel(&mut next_fuel);
        }
        if !self.burning() {
            return 0;
        }
        let per_tick = self.fe_per_tick();
        let stored = per_tick.min(self.fe_remaining).min(self.room()).max(0);
        if stored > 0 {
            self.energy += stored;
            self.fe_remaining -= stored;
        }
        self.burn_remaining = self.burn_remaining.saturating_sub(1);
        let mut produced = stored;
        if !self.burning() {
            // The burn ended inside this tick: upstream loads the next item and
            // tops the tick up, so output does not stutter between fuels.
            self.load_fuel(&mut next_fuel);
            if stored < per_tick {
                let extra = (per_tick - stored)
                    .min(self.fe_remaining)
                    .min(self.room())
                    .max(0);
                if extra > 0 {
                    self.energy += extra;
                    self.fe_remaining -= extra;
                }
                produced += extra;
                self.burn_remaining = self.burn_remaining.saturating_sub(1);
            }
        }
        produced
    }

    /// Clamp what was read back from a save into a valid state.
    pub fn normalise(&mut self) {
        self.energy = clamp_energy(self.energy, CAPACITY);
        self.fe_remaining = self.fe_remaining.max(0);
        self.burn_remaining = self.burn_remaining.max(0);
        self.max_burn = self.max_burn.max(0);
        self.fuel_multiplier = self.fuel_multiplier.max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fuel(burn_time: u32, speed_multiplier: f32) -> Fuel {
        Fuel {
            burn_time,
            speed_multiplier,
        }
    }

    fn one(fuel_item: Fuel) -> impl FnMut() -> Option<Fuel> {
        let mut taken = false;
        move || {
            if taken {
                None
            } else {
                taken = true;
                Some(fuel_item)
            }
        }
    }

    #[test]
    fn the_defaults_are_upstreams() {
        assert_eq!(CAPACITY, 1_000_000);
        assert_eq!(FE_PER_FUEL_TICK, 15);
        assert_eq!(BURN_SPEED_MULTIPLIER, 4);
        assert_eq!(Generator::new().fuel_multiplier, 1);
        assert_eq!(Generator::new().fe_per_tick(), 60);
    }

    #[test]
    fn a_generator_with_no_fuel_makes_nothing() {
        let mut generator = Generator::new();
        let mut empty = || None;
        assert_eq!(generator.tick(true, &mut empty), 0);
        assert_eq!(generator.energy, 0);
        assert!(!generator.burning());
    }

    #[test]
    fn one_coal_yields_twenty_four_thousand_fe_over_four_hundred_ticks() {
        // Vanilla coal: 1,600 burn ticks, speed 1.0 → burn speed 4, 60 FE/t.
        let mut generator = Generator::new();
        let mut fuel = one(fuel(1_600, 1.0));
        let mut ticks = 0;
        let mut total = 0;
        loop {
            let made = generator.tick(true, &mut fuel);
            if made == 0 {
                break;
            }
            total += made;
            ticks += 1;
        }
        assert_eq!(ticks, 400, "1,600 / 4");
        assert_eq!(total, 24_000, "1,600 fuel ticks x 15 FE");
        assert_eq!(generator.energy, 24_000);
        assert_eq!(generator.fe_remaining, 0);
    }

    #[test]
    fn primal_coal_burns_twice_as_fast_for_three_times_the_energy() {
        // coal_t1: 4,800 burn ticks, speed 2.0 → burn speed 8, 120 FE/t.
        let mut generator = Generator::new();
        let mut fuel = one(fuel(4_800, 2.0));
        let mut ticks = 0;
        let mut total = 0;
        loop {
            let made = generator.tick(true, &mut fuel);
            if made == 0 {
                break;
            }
            total += made;
            ticks += 1;
        }
        assert_eq!(ticks, 600, "4,800 / 8");
        assert_eq!(total, 72_000, "4,800 fuel ticks x 15 FE");
        assert_eq!(generator.fuel_multiplier, 2);
    }

    #[test]
    fn a_full_buffer_burns_no_fuel() {
        let mut generator = Generator {
            energy: CAPACITY,
            ..Generator::new()
        };
        let mut calls = 0;
        let mut counter = || {
            calls += 1;
            Some(fuel(1_600, 1.0))
        };
        assert_eq!(generator.tick(true, &mut counter), 0);
        assert_eq!(calls, 0, "the item must not even be looked at");
        assert_eq!(generator.energy, CAPACITY);
    }

    #[test]
    fn a_nearly_full_buffer_keeps_the_rest_of_the_burn() {
        let mut generator = Generator {
            energy: CAPACITY - 10,
            ..Generator::new()
        };
        let mut fuel = one(fuel(1_600, 1.0));
        assert_eq!(generator.tick(true, &mut fuel), 10, "only the room left");
        assert_eq!(generator.energy, CAPACITY);
        assert!(
            generator.burning(),
            "the rest of the fuel stays for when there is room"
        );
        let left = generator.fe_remaining;
        assert_eq!(left, 24_000 - 10);
        // Nothing more fits, so nothing more burns.
        assert_eq!(generator.tick(true, &mut fuel), 0);
        assert_eq!(generator.fe_remaining, left);
    }

    #[test]
    fn one_coal_is_conserved_when_the_buffer_fills_up_and_drains() {
        let mut generator = Generator::new();
        let mut fuel = one(fuel(1_600, 1.0));
        let mut total = 0;
        for tick in 0..1_000 {
            // Drain the buffer every 100 ticks so it never fills for good.
            if tick % 100 == 99 {
                total += generator.energy;
                generator.energy = 0;
            }
            generator.tick(true, &mut fuel);
        }
        total += generator.energy;
        assert_eq!(total, 24_000, "no energy is destroyed at the buffer's edge");
    }

    #[test]
    fn output_continues_across_a_fuel_boundary_in_one_tick() {
        // Two 100-tick fuels are 1,500 FE each; at 60 FE/t that is 50 full
        // ticks, with the boundary tick paying a whole 60 rather than half.
        let mut generator = Generator::new();
        let mut remaining = 2;
        let mut fuel = move || {
            if remaining == 0 {
                return None;
            }
            remaining -= 1;
            Some(fuel(100, 1.0))
        };
        let mut made = Vec::new();
        for _ in 0..50 {
            made.push(generator.tick(true, &mut fuel));
        }
        assert_eq!(made.iter().sum::<i32>(), 3_000, "two 100-tick fuels");
        assert!(
            made.iter().all(|made| *made == 60),
            "the fuel boundary costs no tick: {made:?}"
        );
    }

    #[test]
    fn a_disabled_generator_burns_nothing() {
        let mut generator = Generator::new();
        let mut fuel = one(fuel(1_600, 1.0));
        assert_eq!(generator.tick(false, &mut fuel), 0);
        assert_eq!(generator.energy, 0);
        assert!(!generator.burning());
    }

    #[test]
    fn something_that_does_not_burn_is_not_consumed() {
        let mut generator = Generator::new();
        let mut empty = || None;
        assert!(!generator.load_fuel(&mut empty));
        assert_eq!(generator.fe_remaining, 0);
    }

    #[test]
    fn normalise_clamps_what_a_save_holds() {
        let mut generator = Generator {
            energy: -5,
            fe_remaining: -2,
            burn_remaining: -9,
            max_burn: -1,
            fuel_multiplier: 0,
        };
        generator.normalise();
        assert_eq!(generator.energy, 0);
        assert_eq!(generator.fe_remaining, 0);
        assert_eq!(generator.burn_remaining, 0);
        assert_eq!(generator.max_burn, 0);
        assert_eq!(generator.fuel_multiplier, 1);

        let mut over = Generator {
            energy: CAPACITY + 100,
            ..Generator::new()
        };
        over.normalise();
        assert_eq!(over.energy, CAPACITY);
    }
}
