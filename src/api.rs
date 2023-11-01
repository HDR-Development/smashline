use std::num::NonZeroU64;

use rtld::Section;
use smashline::{
    Acmd, Hash40, L2CAgentBase, LuaConst, ObjectEvent, Priority, StatusLine, StringFFI, Variadic,
};

use crate::{
    callbacks::{StatusCallback, StatusCallbackFunction},
    cloning::{
        fighters::NewFighter,
        weapons::{NewAgent, NewArticle},
    },
    create_agent::{
        AcmdScript, StatusScript, StatusScriptFunction, StatusScriptId, LOWERCASE_FIGHTER_NAMES,
        LOWERCASE_WEAPON_NAMES,
    },
    state_callback::{StateCallback, StateCallbackFunction},
    unwind::{MemoryRegionSearchKey, MEMORY_REGIONS},
};

#[no_mangle]
pub extern "C" fn smashline_install_acmd_script(
    agent: Hash40,
    script: Hash40,
    category: Acmd,
    priority: Priority,
    function: extern "C" fn(&mut L2CAgentBase, &mut Variadic),
) {
    crate::create_agent::ACMD_SCRIPTS
        .write()
        .entry(agent)
        .or_default()
        .set_script(script, category, AcmdScript { function, priority });

    let Some(module) = rtld::find_module_for_address(function as u64, Section::Text) else {
        panic!();
    };

    let Some(region) = module.get_symbol_range_for_address(function as u64) else {
        panic!();
    };

    let mut current = region.start;
    let mut landing_pad = None;
    while current <= region.end {
        unsafe {
            let inst = *(current as *const u32);
            if inst == 0xB000B1E5 {
                landing_pad = Some(current);
                break;
            }
        }

        current += 4;
    }

    let landing_pad = landing_pad.unwrap();

    let mut memory = MEMORY_REGIONS.write();
    memory.insert(MemoryRegionSearchKey::Region(region), landing_pad);
}

#[no_mangle]
pub extern "C" fn smashline_install_status_script(
    agent: Option<NonZeroU64>,
    status: LuaConst,
    line: StatusLine,
    function: *const (),
    original: &'static locks::RwLock<*const ()>,
) {
    let agent = agent
        .map(|x| Hash40(x.get()))
        .unwrap_or(Hash40::new("common"));

    crate::create_agent::STATUS_SCRIPTS
        .write()
        .entry(agent)
        .or_default()
        .push(StatusScript {
            id: StatusScriptId::Replace {
                id: status,
                original,
            },
            function: StatusScriptFunction::from_line(line, function),
        });
}

#[no_mangle]
pub extern "C" fn smashline_install_new_status_script(
    agent: Hash40,
    id: i32,
    line: StatusLine,
    function: *const (),
) {
    crate::create_agent::STATUS_SCRIPTS
        .write()
        .entry(agent)
        .or_default()
        .push(StatusScript {
            id: StatusScriptId::New(id),
            function: StatusScriptFunction::from_line(line, function),
        });
}

#[no_mangle]
pub extern "C" fn smashline_install_line_callback(
    agent: Option<NonZeroU64>,
    line: StatusLine,
    function: *const (),
) {
    let agent = agent.map(|value| Hash40(value.get()));

    crate::callbacks::CALLBACKS.write().push(StatusCallback {
        hash: agent,
        function: StatusCallbackFunction::new(line, function),
    });
}

#[no_mangle]
pub extern "C" fn smashline_install_state_callback(
    agent: Option<NonZeroU64>,
    event: ObjectEvent,
    function: StateCallbackFunction,
) {
    crate::state_callback::STATE_CALLBACKS
        .write()
        .push(StateCallback {
            agent: agent.map(|x| Hash40(x.get())),
            event,
            function,
        });
}

#[no_mangle]
pub extern "C" fn smashline_clone_fighter(original_fighter: StringFFI, new_fighter: StringFFI) {
    let original = original_fighter.as_str().unwrap();
    let new = new_fighter.as_str().unwrap();

    let base_id = LOWERCASE_FIGHTER_NAMES
        .iter()
        .position(|name| name == original)
        .unwrap();

    crate::cloning::fighters::NEW_FIGHTERS
        .write()
        .push(NewFighter {
            base_id: base_id as i32,
            fighter_kind_hash: Hash40::new(&format!("fighter_kind_{new}")),
            name: new.to_string(),
            name_ffi: format!("{new}\0"),
            hash: Hash40::new(new),
        });
}

#[no_mangle]
pub extern "C" fn smashline_clone_weapon(
    original_owner: StringFFI,
    original_name: StringFFI,
    new_owner: StringFFI,
    new_name: StringFFI,
    use_original_code: bool,
) {
    let original_owner = original_owner.as_str().unwrap().to_string();
    let original_name = original_name.as_str().unwrap().to_string();
    let new_owner = new_owner.as_str().unwrap().to_string();
    let new_name = new_name.as_str().unwrap().to_string();

    let original_owner_id = LOWERCASE_FIGHTER_NAMES
        .iter()
        .position(|name| name == original_owner)
        .unwrap();

    let original_name_id = LOWERCASE_WEAPON_NAMES
        .iter()
        .position(|name| name == original_name)
        .unwrap();

    let new_owner_id = LOWERCASE_FIGHTER_NAMES
        .iter()
        .position(|name| name == new_owner)
        .unwrap();

    crate::cloning::weapons::NEW_AGENTS
        .write()
        .entry(original_name_id as i32)
        .or_default()
        .push(NewAgent {
            old_owner_id: original_owner_id as i32,
            owner_id: new_owner_id as i32,
            owner_name_ffi: format!("{new_owner}\0"),
            new_name_ffi: format!("{new_name}\0"),
            owner_name: new_owner,
            new_name,
            old_name: original_name,
            use_original_code,
        });

    crate::cloning::weapons::NEW_ARTICLES
        .write()
        .entry(new_owner_id as i32)
        .or_default()
        .push(NewArticle {
            original_owner: original_owner_id as i32,
            weapon_id: original_name_id as i32,
        });
}
