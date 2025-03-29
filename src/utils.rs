use std::sync::atomic::Ordering;

use smash::app::BattleObject;
use smashline::{Costume, Hash40};

use crate::{
    cloning::fighters::CURRENT_PLAYER_ID,
    cloning::weapons::{try_get_new_agent, CURRENT_OWNER_KIND, NEW_AGENTS},
    create_agent::{COSTUMES, LOWERCASE_WEAPON_NAMES, LOWERCASE_WEAPON_OWNER_NAMES, LOWERCASE_FIGHTER_NAMES}
};

pub fn get_weapon_name(id: i32) -> Option<String> {
    let current_owner = CURRENT_OWNER_KIND.load(Ordering::Relaxed);
    let agents = NEW_AGENTS.read();
    if let Some(name) = try_get_new_agent(&agents, id, current_owner).map(|agent| agent.new_name.clone()) {
        Some(name)
    } else {
        LOWERCASE_WEAPON_NAMES.get(id as usize).map(|x| x.to_string())
    }
}

pub fn get_weapon_owner_name(id: i32) -> Option<String> {
    let current_owner = CURRENT_OWNER_KIND.load(Ordering::Relaxed);
    let agents = NEW_AGENTS.read();

    if let Some(name) = try_get_new_agent(&agents, id, current_owner).map(|agent| agent.owner_name.clone()) {
        Some(name)
    } else {
        LOWERCASE_WEAPON_OWNER_NAMES.get(id as usize).map(|x| x.to_string())
    }
}

pub fn get_weapon_code_dependency(id: i32) -> Option<i32> {
    let current_owner = CURRENT_OWNER_KIND.load(Ordering::Relaxed);
    let agents = NEW_AGENTS.read();

    try_get_new_agent(&agents, id, current_owner).and_then(|x| x.use_original_code.then_some(x.old_owner_id))
}

pub fn get_costume_from_entry_id(entry_id: i32) -> Option<i32> {
    unsafe {
        const VEC_OFFSET: u64 = 0x5324680;
        let some_vec = skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 + VEC_OFFSET;
        let some_vec = *(some_vec as *const u64);

        let index = entry_id as u64 * 8;
        let some_struct = *((some_vec + index + 0xe8) as *const u64);

        const COSTUME_OFFSET: u64 = 100;
        let ptr = ((some_struct + COSTUME_OFFSET) as *const u64);
        if ptr as u64 == 0x64 { // entry articles
            None
        } else {
            Some(*(ptr as *const i32))
        }
    }
}

pub fn get_agent_costume(battle_object: *const BattleObject, is_weapon: bool) -> Option<i32> {
    unsafe {
        let entry_id = if is_weapon {
            CURRENT_PLAYER_ID.load(Ordering::Relaxed) as i32
        } else {
            (*battle_object).entry_id
        };

        crate::utils::get_costume_from_entry_id(entry_id)
    }
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

    // Step 2: Insert the module or inc ref count
    let module = unsafe {
        let tree = manager.add(0x38 / 8) as *mut smash::cpp::Tree<Hash40, *mut skyline::nn::ro::Module>;

        if let Some(module) = (*tree).get_mut(&Hash40::new(name)) {
            *(*module as *mut u64 as *mut i32).add(0x134 / 4) += 1;
            return;
        }

        load_file(format!("prebuilt:/nro/release/lua2cpp_{name}.nro"));

        let mut module_name = [0u8; 0x100];
        module_name[..name.len()].copy_from_slice(name.as_bytes());

        let module = Box::leak(Box::new(skyline::nn::ro::Module {
            ModuleObject: std::ptr::null_mut(),
            State: 0,
            NroPtr: std::ptr::null_mut(),
            BssPtr: std::ptr::null_mut(),
            _x20: std::ptr::null_mut(),
            SourceBuffer: std::ptr::null_mut(),
            Name: module_name,
            _x130: 0,
            _x131: 0,
            isLoaded: false
        })) as *mut _;

        // Safe bc Module is aligned to 0x8 and so the size is 0x138
        *(module as *mut u32).add(0x134 / 4) = 1;

        (*tree).insert(Hash40::new(name), module);
        module
    };

    // Step 3: Send command to manager that we want to load a module
    unsafe {
        let deque = &mut *(manager.add(0x80 / 8) as *mut MyDeque);
        let distance = if deque.end.offset_from(deque.start) != 0 {
            deque.end.offset_from(deque.start) * 0x100 - 1
        } else {
            0
        };

        let next_index = deque.start_index + deque.len;
        if (deque.start_index + deque.len) as isize == distance {
            extend_deque(deque);
        }

        *(*deque.start.add(next_index / 0x100)).add(next_index & 0xFF) = Command {
            id: 3,
            arg: module as u64
        };

        deque.len += 1;
    }

    // Step 4: Signal event
    unsafe {
        signal_event(**(manager.add(0x50 / 8) as *const *const u64));
    }
}

#[skyline::from_offset(0x22b6f60)]
fn dynamic_module_manager_unload(manager: *mut u64, name: &Hash40);

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
        let tree = manager.add(0x38 / 8) as *const smash::cpp::Tree<Hash40, *mut skyline::nn::ro::Module>;

        (*tree).get(&Hash40::new(name)).map(|x| (**x).isLoaded).unwrap_or_default()
    }
}

#[skyline::from_offset(0x353e5a0)]
fn get_search_path_index(index: &mut u32, bytes: *const u8);

#[skyline::from_offset(0x353e750)]
fn get_file_path_from_search_path(search_path: u32) -> u32;

#[skyline::from_offset(0x35406c0)]
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
