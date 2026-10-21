//! Per-kind enabling of the weapon modules the game only creates for some kinds.
//!
//! The weapon module init (0x33b5bf0) builds its optional modules from a factory row at
//! `0x518a818 + row_index * 0xe8`: 29 function pointers, each called as `factory(module_accessor)`
//! and stored into the accessor. Kinds without a given module hold a stub that returns null, and the
//! init then stores a shared do-nothing module in its place. The row index comes from a kind switch
//! inside the function (and, for clones, from `weapon_init_factory_kind`), so several kinds share one
//! row and a plain table patch leaks into all of them.
//!
//! This hooks the first slot load (0x33b64f8, `ldr x8,[x25,x8]`; x24 = row index, x25 = table,
//! x28 = kind, x8 = index * 0xe8). For a kind with registrations it points x25 at a private copy of
//! the row with the requested factories filled in and zeroes the index, so every following
//! `madd x8,x24,x8,x25` reads the copy. Neither x24 nor x25 is read again before being reassigned
//! after the slot loop (verified 13.0.5).

use std::collections::BTreeMap;

use locks::RwLock;
use skyline::hooks::InlineCtx;

pub const WEAPON_MODULE_SLOTS: usize = 29;
const ROW_SIZE: usize = 0xe8;

/// The game's own factory for each row slot (text offset), `None` where no row holds one.
/// Comments give the module accessor field the slot fills, as read by the `lua_bind` accessors.
pub const WEAPON_MODULE_FACTORIES: [Option<usize>; WEAPON_MODULE_SLOTS] = [
    Some(0x33b89a0), // 0  KineticModule        (+0x68)
    Some(0x33b97b0), // 1  ArticleModule        (+0x98)
    Some(0x33b8af0), // 2  AttackModule         (+0xa0)
    Some(0x33b95b0), // 3  ControlModule        (+0x48)
    Some(0x33b8ce0), // 4  EffectModule         (+0x140)
    Some(0x33b8e00), // 5  GroundModule         (+0x58)  plain variant; hydrant/forge use their own
    Some(0x33b8e90), // 6  HitModule            (+0xb0)
    Some(0x33b8300), // 7  MotionModule         (+0x88)
    Some(0x33b8f80), // 8  ReflectModule        (+0xf8)
    Some(0x33b9030), // 9  SearchModule         (+0xe0)
    Some(0x33b9210), // 10 SoundModule          (+0x148)
    Some(0x33b9a20), // 11 VisibilityModule     (+0x150) full variant; 0x33b8470 is the 0x18-byte one
    Some(0x33b8510), // 12 ColorBlendModule     (+0x70)  common variant; 0x33b9b80 is the other
    Some(0x33b85a0), // 13 ShakeModule          (+0x168)
    Some(0x33ba5c0), // 14 ItemModule           (+0xc8)
    Some(0x33b9500), // 15 AreaModule           (+0xc0)
    Some(0x33b9cf0), // 16 JostleModule         (+0x118)
    Some(0x33b9c40), // 17 CameraModule         (+0x60)  full variant; 0x33b92d0 is the 0x20-byte one
    Some(0x33b9ae0), // 18 PhysicsModule        (+0x80)  0x1b0-byte variant; 0x33b8690 is the 0xd0-byte one
    Some(0x33b9360), // 19 DamageModule         (+0xa8)  common variant; 0x33ba520 is the other
    Some(0x33b9fa0), // 20 CatchModule          (+0x120)
    Some(0x33ba060), // 21 GrabModule           (+0x158)
    Some(0x33b9720), // 22 SlopeModule          (+0x160)
    Some(0x33b9de0), // 23 ReflectorModule      (+0x108)
    Some(0x33b9900), // 24 SlowModule           (+0x170) common variant; 0x33ba210 is the other
    Some(0x33b8780), // 25 MotionAnimcmdModule  (+0x188)
    Some(0x33b9ef0), // 26 TurnModule           (+0xf0)
    Some(0x33b93f0), // 27 LuaModule            (+0x190) smashline already forces this for every weapon
    None,            // 28 InkPaintModule       (+0x198) every row holds a stub
];

/// Weapon kind -> row slot -> factory text offset.
pub static WEAPON_MODULE_UPDATE: RwLock<BTreeMap<i32, BTreeMap<usize, usize>>> =
    RwLock::new(BTreeMap::new());

/// Leaked patched rows, built on first init of each registered kind.
static MODULE_CACHE: RwLock<BTreeMap<i32, &'static [usize; WEAPON_MODULE_SLOTS]>> =
    RwLock::new(BTreeMap::new());

pub fn set_factory(weapon_kind: i32, slot: usize, factory_offset: usize) {
    println!(
        "[smashline::modules] Weapon Kind {:#x} gets module slot {} from factory {:#x}",
        weapon_kind,
        slot,
        factory_offset
    );
    WEAPON_MODULE_UPDATE
        .write()
        .entry(weapon_kind)
        .or_default()
        .insert(slot, factory_offset);
    MODULE_CACHE.write().clear();
}

fn text() -> usize {
    unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize }
}

unsafe fn patched_row(kind: i32, original_row: *const usize) -> Option<&'static [usize; WEAPON_MODULE_SLOTS]> {
    if let Some(cached) = MODULE_CACHE.read().get(&kind) {
        return Some(*cached);
    }

    let updates = WEAPON_MODULE_UPDATE.read();
    let slots = updates.get(&kind)?;

    let mut row = [0usize; WEAPON_MODULE_SLOTS];
    std::ptr::copy_nonoverlapping(original_row, row.as_mut_ptr(), WEAPON_MODULE_SLOTS);
    for (slot, factory) in slots {
        row[*slot] = text() + factory;
    }

    let row: &'static [usize; WEAPON_MODULE_SLOTS] = Box::leak(Box::new(row));
    MODULE_CACHE.write().insert(kind, row);
    Some(row)
}

#[skyline::hook(offset = 0x33b64f8, inline)]
unsafe fn weapon_init_factory_row(ctx: &mut InlineCtx) {
    let kind = ctx.registers[28].w() as i32;
    let table = ctx.registers[25].x() as usize;
    let index = ctx.registers[24].x() as usize;

    let Some(row) = patched_row(kind, (table + index * ROW_SIZE) as *const usize) else {
        return;
    };

    ctx.registers[25].set_x(row.as_ptr() as u64);
    ctx.registers[24].set_x(0);
    ctx.registers[8].set_x(0);
}

pub fn install() {
    skyline::install_hooks!(weapon_init_factory_row);
}
