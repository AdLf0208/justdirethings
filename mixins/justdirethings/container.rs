// The generator's fuel slot.
fn observe_block_entity_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::justdirethings_state::on_block_entity_container(block, container);
}
