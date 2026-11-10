// Runs at the head of `native_commands` (API 35): the client chat box lists
// and completes `/ironjetpacks`.
fn observe_commands(commands: &mut Vec<&'static str>) {
    crate::game::ironjetpacks_state::on_commands(commands);
}
