// Runs at the head of `native_component_types` (API 65): register the mod's
// engine switch before content loads.
fn register_component_types(types: &mut Vec<String>) {
    crate::game::ironjetpacks_state::register_component_types(types);
}
