// Game-side glue for Just Dire Things slice 1 (the Simple Coal Generator).
//
// Injected into `crate::game` as `justdirethings_state`; it needs the game, so
// it is not part of the standalone library (only `runtime.rs` is).
//
// Energy: the generator declares a `Capabilities.Energy.BLOCK` buffer (API 142)
// whose amount is the int `energy` in the block entity's saved compound, as
// upstream's `EnergyStorageNoReceive` keeps it. Upstream's generator cannot be
// filled from outside (`canReceive()` is false), so `max_insert` is 0; it gives
// its FE out through the same buffer, so every face may drain it.
//
// Fuel: the burn time and the speed multiplier are the engine's
// `minecraft:cooking_fuel` values (`Providers::burn_time` / `Providers::speed`,
// the counterpart of `stack.getBurnTime(SMELTING)` and of NeoForge's
// `furnace_fuels` data map), so vanilla and modded fuels work without a table
// of our own. Upstream's own coals carry their burn time and multiplier in the
// carried items' components.

use horizon_client::containers::{BlockEntity, ModBlockEntity, ModContainer, ModEnergyBuffer};
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_world::nbt::{Compound, JavaString, Tag};

use crate::game::justdirethings::{CAPACITY, Fuel, Generator, clamp_energy};
use crate::game::native_server::{NativeCommand, NativeServerPlayer};

/// The block this slice implements.
const GENERATOR: &str = "justdirethings:generatort1";
/// The fuel slot (upstream's `MACHINE_SLOTS = 1`).
const FUEL_SLOT: usize = 0;

fn nbt_key(name: &str) -> JavaString {
    JavaString::from(name)
}

fn read_int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&nbt_key(key)) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

fn write_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(nbt_key(key), Tag::Int(value));
}

/// `BlockEntity.setChanged`: the chunk is written at the next save.
fn mark_modified(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
}

/// The state as upstream saves it: `energy`, `feRemaining`, `burnRemaining`,
/// `maxBurn`, `fuelBurnMultiplier`.
fn state_of(entity: &ModBlockEntity) -> Generator {
    let mut state = Generator {
        energy: clamp_energy(read_int(&entity.nbt, "energy"), CAPACITY),
        fe_remaining: read_int(&entity.nbt, "feRemaining"),
        burn_remaining: read_int(&entity.nbt, "burnRemaining"),
        max_burn: read_int(&entity.nbt, "maxBurn"),
        fuel_multiplier: read_int(&entity.nbt, "fuelBurnMultiplier"),
    };
    // An old save without `fuelBurnMultiplier` reads 0; 1 is the default.
    if state.fuel_multiplier == 0 {
        state.fuel_multiplier = 1;
    }
    state.normalise();
    state
}

fn store(entity: &mut ModBlockEntity, state: &Generator) {
    write_int(&mut entity.nbt, "energy", state.energy);
    write_int(&mut entity.nbt, "feRemaining", state.fe_remaining);
    write_int(&mut entity.nbt, "burnRemaining", state.burn_remaining);
    write_int(&mut entity.nbt, "maxBurn", state.max_burn);
    write_int(&mut entity.nbt, "fuelBurnMultiplier", state.fuel_multiplier);
}

/// What the engine says one stack is worth as furnace fuel, `None` if it does
/// not burn (`ItemStack.getBurnTime(SMELTING) <= 0`).
fn fuel_of(game: &crate::game::Game<'_>, stack: &ItemStack) -> Option<Fuel> {
    let providers = game.res.providers();
    let items = game.res.items();
    let burn_time = providers.burn_time(items, stack, false);
    if burn_time <= 0 {
        return None;
    }
    Some(Fuel {
        burn_time: u32::try_from(burn_time).unwrap_or(0),
        speed_multiplier: providers.speed(items, stack, false),
    })
}

/// The generator's fuel slot (`native_block_entity_container`, API 16). One
/// slot, reachable from every face, as upstream's `GeneratorItemHandler`.
#[allow(private_interfaces)]
pub fn on_block_entity_container(block: &str, container: &mut Option<ModContainer>) {
    if block != GENERATOR {
        return;
    }
    *container = Some(ModContainer {
        slots: 1,
        faces: std::array::from_fn(|_| vec![0]),
        inputs: vec![0],
        outputs: vec![0],
        menu: None,
    });
}

/// The generator's insert-only FE buffer (API 142). Upstream's storage is an
/// `EnergyStorageNoReceive` of `GENERATOR_T1_MAX_FE`, so nothing may fill it.
#[allow(private_interfaces)]
pub fn on_block_entity_energy(block: &str, energy: &mut Option<ModEnergyBuffer>) {
    if block != GENERATOR {
        return;
    }
    *energy = Some(ModEnergyBuffer {
        capacity: CAPACITY,
        max_insert: 0,
        max_extract: CAPACITY,
        insert_faces: [false; 6],
        extract_faces: [true; 6],
    });
}

