// Declare `/justdirethings info|set`.
fn observe_register(commands: &mut Vec<NativeCommand>) {
    crate::game::justdirethings_state::on_register_commands(commands);
}
