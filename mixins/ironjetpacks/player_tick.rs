// Runs at the head of `Game::native_player_tick` (API 31): one player's server
// tick end, after the server's travel.
fn observe_player_tick(game: &mut crate::game::Game<'_>) {
    crate::game::ironjetpacks_state::on_player_tick(game);
}
