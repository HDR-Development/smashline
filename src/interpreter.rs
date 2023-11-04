use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Weak},
};

use acmd_engine::{action::ActionRegistry, SmashlineScript};
use locks::Mutex;
use skyline::hooks::InlineCtx;
use smashline::{Hash40, L2CAgentBase, Variadic};

use crate::create_agent::UserScript;

extern "C" {
    #[allow(unused)]
    #[link_name = "_ZN3phx5Fiber5setupENSt3__18functionIFPS0_vEEEPKcm"]
    fn setup_fiber(fiber: u64, function: u64, name: *const i8);

    #[link_name = "_ZN3phx5Fiber17get_current_fiberEv"]
    fn get_current_fiber() -> u64;

    fn __smashline_interpreter(
        agent: &mut L2CAgentBase,
        script_name: u64,
        original: extern "C" fn(&mut L2CAgentBase, &mut Variadic),
    );
}

std::arch::global_asm!(include_str!("interpreter.asm"));

pub fn get_scripts(fighter_name: &str, weapon_name: Option<&str>) -> Vec<LoadedScript> {
    let sub_folder = weapon_name.unwrap_or("body");
    let path = PathBuf::from(format!("mods:/fighter/{fighter_name}/acmd/{sub_folder}/"));
    let read_dir = match std::fs::read_dir(&path) {
        Ok(read_dir) => read_dir,
        Err(e) => {
            println!("Failed to get scripts: {e}");
            return vec![];
        }
    };

    let mut scripts = vec![];

    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                println!("Failed to get script entry: {e}");
                continue;
            }
        };

        match entry.file_type() {
            Ok(ty) if ty.is_file() => {}
            _ => continue,
        }

        let Some("acmd") = entry.path().extension().and_then(|s| s.to_str()) else {
            continue;
        };

        let data = match std::fs::read_to_string(entry.path()) {
            Ok(data) => data,
            Err(e) => {
                println!("Failed to read {}: {e}", entry.path().display());
                continue;
            }
        };

        let script = match SmashlineScript::from_json(&ACTION_REGISTRY, data) {
            Ok(data) => data,
            Err(e) => {
                println!("Failed to parse {}: {e}", entry.path().display());
                continue;
            }
        };

        scripts.push(LoadedScript {
            file_name: entry
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            script: Arc::new(locks::RwLock::new(Arc::new(script))),
        });
    }

    scripts
}

pub fn load_single_script(fighter: &str, weapon_name: Option<&str>, file_name: &str) {
    let sub_folder = weapon_name.unwrap_or("body");
    let path = PathBuf::from(format!(
        "mods:/fighter/{fighter}/acmd/{sub_folder}/{file_name}"
    ));
    let data = match std::fs::read_to_string(&path) {
        Ok(data) => data,
        Err(e) => {
            println!("Failed to read {}: {e}", path.display());
            return;
        }
    };

    let new_script = match SmashlineScript::from_json(&ACTION_REGISTRY, data) {
        Ok(data) => data,
        Err(e) => {
            println!("Failed to parse {}: {e}", path.display());
            return;
        }
    };

    let agent = if let Some(weapon) = weapon_name {
        Hash40::new(fighter).concat_str("_").concat_str(weapon)
    } else {
        Hash40::new(fighter)
    };

    let loaded_scripts = LOADED_SCRIPTS.write();
    let Some(loaded) = loaded_scripts.get(&agent).and_then(|weak| weak.upgrade()) else {
        println!("No loaded scripts to insert into");
        return;
    };

    for script in loaded.iter() {
        if script.file_name == file_name {
            *script.script.write() = Arc::new(new_script);
            return;
        }
    }
}

pub fn get_or_load_scripts(fighter: &str, weapon_name: Option<&str>) -> Arc<Vec<LoadedScript>> {
    let agent = if let Some(weapon) = weapon_name {
        Hash40::new(fighter).concat_str("_").concat_str(weapon)
    } else {
        Hash40::new(fighter)
    };

    let mut loaded_scripts = LOADED_SCRIPTS.write();
    if let Some(loaded) = loaded_scripts.get(&agent).and_then(|weak| weak.upgrade()) {
        return loaded;
    }

    let scripts = get_scripts(fighter, weapon_name);
    let scripts = Arc::new(scripts);
    loaded_scripts.insert(agent, Arc::downgrade(&scripts));
    scripts
}

pub struct LoadedScript {
    pub file_name: String,
    pub script: Arc<locks::RwLock<Arc<SmashlineScript>>>,
}

pub static ACTION_REGISTRY: ActionRegistry = ActionRegistry::new();
pub static LOADED_SCRIPTS: locks::RwLock<BTreeMap<Hash40, Weak<Vec<LoadedScript>>>> =
    locks::RwLock::new(BTreeMap::new());

static FIBER_MAP: Mutex<BTreeMap<u64, (u64, u64)>> = Mutex::new(BTreeMap::new());

#[no_mangle]
extern "C" fn smashline_interpreter(
    agent: &mut L2CAgentBase,
    script_name: u64,
    original: extern "C" fn(&mut L2CAgentBase, &mut Variadic),
) {
    let name = Hash40(script_name);

    if let Some(script) =
        crate::create_agent::user_scripts(agent).and_then(|scripts| scripts.get(&name))
    {
        match script {
            UserScript::Function(func) => {
                unsafe { (*func)(agent) };
            }
            UserScript::Script(script) => {
                let script = script.read().clone();
                for action in script.actions.iter() {
                    if let Err(e) = ACTION_REGISTRY
                        .execute(action, unsafe { std::mem::transmute(agent as *mut _) })
                    {
                        println!("Failed to run action '{}': {e}", action.name);
                    }
                }
            }
        }
    } else {
        original(agent, &mut Variadic::new());
    }
}

#[no_mangle]
extern "C" fn set_smashline_interpreter_landing(landing_pad: u64, sp: u64) {
    FIBER_MAP
        .lock()
        .insert(unsafe { get_current_fiber() }, (landing_pad, sp));
}

#[no_mangle]
extern "C" fn clear_smashline_interpreter_landing() {
    FIBER_MAP.lock().remove(unsafe { &get_current_fiber() });
}

#[no_mangle]
extern "C" fn restore_smashline_interpreter_stack() -> u64 {
    FIBER_MAP
        .lock()
        .get(&unsafe { get_current_fiber() })
        .copied()
        .unwrap()
        .1
}

pub fn try_get_interpreter_landing_pad() -> Option<u64> {
    FIBER_MAP
        .lock()
        .get(&unsafe { get_current_fiber() })
        .copied()
        .map(|(ip, _)| ip)
}

static mut CALL_COROUTINE_OFFSET: usize = 0x52e400;

#[skyline::hook(replace = CALL_COROUTINE_OFFSET, inline)]
unsafe fn call_coroutine_hook(ctx: &mut InlineCtx) {
    let hash = Hash40(*(*ctx.registers[21].x.as_ref() as *const u64).add(2));
    *ctx.registers[2].x.as_mut() = *ctx.registers[8].x.as_ref();
    *ctx.registers[8].x.as_mut() = __smashline_interpreter as *const u64 as _;
    *ctx.registers[1].x.as_mut() = hash.0;
}

pub fn nro_hook(module_base: u64) {
    unsafe {
        CALL_COROUTINE_OFFSET += module_base as usize;
        skyline::install_hook!(call_coroutine_hook);
    }
}
