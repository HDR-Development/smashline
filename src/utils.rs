use std::sync::atomic::Ordering;

use smash::app::BattleObject;
use smashline::{Costume, Hash40};

use crate::{
    cloning::fighters::CURRENT_PLAYER_ID,
    cloning::weapons::{try_get_new_agent, NEW_AGENTS},
    create_agent::{COSTUMES, LOWERCASE_WEAPON_NAMES, LOWERCASE_WEAPON_OWNER_NAMES, LOWERCASE_FIGHTER_NAMES}
};

pub fn get_weapon_name(id: i32) -> Option<String> {
    let agents = NEW_AGENTS.read();
    if let Some(name) = try_get_new_agent(&agents, id).map(|agent| agent.article_name.clone()) {
        Some(name)
    } else {
        LOWERCASE_WEAPON_NAMES.get(id as usize).map(|x| x.to_string())
    }
}

pub fn get_weapon_owner_name(id: i32) -> Option<String> {
    let agents = NEW_AGENTS.read();

    if let Some(name) = try_get_new_agent(&agents, id).map(|agent| agent.owner_name.clone()) {
        Some(name)
    } else {
        LOWERCASE_WEAPON_OWNER_NAMES.get(id as usize).map(|x| x.to_string())
    }
}

pub fn get_weapon_code_dependency(id: i32) -> Option<i32> {
    let agents = NEW_AGENTS.read();

    try_get_new_agent(&agents, id).and_then(|x| x.use_original_code.then_some(x.original_owner_id))
}

pub fn get_costume_from_entry_id(entry_id: i32) -> Option<i32> {
    unsafe {
        let text = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *const u64;
        let some_vec = *text.add(0x5324680 / 0x8);

        let index = entry_id as u64 * 8;
        let some_struct = *((some_vec + index + 0xe8) as *const u64);

        const COSTUME_OFFSET: u64 = 100;
        let ptr = (some_struct + COSTUME_OFFSET) as *const i32;
        if ptr as u64 == 0x64 { // entry articles
            None
        } else {
            Some(*ptr)
        }
    }
}

pub fn get_agent_costume(battle_object: *const BattleObject) -> Option<i32> {
    let entry_id = unsafe { (*battle_object).entry_id };
    if entry_id > 7 || entry_id < 0 {
        return None;
    }

    crate::utils::get_costume_from_entry_id(entry_id)
}

pub fn has_costume(hash: Hash40, costume: i32) -> bool {
    COSTUMES
        .read()
        .get(&hash).map_or(false, |costume_vec| {
            costume_vec.iter().any(|c| {
                c.as_slice().contains(&(costume as usize))
            })
        })
}

pub fn get_costume_data(hash: Hash40, costume: i32) -> Costume {
    let def = Costume::default();
    COSTUMES
        .read()
        .get(&hash).map_or(def, |costume_vec| {
            costume_vec.iter().find(|c| {
                c.as_slice().contains(&(costume as usize))
            }).copied().unwrap_or(def)
        })
}

fn dynamic_module_manager() -> *mut u64 {
    let text = unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *mut u8 };
    unsafe { **text.add(0x5327cd0).cast::<*mut *mut u64>() }
}

#[repr(C)]
struct Command {
    id: u32,
    arg: u64,
}

#[repr(C)]
struct MyDeque {
    start: *mut *mut Command,
    begin: *mut *mut Command,
    end: *mut *mut Command,
    end_cap: *mut *mut Command,
    start_index: usize,
    len: usize,
}

#[skyline::from_offset(0x22b59c0)]
fn extend_deque(deque: *mut MyDeque);

extern "C" {
    #[link_name = "_ZN2nn2os11SignalEventEPNS0_9EventTypeE"]
    fn signal_event(event: u64);

    #[link_name = "_ZNSt3__115recursive_mutex4lockEv"]
    fn recursive_mutex_lock(mutex: *mut u8);

    #[link_name = "_ZNSt3__115recursive_mutex6unlockEv"]
    fn recursive_mutex_unlock(mutex: *mut u8);
}

/// Size of the game's module record: an `nn::ro::Module` followed by the manager's own fields.
const MODULE_ALLOC_SIZE: usize = 0x138;

/// Offset of the `u16` name length the game stores after the name buffer.
const MODULE_NAME_LEN_OFFSET: usize = 0x130;

