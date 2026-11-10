// Runs at the head of `native_item_energy_buffer` (API 307): the jetpack item
// declares the tier's energy tank.
fn observe_item_energy_buffer(
    item: &str,
    buffer: &mut Option<horizon_client::mod_api::ItemEnergyBuffer>,
) {
    crate::game::ironjetpacks_state::on_item_energy_buffer(item, buffer);
}
