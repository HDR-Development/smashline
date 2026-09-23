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
    call_original!(fighter, id, kind, entry_id, hash);
}

pub fn install() {
    // fighters::install();
    weapons::install();
    crate::utils::install_module_hooks();
    skyline::install_hooks!(fighter_initialize_object_data);
}
