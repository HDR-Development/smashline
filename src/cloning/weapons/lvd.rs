//! Per-weapon `.lvd` ground collision for cloned (or vanilla) weapons.
//!
//! Vanilla only wires this mechanism up for Pac-Man's firehydrant and Steve's forge. All offsets are
//! 13.0.5. How the game does it, which this module generalises:
//!
//! 1. **Module factory.** The weapon module init builds every module from the per-kind factory row
//!    (see `modules`). Row slot 5 is the GroundModule. The hydrant and forge rows hold their own
//!    factories there (0x33ba680 / 0x33baa10): a 0x170-byte GroundModule built by the shared weapon
//!    constructor (0x45ceb0) with the lvd fields cleared and a per-kind vtable. Plain weapons get a
//!    null stub and share a do-nothing GroundModule, so a clone of one has nowhere to put an lvd.
//!    [`register`] therefore sets the kind's slot 5 factory to one of those two through
//!    `modules::set_factory`, no custom allocation or vtable needed.
//! 2. **Vtable.** Both per-kind vtables differ from the base weapon GroundModule vtable (0x4f6d8b0)
//!    in slots 0/1 (destructor), 6, 7, 12 and 245 (`SetGrCollisionMode`). The hydrant's also fills
//!    slot 18 with a per-frame re-pin of the collision vertices to model nodes (0x3484fc0), which is
//!    what makes the collision follow a moving weapon; the forge's leaves it empty. That is the
//!    `follow_nodes` choice: hydrant factory for moving weapons, forge factory for static ones.
//! 3. **Node descriptor** at `GroundModule + 0x168`: `{facing_right, facing_left, count}` of 16-byte
//!    `{vertex index, hash40 node}` entries. The array is chosen by `PostureModule::lr` and each
//!    vertex is written with the node's world position (the hydrant and forge both bind
//!    `virtualgrdcol01`/`virtualgrdcol02`). The vanilla weapons store it from their specializer slot
//!    3; we store it on every [`load`] / [`refresh`]. Until then it is null, which every reader checks.
//! 4. **Generate.** The weapon's specializer slot 8 (`WeaponGenerate3`) looks the file up by name in
//!    the weapon's model directory (hard-coded kind and `c00` for the vanilla weapons), stores the
//!    file path index at `GroundModule + 0x128`, parses it (0x33bfbc0), pins the vertices (0x3493280)
//!    and enables collision through vtable slot 245. That is [`load`], using the clone's own names and
//!    the weapon's costume byte; [`refresh`] is the forge's on-demand re-pin (its slot 14). A plugin
//!    calls them from its own `WeaponGenerate3` replacement installed through `replace_weapon_vtable`.
//!
//! Caveat: 0x33bfbc0 reads the file through the loaded tables, so the lvd must already be resident.
//! The weapon model directory is preloaded with the owner, and the model-map insert (0x17dde90) takes a
//! reference on any `.lvd` found in that directory, so keep the file next to the model it belongs to.

use std::collections::BTreeMap;

use locks::RwLock;
use smashline::{LvdNodeBinding, WeaponModule};

use crate::{
    cloning::weapons::{modules, original_kind_of},
    create_agent::LOWERCASE_WEAPON_CATEGORY_NAMES,
    utils,
};

/// The game's lvd-capable GroundModule factories (row slot 5), see the module docs.
const GROUND_MODULE_FACTORY_FOLLOW_NODES: usize = 0x33ba680; // firehydrant: slot 18 re-pins every frame
const GROUND_MODULE_FACTORY_STATIC: usize = 0x33baa10; // forge: no per-frame re-pin

// Lvd-capable weapon GroundModule layout.
/// u32 file path index of the applied lvd, `NO_FILE` when none.
const GM_LVD_FILE_INDEX: usize = 0x128;
/// Two `{collision handle, line data, shared weak count}` triples (+0x130 and +0x148).
const GM_COLLISIONS: usize = 0x130;
/// `*const LvdNodeDescriptor`.
const GM_NODE_DESCRIPTOR: usize = 0x168;
const NO_FILE: u32 = 0xff_ffff;

const SET_GR_COLLISION_MODE_SLOT: usize = 245;
/// Any GroundModule whose slot 6 is the lvd reset (0x33bf690) is an lvd-capable one.
const LVD_MARKER_SLOT: (usize, usize) = (6, 0x33bf690);

