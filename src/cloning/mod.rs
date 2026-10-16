use std::sync::atomic::Ordering;

use smash::app::BattleObject;
use smashline::Hash40;

#[allow(dead_code)]
pub mod fighters;
pub mod weapons;

#[skyline::hook(offset = 0x6079d0)]
fn fighter_initialize_object_data(
    fighter: &mut BattleObject,
    id: u32,
    kind: i32,
    entry_id: i32,
    hash: Hash40,
) {
    // Both fighter vtable lookups (here and in the nested module-accessor init) happen inside this call,
    // so the costume is only published for its duration.
    let costume = crate::utils::get_costume_from_entry_id(entry_id).unwrap_or(fighters::NO_COSTUME);
    let previous = fighters::CURRENT_INIT_COSTUME.swap(costume, Ordering::Relaxed);
    call_original!(fighter, id, kind, entry_id, hash);
    fighters::CURRENT_INIT_COSTUME.store(previous, Ordering::Relaxed);
}

pub fn install() {
    fighters::install();
    weapons::install();
    crate::utils::install_module_hooks();
    skyline::install_hooks!(fighter_initialize_object_data);
}