/// Offset of the `isLoaded` byte, set by the manager's worker thread once `nn::ro::LoadModule` succeeds.
const MODULE_IS_LOADED_OFFSET: usize = 0x132;

/// Offset of the "unload pending" flag byte. The worker thread refuses to load a module whose flag
/// is set, so it must be explicitly cleared on a freshly created record.
const MODULE_UNLOAD_PENDING_OFFSET: usize = 0x133;

/// Offset of the manager's reference count for the module.
const MODULE_REFCOUNT_OFFSET: usize = 0x134;

/// The manager's `std::recursive_mutex`, which guards both the module tree and the command deque.
const MANAGER_MUTEX_OFFSET: usize = 0x60;

type ModuleTree = smash::cpp::Tree<Hash40, *mut skyline::nn::ro::Module>;

struct ManagerLock(*mut u8);

impl ManagerLock {
    unsafe fn acquire(manager: *mut u64) -> Self {
        let mutex = (manager as *mut u8).add(MANAGER_MUTEX_OFFSET);
        recursive_mutex_lock(mutex);
        Self(mutex)
    }
}

impl Drop for ManagerLock {
    fn drop(&mut self) {
        unsafe { recursive_mutex_unlock(self.0) };
    }
}

/// Allocates and initializes a module record the same way the game's inlined loader does.
///
/// The record is zero-initialized so that every field the worker thread inspects (most importantly
/// the unload-pending flag, which lives in what Rust considers padding of `nn::ro::Module`) has a
/// defined value.
unsafe fn new_module_record(name: &str) -> *mut skyline::nn::ro::Module {
    // Same alignment the game requests for this record. The worker thread frees it with the game's
    // own deallocator, which shares the jemalloc heap with skyline's global allocator.
    let layout = std::alloc::Layout::from_size_align(MODULE_ALLOC_SIZE, 0x10).unwrap();
    let record = std::alloc::alloc_zeroed(layout);
    if record.is_null() {
        std::alloc::handle_alloc_error(layout);
    }

    let name_bytes = name.as_bytes();
    let module = record as *mut skyline::nn::ro::Module;
    let name_dst = std::ptr::addr_of_mut!((*module).Name).cast::<u8>();
    std::ptr::copy_nonoverlapping(name_bytes.as_ptr(), name_dst, name_bytes.len().min(0xFF));
    record
        .add(MODULE_NAME_LEN_OFFSET)
        .cast::<u16>()
        .write_unaligned(name_bytes.len() as u16);
    *record.add(MODULE_IS_LOADED_OFFSET) = 0;
    *record.add(MODULE_UNLOAD_PENDING_OFFSET) = 0;
    record
        .add(MODULE_REFCOUNT_OFFSET)
        .cast::<i32>()
        .write_unaligned(1);

    module
}

/// Pushes a command onto the manager's deque and wakes the worker thread.
///
/// Must be called with the manager lock held.
unsafe fn push_manager_command(manager: *mut u64, command: Command) {
    let deque = &mut *(manager.add(0x80 / 8) as *mut MyDeque);

    // Mirrors libc++'s deque::push_back, which measures the map from `begin`, not `start`.
    let capacity = if deque.end != deque.begin {
        deque.end.offset_from(deque.begin) as usize * 0x100 - 1
    } else {
        0
    };

    if deque.start_index + deque.len == capacity {
        extend_deque(deque);
    }

    let next_index = deque.start_index + deque.len;
    *(*deque.begin.add(next_index / 0x100)).add(next_index & 0xFF) = command;
    deque.len += 1;

    let event = **(manager.add(0x50 / 8) as *const *const u64);
    if event != 0 {
        signal_event(event);
    }
}

/// Loads a fighter module via the game's internal methods
pub fn load_fighter_module(kind: i32) {
    if kind < 0 {
        return;
    }

    // Step 1: Get manager
    let manager = dynamic_module_manager();

    // Step 2: Get fighter name
    let Some(name) = LOWERCASE_FIGHTER_NAMES.get(kind as usize) else {
        return;
    };

    unsafe {
        let _lock = ManagerLock::acquire(manager);

        // Step 3: Insert the module or inc ref count
        let tree = manager.add(0x38 / 8) as *mut ModuleTree;

        if let Some(module) = (*tree).get_mut(&Hash40::new(name)) {
            let refcount = (*module as *mut u8).add(MODULE_REFCOUNT_OFFSET).cast::<i32>();
            refcount.write_unaligned(refcount.read_unaligned() + 1);
            return;
        }

        let module = new_module_record(name);
        (*tree).insert(Hash40::new(name), module);

        // Step 4: Send command to manager that we want to load a module. The worker thread
        // resolves and loads the NRO file itself.
        push_manager_command(
            manager,
            Command {
                id: 3,
                arg: module as u64,
            },
        );
    }
}

