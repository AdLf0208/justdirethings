// The `/justdirethings` command bodies.
fn observe_perform(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::justdirethings_state::on_perform_command(game, player, command, ran);
}
