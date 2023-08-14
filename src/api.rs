use std::num::NonZeroU64;

use rtld::Section;
use smashline::{
    Acmd, Hash40, L2CAgentBase, LuaConst, ObjectEvent, Priority, StatusLine, Variadic,
};

use crate::{
    callbacks::{StatusCallback, StatusCallbackFunction},
    create_agent::{AcmdScript, StatusScript, StatusScriptFunction, StatusScriptId},
    state_callback::{StateCallback, StateCallbackFunction}, unwind::{MEMORY_REGIONS, MemoryRegionSearchKey},
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

    println!("{:#x?}", region);
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
