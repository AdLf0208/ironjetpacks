// Runs at the head of `Game::native_perform_command` (API 8): answer
// `/ironjetpacks fuel`.
fn observe_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::ironjetpacks_state::on_perform_command(game, player, command, ran);
}
