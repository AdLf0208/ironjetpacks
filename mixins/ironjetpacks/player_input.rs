// Runs at the head of `native_player_input` (API 343): a guest's jump, shift
// and sprint flags feed the same flight model the host's window key does.
fn observe_player_input(player: u128, name: &str, flags: i32) {
    crate::game::ironjetpacks_state::on_player_input(player, name, flags);
}
