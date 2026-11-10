// The insert-only FE buffer the generator declares (API 142).
fn observe_block_entity_energy(
    block: &str,
    energy: &mut Option<horizon_client::containers::ModEnergyBuffer>,
) {
    crate::game::justdirethings_state::on_block_entity_energy(block, energy);
}
