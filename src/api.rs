use std::{
    num::{NonZeroU64, NonZeroUsize},
    ptr::NonNull,
};

use acmd_engine::action::ActionRegistry;
use rtld::Section;
use smashline::{
    Acmd, AcmdFunction, AgentEntry, Costume, Hash40, L2CAgentBase, ObjectEvent, Priority, StatusLine, StringFFI,
};

use crate::{
    callbacks::{StatusCallback, StatusCallbackFunction}, cloning::weapons::{NewAgent, NewArticle, VANILLA_WEAPON_COUNT}, create_agent::{
        AcmdScript, LOWERCASE_FIGHTER_NAMES, LOWERCASE_WEAPON_NAMES, StatusScript, StatusScriptFunction
    }, state_callback::{StateCallback, StateCallbackFunction},
};

fn mark_costume(
    agent: Hash40,
    costume: Costume,
) {
    let costume_slice = costume.as_slice();

    if costume_slice.is_empty() {
        return;
    }

    let mut costumes = crate::create_agent::COSTUMES.write();
    let costumes = costumes
        .entry(agent)
        .or_default();

    for c in &mut *costumes {
        if costume_slice == c.as_slice() {
            return;
        }

        let exists = costume_slice.iter().any(|e| {
            c.as_slice().contains(e)
        });

        if exists {
            // TODO: Do something if a costume has already been marked for this agent
        }
    }

    costumes.push(costume);
}

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
pub extern "C" fn smashline_install_acmd_script_costume(
    agent: Hash40,
    costume: Costume,
    script: Hash40,
    category: Acmd,
    priority: Priority,
    function: unsafe extern "C" fn(&mut L2CAgentBase),
) {
    mark_costume(agent, costume);

    if unsafe { crate::runtime_reload::LOADING_DEVELOPMENT_SCRIPTS } {
        crate::create_agent::ACMD_SCRIPTS_DEV
            .write()
            .entry(AgentEntry::new(agent.0, costume))
            .or_default()
            .set_script(script, category, AcmdScript { function, priority, costume });
        return;
    }
    crate::create_agent::ACMD_SCRIPTS
        .write()
        .entry(AgentEntry::new(agent.0, costume))
        .or_default()
        .set_script(script, category, AcmdScript { function, priority, costume });
}

#[no_mangle]
pub extern "C" fn smashline_install_acmd_script(
    agent: Hash40,
    script: Hash40,
    category: Acmd,
    priority: Priority,
    function: unsafe extern "C" fn(&mut L2CAgentBase),
) {
    smashline_install_acmd_script_costume(agent, Costume::default(), script, category, priority, function);
}

