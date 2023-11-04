use std::sync::atomic::Ordering;

use smash::app::BattleObject;
use smashline::Hash40;

#[allow(dead_code)]
pub mod fighters;
pub mod weapons;

#[skyline::hook(offset = 0x6079b0)]
fn fighter_initialize_object_data(
    fighter: &mut BattleObject,
    id: u32,
    kind: i32,
    entry_id: i32,
    hash: Hash40,
) {
    fighters::CURRENT_PLAYER_ID.store(entry_id as usize, Ordering::Relaxed);
    weapons::CURRENT_OWNER_KIND.store(kind, Ordering::Relaxed);

    call_original!(fighter, id, kind, entry_id, hash);

    weapons::CURRENT_OWNER_KIND.store(-1, Ordering::Relaxed);
    fighters::CURRENT_PLAYER_ID.store(usize::MAX, Ordering::Relaxed);
}

pub fn install() {
    // fighters::install();
    weapons::install();
    skyline::install_hooks!(fighter_initialize_object_data);
}
