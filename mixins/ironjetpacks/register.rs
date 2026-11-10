// Runs at the head of `Game::native_register_commands` (API 8): declare the
// `/ironjetpacks fuel` usages.
fn observe_register_commands(commands: &mut Vec<NativeCommand>) {
    crate::game::ironjetpacks_state::on_register_commands(commands);
}
