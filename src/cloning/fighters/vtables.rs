use std::{collections::BTreeMap, sync::atomic::Ordering};

use locks::RwLock;
use smashline::{Costume, Hash40};

use crate::{cloning::{RESOLVE_AS_ORIGINAL, fighters::{CURRENT_INIT_COSTUME, NO_COSTUME}}, create_agent::LOWERCASE_FIGHTER_NAMES};

pub const FIGHTER_VTABLE_SLOTS: usize = 147;

#[repr(C)]
pub struct FighterVtable {
    vptr: *const usize,
    padding: [usize; 2],
    pub slots: [usize; FIGHTER_VTABLE_SLOTS],
}

pub struct FighterVtableUpdate {
    /// Empty costume means the replacement applies to every costume no registered costume set claims
    /// (and to lookups made without a fighter instance).
    pub costume: Costume,
    pub slots: BTreeMap<usize, usize>,
}

impl FighterVtableUpdate {
    fn is_default(&self) -> bool {
        self.costume.as_slice().is_empty()
    }
}

pub static FIGHTER_VTABLE_UPDATE: RwLock<BTreeMap<i32, Vec<FighterVtableUpdate>>> =
    RwLock::new(BTreeMap::new());

pub static FIGHTER_VTABLE_CACHE: RwLock<BTreeMap<(i32, i32), &'static FighterVtable>> =
    RwLock::new(BTreeMap::new());

#[skyline::hook(offset = 0x68d530)]
pub fn get_fighter_vtable_hook(kind: i32) -> *const usize {
    if RESOLVE_AS_ORIGINAL.load(Ordering::Relaxed) {
        return call_original!(kind);
    }
    let costume = CURRENT_INIT_COSTUME.load(Ordering::Relaxed);

    if let Some(cached) = FIGHTER_VTABLE_CACHE.read().get(&(kind, costume)) {
        return *cached as *const FighterVtable as *const usize;
    }

    let original_vtable = call_original!(kind);

    let fighter_vtable_update = FIGHTER_VTABLE_UPDATE.read();
    let Some(updates) = fighter_vtable_update.get(&kind) else {
        return original_vtable;
    };

    // Same rule as the script installers (create_agent.rs): a costume claimed by any registered costume
    // set for this fighter only takes entries that list it; an unclaimed costume (and lookups made
    // without a fighter instance) only takes default entries. The two never layer.
    let has_costume = costume != NO_COSTUME
        && LOWERCASE_FIGHTER_NAMES
            .get(kind as usize)
            .map_or(false, |name| crate::utils::has_costume(Hash40::new(name), costume));
    let mut merged: BTreeMap<usize, usize> = BTreeMap::new();
    for update in updates.iter().filter(|update| {
        if has_costume {
            update.costume.as_slice().contains(&(costume as usize))
        } else {
            update.is_default()
        }
    }) {
        merged.extend(update.slots.iter().map(|(slot, func)| (*slot, *func)));
    }

    if merged.is_empty() {
        return original_vtable;
    }
    println!(
        "[smashline::fighters] Replacing vtable functions for Fighter Kind {:#x} costume {}",
        kind,
        costume
    );

    let vptr = unsafe { *(original_vtable as *const *const usize) };

    let mut new_vtable_slots = [0usize; FIGHTER_VTABLE_SLOTS];
    unsafe { std::ptr::copy_nonoverlapping(vptr, new_vtable_slots.as_mut_ptr(), FIGHTER_VTABLE_SLOTS); }

    for (index, func) in merged.iter() {
        println!(
            "[smashline::fighters] Replacing {:#x} Vtable Entry {} with {:#x}",
            kind,
            *index,
            *func
        );
        new_vtable_slots[*index] = *func;
    }

    let patched = Box::leak(Box::new(FighterVtable {
        vptr: std::ptr::null(),
        padding: [0; 2],
        slots: new_vtable_slots,
    }));
    patched.vptr = patched.slots.as_ptr();
    FIGHTER_VTABLE_CACHE.write().insert((kind, costume), patched);
    patched as *const FighterVtable as *const usize
}

pub fn install() {
    skyline::install_hooks!(get_fighter_vtable_hook);
}