/// Weapon object fields.
const WEAPON_KIND: usize = 0xc;
const WEAPON_MODULE_ACCESSOR: usize = 0x20;
/// Resolved costume byte, written by the weapon model setup (0x33a0490) before anything else runs.
const WEAPON_COSTUME: usize = 0x9a;
const BOMA_GROUND_MODULE: usize = 0x58;

/// Parses the lvd at `GroundModule + 0x128` and creates its (one or two) collisions.
#[skyline::from_offset(0x33bfbc0)]
fn ground_module_apply_lvd(module: *mut u8);

/// Writes each bound node's world position into its collision vertex and refreshes the segments.
#[skyline::from_offset(0x3493280)]
fn ground_module_pin_nodes(module: *mut u8, flag: u64);

/// Marks both collision handles active again (when `GroundModule + 0x163` is set); the hydrant runs
/// it every frame and the forge before an on-demand re-pin.
#[skyline::from_offset(0x33bfec0)]
fn ground_module_wake_collisions(module: *mut u8);

#[skyline::from_offset(0x3540a00)]
fn loaded_tables_add_ref(tables: *mut u8, file_path_index: u32);

#[skyline::from_offset(0x3540b10)]
fn loaded_tables_release(tables: *mut u8, file_path_index: u32);

fn text() -> usize {
    unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize }
}

fn loaded_tables() -> *mut u8 {
    unsafe { *((text() + 0x5332f20) as *const *mut u8) }
}

/// Mirrors the game's `{right array, left array, count}` block stored at `GroundModule + 0x168`.
#[repr(C)]
struct LvdNodeDescriptor {
    facing_right: *const LvdNodeBinding,
    facing_left: *const LvdNodeBinding,
    count: u64,
}

/// Which vertices follow which nodes, and whether that happens every frame.
pub struct WeaponLvd {
    /// Re-pin the vertices to their nodes every frame (needed when the weapon moves or rotates).
    pub follow_nodes: bool,
    /// Leaked on purpose: live GroundModules keep pointing at it after a re-registration.
    descriptor: &'static LvdNodeDescriptor,
}

impl WeaponLvd {
    pub fn new(facing_right: &[LvdNodeBinding], facing_left: &[LvdNodeBinding], follow_nodes: bool) -> Self {
        // The game uses one count for both arrays.
        let facing_left = if facing_left.is_empty() { facing_right } else { facing_left };
        let count = facing_right.len().min(facing_left.len());
        if facing_right.len() != facing_left.len() {
            println!(
                "[smashline::lvd] facing-right and facing-left bindings differ in length, using the first {}",
                count
            );
        }
        let facing_right: &'static [LvdNodeBinding] = Box::leak(facing_right[..count].to_vec().into_boxed_slice());
        let facing_left: &'static [LvdNodeBinding] = Box::leak(facing_left[..count].to_vec().into_boxed_slice());
        let descriptor = Box::leak(Box::new(LvdNodeDescriptor {
            facing_right: facing_right.as_ptr(),
            facing_left: facing_left.as_ptr(),
            count: count as u64,
        }));
        Self { follow_nodes, descriptor }
    }

    fn ground_module_factory(&self) -> usize {
        if self.follow_nodes {
            GROUND_MODULE_FACTORY_FOLLOW_NODES
        } else {
            GROUND_MODULE_FACTORY_STATIC
        }
    }
}

unsafe impl Send for WeaponLvd {}
unsafe impl Sync for WeaponLvd {}

pub static WEAPON_LVDS: RwLock<BTreeMap<i32, WeaponLvd>> = RwLock::new(BTreeMap::new());

/// Gives `weapon_kind` the hydrant's or forge's GroundModule (through the module factory row) and
/// remembers the node bindings for [`load`]. Takes effect on the kind's next weapon init.
pub fn register(weapon_kind: i32, lvd: WeaponLvd) {
    println!(
        "[smashline::lvd] Weapon Kind {:#x} gets an lvd GroundModule ({} node bindings, follow nodes: {})",
        weapon_kind,
        lvd.descriptor.count,
        lvd.follow_nodes
    );
    modules::set_factory(weapon_kind, WeaponModule::Ground as usize, lvd.ground_module_factory());
    WEAPON_LVDS.write().insert(weapon_kind, lvd);
}

unsafe fn is_lvd_ground_module(module: *const u8) -> bool {
    if module.is_null() {
        return false;
    }
    let vptr = *(module as *const *const usize);
    *vptr.add(LVD_MARKER_SLOT.0) == text() + LVD_MARKER_SLOT.1
}