#[skyline::from_offset(0x22b6f60)]
fn dynamic_module_manager_unload(manager: *mut u64, name: &Hash40);

#[skyline::hook(offset = 0x17e4900)]
fn load_fighter_code_module_hook(loader: *mut u8, kind: i32) -> *mut u8 {
    let result = call_original!(loader, kind);

    if kind >= 0 {
        for dependency in crate::cloning::weapons::code_dependencies_of(kind) {
            println!("[smashline::modules] Fighter {:#x} borrows code from fighter {:#x}, loading its module", kind, dependency);
            load_fighter_module(dependency);
        }
    }

    result
}

#[skyline::hook(offset = 0x22b6f60)]
fn dynamic_module_manager_unload_hook(manager: *mut u64, name: &Hash40) -> u64 {
    let name = *name;
    let result = call_original!(manager, &name);

    let dependents: Vec<i32> = crate::cloning::weapons::NEW_AGENTS
        .read()
        .iter()
        .filter(|agent| agent.use_original_code && agent.original_owner_id != agent.owner_id)
        .map(|agent| agent.owner_id)
        .collect();

    for owner in dependents {
        let Some(owner_name) = LOWERCASE_FIGHTER_NAMES.get(owner as usize) else {
            continue;
        };
        if Hash40::new(owner_name) != name {
            continue;
        }
        for dependency in crate::cloning::weapons::code_dependencies_of(owner) {
            println!("[smashline::modules] Fighter {:#x} unloaded, releasing borrowed module of fighter {:#x}", owner, dependency);
            unload_fighter_module(dependency);
        }
        break;
    }

    result
}

pub fn install_module_hooks() {
    skyline::install_hooks!(load_fighter_code_module_hook, dynamic_module_manager_unload_hook);
}

pub fn unload_fighter_module(id: i32) {
    if id < 0 {
        return;
    }

    let Some(name) = LOWERCASE_FIGHTER_NAMES.get(id as usize) else {
        return;
    };

    unsafe {
        dynamic_module_manager_unload(dynamic_module_manager(), &Hash40::new(name));
    }
}

pub fn is_fighter_module_loaded(id: i32) -> bool {
    if id < 0 {
        return false;
    }

    let Some(name) = LOWERCASE_FIGHTER_NAMES.get(id as usize) else {
        return false;
    };

    let manager = dynamic_module_manager();
    unsafe {
        let _lock = ManagerLock::acquire(manager);
        let tree = manager.add(0x38 / 8) as *const ModuleTree;

        (*tree)
            .get(&Hash40::new(name))
            .map(|module| *(*module as *const u8).add(MODULE_IS_LOADED_OFFSET) & 1 != 0)
            .unwrap_or_default()
    }
}

#[skyline::from_offset(0x353e330 + 0x5B0)]
fn get_search_path_index(index: &mut u32, bytes: *const u8);

#[skyline::from_offset(0x353e4e0 + 0x5B0)]
fn get_file_path_from_search_path(search_path: u32) -> u32;

#[skyline::from_offset(0x3540450 + 0x5B0)]
fn add_to_res_service(filesystem: *mut u64, file_path: u32);

fn get_filesystem() -> *mut u64 {
    let text = unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as *mut u8 };
    unsafe { *text.add(0x5332f20).cast::<*mut u64>() }
}

pub fn load_file(name: impl Into<String>) {
    let mut search_path = 0u32;
    unsafe {
        get_search_path_index(&mut search_path, format!("{}\0", name.into()).as_ptr());
    }

    let file_path = unsafe { get_file_path_from_search_path(search_path) };
    if file_path != 0xFFFFFF {
        unsafe {
            add_to_res_service(get_filesystem(), file_path);
        }
    }
}
