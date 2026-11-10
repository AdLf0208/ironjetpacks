// Runs at the head of `Game::native_tick` (API 1): the window's own tick, the
// only place the local player's raw Space key is visible.
fn observe_tick(game: &mut crate::game::Game<'_>) {
    crate::game::ironjetpacks_state::on_client_tick(game);
}
