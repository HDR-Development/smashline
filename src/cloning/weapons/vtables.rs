use std::{collections::BTreeMap, sync::atomic::Ordering};
use locks::RwLock;

use crate::cloning::{RESOLVE_AS_ORIGINAL, weapons::original_kind_of};

pub static WEAPON_VTABLE_UPDATE: RwLock<BTreeMap<i32, BTreeMap<usize, usize>>> = RwLock::new(BTreeMap::new());

pub const WEAPON_VTABLE_SLOTS: usize = 104;

pub static WEAPON_VTABLE_CACHE: RwLock<BTreeMap<i32, &'static WeaponVtable>> =
    RwLock::new(BTreeMap::new());

#[repr(C)]
pub struct WeaponVtable {
    pub vptr: *const usize,
    padding: [usize; 2],
    pub slots: [usize; WEAPON_VTABLE_SLOTS]
}

#[skyline::hook(offset = 0x33bed40)]
pub fn get_weapon_vtable_hook(kind: i32) -> *const usize {
    if RESOLVE_AS_ORIGINAL.load(Ordering::Relaxed) {
        let original_kind = original_kind_of(kind).unwrap_or(kind);
        return call_original!(original_kind);
    }
    if let Some(cached) = WEAPON_VTABLE_CACHE.read().get(&kind) {
        return *cached as *const WeaponVtable as *const usize;
    }
    let original_kind = original_kind_of(kind).unwrap_or(kind);
    let original_vtable = call_original!(original_kind);

    let weapon_vtable_update = WEAPON_VTABLE_UPDATE.read();
    let Some(vtable_update) = weapon_vtable_update.get(&kind) else {
        println!("[smashline::weapons] Weapon Kind {:#x} has no vtable functions to replace!", kind);
        return original_vtable;
    };
    println!("[smashline::weapons] Replacing vtable functions for Weapon Kind {:#x}", kind);

    let vptr = unsafe { *(original_vtable as *const *const usize) };
    let original_slots = unsafe { std::slice::from_raw_parts(vptr, WEAPON_VTABLE_SLOTS) };

    let mut new_vtable_slots = [0usize; WEAPON_VTABLE_SLOTS];

    unsafe { std::ptr::copy_nonoverlapping(vptr, new_vtable_slots.as_mut_ptr(), WEAPON_VTABLE_SLOTS); }

    for (index, func) in vtable_update.iter() {
        println!(
            "[smashline::weapons] Replacing {:#x} Vtable Entry {} with {:#x}",
            kind,
            *index,
            *func
        );
        new_vtable_slots[*index] = *func;
    }

    let patched = Box::leak(Box::new(WeaponVtable {
        vptr: std::ptr::null(),
        padding: [0; 2],
        slots: new_vtable_slots,
    }));
    patched.vptr = patched.slots.as_ptr();
    WEAPON_VTABLE_CACHE.write().insert(kind, patched);
    patched as *const WeaponVtable as *const usize
}

pub fn install() {
    skyline::install_hooks!(
        get_weapon_vtable_hook
    );
}