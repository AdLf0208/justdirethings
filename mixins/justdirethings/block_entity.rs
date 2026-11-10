// The generator has a block entity.
fn observe_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::justdirethings_state::on_block_entity(block, entity);
}
