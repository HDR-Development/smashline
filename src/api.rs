use std::{
    num::{NonZeroU64, NonZeroUsize},
    ptr::NonNull,
};

use acmd_engine::action::ActionRegistry;
use rtld::Section;
use smashline::{
    Acmd, AcmdFunction, Hash40, L2CAgentBase, ObjectEvent, Priority, StatusLine, StringFFI,
};

use crate::{
    callbacks::{StatusCallback, StatusCallbackFunction},
    cloning::weapons::{NewAgent, NewArticle},
    create_agent::{
        AcmdScript, StatusScript, StatusScriptFunction, LOWERCASE_FIGHTER_NAMES,
        LOWERCASE_WEAPON_NAMES,
    },
    state_callback::{StateCallback, StateCallbackFunction},
};

#[no_mangle]
pub extern "C" fn smashline_remove_by_plugin_range(start: usize, end: usize) {
    crate::create_agent::ACMD_SCRIPTS_DEV
        .write()
        .clear();

    crate::create_agent::STATUS_SCRIPTS_DEV
        .write()
        .clear();

    {
        let mut callbacks = crate::callbacks::CALLBACKS.write();

        let working = std::mem::take(&mut *callbacks);
        *callbacks = working
            .into_iter()
            .filter(|cb| !(start..end).contains(&cb.function.as_address()))
            .collect();
    }

    {
        let mut callbacks = crate::state_callback::STATE_CALLBACKS.write();
        let working = std::mem::take(&mut *callbacks);
        *callbacks = working
            .into_iter()
            .filter(|cb| !(start..end).contains(&(cb.function as *const () as usize)))
            .collect();
    }
}

#[no_mangle]
pub extern "C" fn smashline_install_acmd_script(
    agent: Hash40,
    script: Hash40,
    category: Acmd,
    priority: Priority,
    function: unsafe extern "C" fn(&mut L2CAgentBase),
) {
    if unsafe { crate::runtime_reload::LOADING_DEVELOPMENT_SCRIPTS } {
        crate::create_agent::ACMD_SCRIPTS_DEV
            .write()
            .entry(agent)
            .or_default()
            .set_script(script, category, AcmdScript { function, priority });
        return;
    }
    crate::create_agent::ACMD_SCRIPTS
        .write()
        .entry(agent)
        .or_default()
        .set_script(script, category, AcmdScript { function, priority });
}

#[no_mangle]
pub extern "C" fn smashline_install_status_script(
    agent: Option<NonZeroU64>,
    status: i32,
    line: StatusLine,
    function: *const (),
) {
    let agent = agent
        .map(|x| Hash40(x.get()))
        .unwrap_or(Hash40::new("common"));

    if unsafe { crate::runtime_reload::LOADING_DEVELOPMENT_SCRIPTS } {
        crate::create_agent::STATUS_SCRIPTS_DEV
            .write()
            .entry(agent)
            .or_default()
            .push(StatusScript {
                id: status,
                function: StatusScriptFunction::from_line(line, function),
            });
        return;
    }
    crate::create_agent::STATUS_SCRIPTS
        .write()
        .entry(agent)
        .or_default()
        .push(StatusScript {
            id: status,
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
pub extern "C" fn smashline_get_target_function(
    target: StringFFI,
    offset: u64,
) -> Option<NonZeroUsize> {
    NonZeroUsize::new(
        (rtld::find_module_by_name(target.as_str().unwrap())
            .unwrap()
            .get_address_range(Section::Text)
            .start
            + offset) as usize,
    )
}

#[no_mangle]
pub extern "C" fn smashline_get_original_acmd(
    fighter: &mut L2CAgentBase,
    name: Hash40,
) -> Option<AcmdFunction> {
    let scripts = crate::create_agent::original_scripts(fighter)?;
    scripts.get(&name).copied()
}

#[no_mangle]
pub extern "C" fn smashline_get_original_status(
    fighter: &mut smashline::L2CFighterBase,
    line: StatusLine,
    kind: i32,
) -> Option<NonNull<()>> {
    let scripts = crate::create_agent::original_status(fighter)?;
    scripts
        .get(&(line, kind))
        .copied()
        .and_then(|ptr| NonNull::new(ptr.cast_mut()))
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
pub extern "C" fn smashline_add_param_object(fighter: StringFFI, name: StringFFI) {
    let fighter = fighter.as_str().unwrap();
    let fighter_id = LOWERCASE_FIGHTER_NAMES
        .iter()
        .position(|name| name == fighter)
        .unwrap();

    crate::params::WHITELISTED_PARAMS
        .write()
        .entry(fighter_id as i32)
        .or_default()
        .push(Hash40::new(name.as_str().unwrap()));
}

#[no_mangle]
pub extern "C" fn smashline_get_action_registry() -> &'static ActionRegistry {
    &crate::interpreter::ACTION_REGISTRY
}

#[no_mangle]
pub extern "C" fn smashline_reload_script(
    fighter: StringFFI,
    weapon: StringFFI,
    file_name: StringFFI,
) {
    let fighter = fighter.as_str().unwrap();
    let weapon = weapon.as_str().unwrap();
    let file_name = file_name.as_str().unwrap();
    crate::interpreter::load_single_script(
        fighter,
        (!weapon.is_empty()).then_some(weapon),
        file_name,
    );
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
