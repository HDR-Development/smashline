use std::num::NonZeroU64;

use smashline::{Acmd, Hash40, L2CAgentBase, LuaConst, Priority, StatusLine, Variadic};

use crate::{
    callbacks::{StatusCallback, StatusCallbackFunction},
    create_agent::{AcmdScript, StatusScript, StatusScriptFunction, StatusScriptId},
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
}

#[no_mangle]
pub extern "C" fn smashline_install_status_script(
    agent: Hash40,
    status: LuaConst,
    line: StatusLine,
    function: *const (),
    original: &'static locks::RwLock<*const ()>,
) {
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