/// `weapon`'s lvd-capable GroundModule with the registered node descriptor installed, or `None`
/// with a log line when the kind was not registered before the weapon was created.
unsafe fn lvd_ground_module(weapon: *mut u8, lvd: &WeaponLvd) -> Option<*mut u8> {
    let boma = *(weapon.add(WEAPON_MODULE_ACCESSOR) as *const *mut u8);
    let module = *(boma.add(BOMA_GROUND_MODULE) as *const *mut u8);
    if !is_lvd_ground_module(module) {
        println!(
            "[smashline::lvd] Weapon Kind {:#x} has no lvd-capable GroundModule, register its nodes before the weapon is created",
            *(weapon.add(WEAPON_KIND) as *const i32)
        );
        return None;
    }
    *(module.add(GM_NODE_DESCRIPTOR) as *mut *const LvdNodeDescriptor) = lvd.descriptor;
    Some(module)
}

fn lvd_file_path_index(kind: i32, costume: u8, file_name: &str) -> Option<u32> {
    let category = LOWERCASE_WEAPON_CATEGORY_NAMES.get(original_kind_of(kind).unwrap_or(kind) as usize)?;
    let owner = utils::get_weapon_owner_name(kind)?;
    let article = utils::get_weapon_name(kind)?;
    let dir = format!("{category}/{owner}/model/{article}");

    // Vanilla always reads c00; we prefer the weapon's costume and fall back to it.
    [costume, 0]
        .into_iter()
        .find_map(|costume| utils::get_file_path_index(&format!("{dir}/c{costume:02}/{file_name}")))
}

/// What the hydrant's `WeaponGenerate3` does with its lvd, for any weapon: resolves `file_name` in
/// the weapon's model directory (its costume, then c00), makes the GroundModule own one reference on
/// the file, parses it into collisions, pins the vertices to the registered nodes and enables the
/// collision. Calling it again on a live weapon releases the previous file and rebuilds.
///
/// Returns `false`, with the reason logged, when nothing was applied.
pub unsafe fn load(weapon: *mut u8, file_name: &str) -> bool {
    let kind = *(weapon.add(WEAPON_KIND) as *const i32);
    let lvds = WEAPON_LVDS.read();
    let Some(lvd) = lvds.get(&kind) else {
        println!("[smashline::lvd] Weapon Kind {:#x} has no registered lvd nodes", kind);
        return false;
    };
    let Some(module) = lvd_ground_module(weapon, lvd) else {
        return false;
    };

    let costume = *weapon.add(WEAPON_COSTUME);
    let Some(file_index) = lvd_file_path_index(kind, costume, file_name) else {
        println!(
            "[smashline::lvd] {} not found for Weapon Kind {:#x} (costume {})",
            file_name, kind, costume
        );
        return false;
    };

    // Same reference juggling as the hydrant: the module owns one reference on the applied file.
    let tables = loaded_tables();
    loaded_tables_add_ref(tables, file_index);
    let previous = std::mem::replace(&mut *(module.add(GM_LVD_FILE_INDEX) as *mut u32), file_index);
    if previous != NO_FILE {
        loaded_tables_release(tables, previous);
    }

    ground_module_apply_lvd(module);
    if *(module.add(GM_COLLISIONS) as *const usize) == 0 {
        println!(
            "[smashline::lvd] {} produced no collision for Weapon Kind {:#x} (it needs one or two collision shapes)",
            file_name, kind
        );
        return false;
    }

    ground_module_pin_nodes(module, 1);

    let vptr = *(module as *const *const usize);
    let set_gr_collision_mode: extern "C" fn(*mut u8, bool) =
        std::mem::transmute(*vptr.add(SET_GR_COLLISION_MODE_SLOT));
    set_gr_collision_mode(module, true);
    true
}

/// Re-pins the vertices to their nodes once, what the forge does from its own vtable when the table
/// is moved. Weapons registered with `follow_nodes` do this every frame already.
pub unsafe fn refresh(weapon: *mut u8) {
    let kind = *(weapon.add(WEAPON_KIND) as *const i32);
    let lvds = WEAPON_LVDS.read();
    let Some(lvd) = lvds.get(&kind) else {
        return;
    };
    let Some(module) = lvd_ground_module(weapon, lvd) else {
        return;
    };
    if *(module.add(GM_LVD_FILE_INDEX) as *const u32) == NO_FILE {
        return;
    }
    ground_module_wake_collisions(module);
    ground_module_pin_nodes(module, 1);
}