/// The block has a block entity (`native_block_entity`, API 15).
#[allow(private_interfaces)]
pub fn on_block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == GENERATOR {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

/// ... and ticks (`native_block_entity_ticks`, API 15).
#[allow(private_interfaces)]
pub fn on_block_entity_ticks(block: &str, ticks: &mut bool) {
    if block == GENERATOR {
        *ticks = true;
    }
}

/// One server tick (`native_block_entity_tick`, API 17).
#[allow(private_interfaces)]
pub fn on_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut ModBlockEntity,
) {
    if entity.id != GENERATOR {
        return;
    }
    let mut state = state_of(entity);
    let slot = entity.items.get_mut(FUEL_SLOT).and_then(Option::take);
    let mut slot = slot;
    let res = &*game;
    let mut pulls = 0_u32;
    // The closure hands the runtime one fuel item at a time and consumes it as
    // upstream's `doBurn` does: one item, or the crafting remainder when the
    // stack runs out. It is called at most twice in a tick.
    let produced = state.tick(true, || {
        let stack = slot.as_ref()?;
        let fuel = fuel_of(res, stack)?;
        let count = stack.count;
        let remainder = res
            .res
            .items()
            .get(&stack.item)
            .and_then(|info| info.remainder.clone());
        pulls += 1;
        slot = if count > 1 {
            let mut kept = stack.clone();
            kept.count = count - 1;
            Some(kept)
        } else {
            remainder.map(|item| ItemStack::new(item.as_str(), 1))
        };
        Some(fuel)
    });
    if let Some(entry) = entity.items.get_mut(FUEL_SLOT) {
        *entry = slot;
    }
    if produced > 0 || pulls > 0 {
        mark_modified(game, pos);
    }
    store(entity, &state);
}

/// `/justdirethings info|set` (`register_commands`, API 8).
#[allow(private_interfaces)]
pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::new("justdirethings info <pos:block_pos>"));
    commands.push(NativeCommand::new(
        "justdirethings set <amount:integer> <pos:block_pos>",
    ));
}

fn parse_pos(words: &[&str]) -> Option<[i32; 3]> {
    Some([
        words.first()?.parse().ok()?,
        words.get(1)?.parse().ok()?,
        words.get(2)?.parse().ok()?,
    ])
}

fn stack_line(stack: Option<&ItemStack>) -> String {
    match stack {
        Some(stack) => format!("{} x{}", stack.item, stack.count),
        None => "empty".to_owned(),
    }
}

/// A machine's status line. `t=` is the world game time, so the e2e can tell a
/// fresh reply from a stale one.
fn status_text(game: &crate::game::Game<'_>, pos: [i32; 3]) -> String {
    let key = (pos[0], pos[1], pos[2]);
    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&key) else {
        return "No Just Dire Things block there.".to_owned();
    };
    if entity.id != GENERATOR {
        return "No Just Dire Things block there.".to_owned();
    }
    let state = state_of(entity);
    format!(
        "Simple Coal Generator at {},{},{}: {}/{} FE (burning {}, fe {} of {}, \
         burn {}/{}, multiplier {}, fuel {}, t={})",
        pos[0],
        pos[1],
        pos[2],
        state.energy,
        CAPACITY,
        state.burning(),
        state.fe_remaining,
        state.max_burn.saturating_mul(state.fe_per_tick()),
        state.burn_remaining,
        state.max_burn,
        state.fuel_multiplier,
        stack_line(entity.items.first().and_then(Option::as_ref)),
        game.world.game_time
    )
}

/// The `/justdirethings` command body (`perform_command`, API 8): `info` reads,
/// `set` (a harness control) writes the stored FE.
#[allow(private_interfaces)]
pub fn on_perform_command(
    game: &mut crate::game::Game<'_>,
    _player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let words: Vec<&str> = command.split_whitespace().collect();
    match words.as_slice() {
        ["justdirethings", "info", x, y, z] => {
            *ran = true;
            let Some(pos) = parse_pos(&[*x, *y, *z]) else {
                game.native_feedback(Text::literal(
                    "Usage: /justdirethings info <x> <y> <z>",
                ));
                return;
            };
            game.native_feedback(Text::literal(status_text(game, pos)));
        }
        ["justdirethings", "set", amount, x, y, z] => {
            *ran = true;
            let Some(pos) = parse_pos(&[*x, *y, *z]) else {
                game.native_feedback(Text::literal(
                    "Usage: /justdirethings set <amount> <x> <y> <z>",
                ));
                return;
            };
            let Ok(amount) = amount.parse::<i32>() else {
                game.native_feedback(Text::literal("The amount must be a whole number of FE."));
                return;
            };
            let Some(after) = game.native_with_mod_block_entity(pos, |entity| {
                if entity.id != GENERATOR {
                    return None;
                }
                let after = clamp_energy(amount, CAPACITY);
                write_int(&mut entity.nbt, "energy", after);
                Some(after)
            }) else {
                game.native_feedback(Text::literal("No Just Dire Things block there."));
                return;
            };
            let Some(after) = after else {
                game.native_feedback(Text::literal("No Just Dire Things block there."));
                return;
            };
            mark_modified(game, pos);
            game.native_feedback(Text::literal(format!("Energy set to {after} FE.")));
        }
        _ => {}
    }
}