#[no_mangle]
pub extern "C" fn smashline_install_status_script_costume(
    agent: Option<NonZeroU64>,
    costume: Costume,
    status: i32,
    line: StatusLine,
    function: *const (),
) {
    let agent = agent
        .map(|x| Hash40(x.get()))
        .unwrap_or(Hash40::new("common"));

    mark_costume(agent, costume);

    if unsafe { crate::runtime_reload::LOADING_DEVELOPMENT_SCRIPTS } {
        crate::create_agent::STATUS_SCRIPTS_DEV
            .write()
            .entry(agent)
            .or_default()
            .push(StatusScript {
                id: status,
                function: StatusScriptFunction::from_line(line, function),
                costume
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
            costume
        });
}

#[no_mangle]
pub extern "C" fn smashline_install_status_script(
    agent: Option<NonZeroU64>,
    status: i32,
    line: StatusLine,
    function: *const (),
) {
    smashline_install_status_script_costume(agent, Costume::default(), status, line, function);
}

#[no_mangle]
pub extern "C" fn smashline_install_line_callback_costume(
    agent: Option<NonZeroU64>,
    costume: Costume,
    line: StatusLine,
    function: *const (),
) {
    let agent = agent.map(|value| Hash40(value.get()));

    if agent != Some(Hash40::new("fighter"))
    && agent != Some(Hash40::new("weapon")) {
        mark_costume(agent.unwrap(), costume);
    }

    crate::callbacks::CALLBACKS.write().push(StatusCallback {
        hash: agent,
        function: StatusCallbackFunction::new(line, function),
        costume,
    });
}

#[no_mangle]
pub extern "C" fn smashline_install_line_callback(
    agent: Option<NonZeroU64>,
    line: StatusLine,
    function: *const (),
) {
    smashline_install_line_callback_costume(agent, Costume::default(), line, function);
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
pub extern "C" fn smashline_install_state_callback_costume(
    agent: Option<NonZeroU64>,
    costume: Costume,
    event: ObjectEvent,
    function: StateCallbackFunction,
) {
    let agent = agent.map(|value| Hash40(value.get()));

    if agent != Some(Hash40::new("fighter"))
    && agent != Some(Hash40::new("weapon")) {
        mark_costume(agent.unwrap(), costume);
    }

    crate::state_callback::STATE_CALLBACKS
        .write()
        .push(StateCallback {
            agent,
            event,
            function,
            costume,
        });
}

#[no_mangle]
pub extern "C" fn smashline_install_state_callback(
    agent: Option<NonZeroU64>,
    event: ObjectEvent,
    function: StateCallbackFunction,
) {
    smashline_install_state_callback_costume(agent, Costume::default(), event, function);
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
    owner_name: StringFFI,
    article_name: StringFFI,
    original_article_id: i32,
    use_original_code: bool,
) -> i32 {
    let owner_name = owner_name.as_str().unwrap().to_string();
    let article_name = article_name.as_str().unwrap().to_string();
    println!("[smashline::cloning] New weapon {}_{} is being cloned!", owner_name, article_name);

    let owner_id = LOWERCASE_FIGHTER_NAMES
        .iter()
        .position(|name| name == owner_name)
        .unwrap() as i32;
    println!("[smashline::cloning] Owner ID has been found: {:#x}", owner_id);

    let original_owner_name = LOWERCASE_FIGHTER_NAMES.get(original_article_id as usize).unwrap();

    let original_article_name = LOWERCASE_WEAPON_NAMES.get(original_article_id as usize).unwrap();

    let original_owner_id = LOWERCASE_FIGHTER_NAMES
        .iter()
        .position(|name| name == original_owner_name)
        .unwrap() as i32;
    println!("[smashline::cloning] The original article being cloned is {}_{}", original_owner_name, original_article_name);

    let mut new_agents = crate::cloning::weapons::NEW_AGENTS.write();

    let mut new_articles = crate::cloning::weapons::NEW_ARTICLES.write();
    let articles = new_articles
        .entry(owner_id as i32)
        .or_default();

    if let Some(_id) = articles.iter().position(|article|
        article.original_owner == original_owner_id as i32 &&
        article.weapon_id == original_article_id
    ) {
        panic!("[smashline::cloning] This article name is already cloned on this fighter!");
    }

    let new_weapon_count = new_agents.len();
    new_agents.push(
        NewAgent{
            article_id: (VANILLA_WEAPON_COUNT + new_weapon_count) as i32,
            owner_id,
            article_name,
            owner_name,
            original_article_id,
            original_owner_id,
            original_article_name: original_article_name.to_string(),
            use_original_code
        }
    );

    let id = articles.len();
    articles.push(NewArticle {
        original_owner: original_owner_id as i32,
        weapon_id: original_article_id,
    });

    crate::cloning::weapons::invalidate_article_cache();

    id as i32
}

#[no_mangle]
pub extern "C" fn smashline_update_weapon_count(
    article_id: i32,
    new_count: i32
) {
    *crate::cloning::weapons::WEAPON_COUNT_UPDATE
        .write()
        .entry(article_id)
        .or_default() = new_count;

    crate::cloning::weapons::invalidate_article_cache();
}

#[no_mangle]
pub extern "C" fn smashline_whitelist_kirby_copy_article(
    fighter_id: i32,
    article_id: i32
) {
    let mut copy_whitelist = crate::cloning::weapons::KIRBY_COPY_ARTICLE_WHITELIST.write();
    if let Some(whitelist) = copy_whitelist.get_mut(&fighter_id) {
        if whitelist.contains(&article_id) {
            println!("Copy Whitelist already contains fighter {:#x} article {:#x}!", fighter_id, article_id);
        }
        else {
            whitelist.push(article_id);
        }
    }
    else {
        copy_whitelist.insert(fighter_id, vec![article_id]);
    }

    crate::cloning::weapons::invalidate_article_cache();
}
