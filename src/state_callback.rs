use locks::RwLock;
use skyline::hooks::InlineCtx;
use smash::app::BattleObject;
use smashline::{BattleObjectCategory, Hash40, L2CFighterBase, ObjectEvent};

pub type StateCallbackFunction = unsafe extern "C" fn(&mut L2CFighterBase);

pub struct StateCallback {
    pub agent: Option<Hash40>,
    pub event: ObjectEvent,
    pub function: StateCallbackFunction,
}

pub static STATE_CALLBACKS: RwLock<Vec<StateCallback>> = RwLock::new(Vec::new());

fn call_state_callback(agent: &mut L2CFighterBase, event: ObjectEvent) {
    let hash = crate::create_agent::agent_hash(agent);
    let callbacks = STATE_CALLBACKS.read();

    for callback in callbacks.iter().filter(|cb| cb.event == event) {
        if let Some(required) = callback.agent {
            if hash != required {
                let object: &mut BattleObject = unsafe {std::mem::transmute(agent.battle_object)};
                if let Some(category) = BattleObjectCategory::from_battle_object_id(object.battle_object_id) {
                    match category {
                        BattleObjectCategory::Fighter => if required != Hash40::new("fighter") { continue; },
                        BattleObjectCategory::Weapon => if required != Hash40::new("weapon") { continue; },
                        _ => { continue; }
                    }
                }
                else {
                    continue;
                }
            }
        }

        unsafe {
            (callback.function)(agent);
        }
    }
}

pub static mut CURRENT_AGENT_BASE : u64 = 0;

#[skyline::hook(offset = 0x48ad04, inline)]
unsafe fn lua_module_start_lua2cpp(ctx: &InlineCtx) {
    let module = *ctx.registers[19].x.as_ref() as *const u64;
    CURRENT_AGENT_BASE = *module.add(0x1d8 / 8);
}

#[skyline::hook(offset = 0x48ada0)]
unsafe fn lua_module_end(lua_module: *const u64) {
    let agent = std::mem::transmute(*lua_module.add(0x1d8 / 8));
    call_state_callback(agent, ObjectEvent::End);
}

#[skyline::hook(offset = 0x48abac, inline)]
unsafe fn lua_module_initialize_lua2cpp(ctx: &InlineCtx) {
    let agent = std::mem::transmute(*ctx.registers[0].x.as_ref());
    call_state_callback(agent, ObjectEvent::Initialize);
}

#[skyline::hook(offset = 0x48ac44, inline)]
unsafe fn lua_module_finalize_lua2cpp(ctx: &InlineCtx) {
    let agent = std::mem::transmute(*ctx.registers[0].x.as_ref());
    call_state_callback(agent, ObjectEvent::Finalize);
}

#[skyline::hook(offset = 0x3afdf4, inline)]
unsafe fn start_module_accessor_end(ctx: &InlineCtx) {
    let agent = std::mem::transmute(CURRENT_AGENT_BASE);
    call_state_callback(agent, ObjectEvent::Start);
}

pub fn install_state_callback_hooks() {
    skyline::install_hooks!(
        lua_module_start_lua2cpp,
        lua_module_end,
        lua_module_initialize_lua2cpp,
        lua_module_finalize_lua2cpp,

        start_module_accessor_end,
    );
}
