// The generator ticks on the server.
fn observe_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::justdirethings_state::on_block_entity_ticks(block, ticks);
}
