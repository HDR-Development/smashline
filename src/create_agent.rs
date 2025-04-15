use std::{
    borrow::BorrowMut, collections::{BTreeMap, HashMap}, ops::{Deref, DerefMut}, sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    }, time::Duration
};

use acmd_engine::SmashlineScript;
use skyline::hooks::InlineCtx;
use smash::{
    app::{BattleObject, BattleObjectModuleAccessor},
    lua2cpp::{
        L2CFighterAnimcmdEffectCommon, L2CFighterAnimcmdExpressionCommon,
        L2CFighterAnimcmdGameCommon, L2CFighterAnimcmdSoundCommon, L2CFighterCommon,
        L2CWeaponCommon,
    },
    lua_State,
};
use smashline::{
    locks::RwLock, Acmd, AcmdFunction, AgentEntry, AsHash40, BattleObjectCategory, Costume, Hash40, L2CAgentBase,
    L2CFighterBase, L2CValue, Priority, StatusLine, Variadic,
};
use vtables::{CustomDataAccessError, VirtualClass};

use crate::{
    cloning::weapons::IGNORE_NEW_AGENTS, interpreter::LoadedScript,
    static_accessor::StaticArrayAccessor, callbacks::{CALLBACKS, StatusCallbackFunction}
};

#[allow(improper_ctypes)]
extern "C" {
    #[link_name = "_ZN7lua2cpp16L2CFighterCommonC2EPN3app12BattleObjectEPNS1_26BattleObjectModuleAccessorEP9lua_State"]
    fn fighter_common_ctor(
        agent: *mut L2CFighterCommon,
        object: *mut BattleObject,
        boma: *mut BattleObjectModuleAccessor,
        lua_state: *mut lua_State,
    );

    #[link_name = "_ZN7lua2cpp15L2CWeaponCommonC2EPN3app12BattleObjectEPNS1_26BattleObjectModuleAccessorEP9lua_State"]
    fn weapon_common_ctor(
        agent: *mut L2CWeaponCommon,
        object: *mut BattleObject,
        boma: *mut BattleObjectModuleAccessor,
        lua_state: *mut lua_State,
    );

    #[link_name = "_ZN7lua2cpp27L2CFighterAnimcmdGameCommonC2EPN3app12BattleObjectEPNS1_26BattleObjectModuleAccessorEP9lua_State"]
    fn fighter_game_ctor(
        agent: *mut L2CFighterAnimcmdGameCommon,
        object: *mut BattleObject,
        boma: *mut BattleObjectModuleAccessor,
        lua_state: *mut lua_State,
    );

    #[link_name = "_ZN7lua2cpp29L2CFighterAnimcmdEffectCommonC1EPN3app12BattleObjectEPNS1_26BattleObjectModuleAccessorEP9lua_State"]
    fn fighter_effect_ctor(
        agent: *mut L2CFighterAnimcmdEffectCommon,
        object: *mut BattleObject,
        boma: *mut BattleObjectModuleAccessor,
        lua_state: *mut lua_State,
    );

    #[link_name = "_ZN7lua2cpp28L2CFighterAnimcmdSoundCommonC2EPN3app12BattleObjectEPNS1_26BattleObjectModuleAccessorEP9lua_State"]
    fn fighter_sound_ctor(
        agent: *mut L2CFighterAnimcmdSoundCommon,
        object: *mut BattleObject,
        boma: *mut BattleObjectModuleAccessor,
        lua_state: *mut lua_State,
    );

    #[link_name = "_ZN7lua2cpp33L2CFighterAnimcmdExpressionCommonC1EPN3app12BattleObjectEPNS1_26BattleObjectModuleAccessorEP9lua_State"]
    fn fighter_expression_ctor(
        agent: *mut L2CFighterAnimcmdExpressionCommon,
        object: *mut BattleObject,
        boma: *mut BattleObjectModuleAccessor,
        lua_state: *mut lua_State,
    );

    #[link_name = "_ZN7lua2cpp21L2CFighterAnimcmdBaseC2EPN3app12BattleObjectEPNS1_26BattleObjectModuleAccessorEP9lua_State"]
    fn fighter_animcmd_base_ctor(
        agent: *mut L2CAgentBase,
        object: *mut BattleObject,
        boma: *mut BattleObjectModuleAccessor,
        lua_state: *mut lua_State,
    );
}

type StatusFunc = Option<extern "C" fn(&mut L2CFighterBase) -> L2CValue>;
type StatusFunc1 = Option<extern "C" fn(&mut L2CFighterBase, L2CValue) -> L2CValue>;
type StatusFunc2 = Option<extern "C" fn(&mut L2CFighterBase, L2CValue, L2CValue) -> L2CValue>;

#[derive(Copy, Clone)]
pub enum StatusScriptFunction {
    Pre(StatusFunc),
    Main(StatusFunc),
    End(StatusFunc),
    Init(StatusFunc),
    Exec(StatusFunc),
    ExecStop(StatusFunc),
    Post(StatusFunc),
    Exit(StatusFunc),
    MapCorrection(StatusFunc),
    FixCamera(StatusFunc),
    FixPosSlow(StatusFunc),
    CheckDamage(StatusFunc1),
    CheckAttack(StatusFunc2),
    OnChangeLr(StatusFunc2),
    LeaveStop(StatusFunc2),
    NotifyEventGimmick(StatusFunc1),
    CalcParam(StatusFunc),
}

impl StatusScriptFunction {
    pub fn as_address(&self) -> usize {
        match self {
            Self::Pre(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::Main(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::End(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::Init(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::Exec(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::ExecStop(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::Post(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::Exit(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::MapCorrection(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::FixCamera(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::FixPosSlow(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::CheckDamage(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::CheckAttack(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::OnChangeLr(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::LeaveStop(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
            Self::NotifyEventGimmick(func) => {
                func.map(|f| f as *const () as usize).unwrap_or_default()
            }
            Self::CalcParam(func) => func.map(|f| f as *const () as usize).unwrap_or_default(),
        }
    }

    pub fn from_line(line: StatusLine, function: *const ()) -> Self {
        use StatusLine::*;
        match line {
            Pre => Self::Pre(unsafe { std::mem::transmute(function) }),
            Main => Self::Main(unsafe { std::mem::transmute(function) }),
            End => Self::End(unsafe { std::mem::transmute(function) }),
            Init => Self::Init(unsafe { std::mem::transmute(function) }),
            Exec => Self::Exec(unsafe { std::mem::transmute(function) }),
            ExecStop => Self::ExecStop(unsafe { std::mem::transmute(function) }),
            Post => Self::Post(unsafe { std::mem::transmute(function) }),
            Exit => Self::Exit(unsafe { std::mem::transmute(function) }),
            MapCorrection => Self::MapCorrection(unsafe { std::mem::transmute(function) }),
            FixCamera => Self::FixCamera(unsafe { std::mem::transmute(function) }),
            FixPosSlow => Self::FixPosSlow(unsafe { std::mem::transmute(function) }),
            CheckDamage => Self::CheckDamage(unsafe { std::mem::transmute(function) }),
            CheckAttack => Self::CheckAttack(unsafe { std::mem::transmute(function) }),
            OnChangeLr => Self::OnChangeLr(unsafe { std::mem::transmute(function) }),
            LeaveStop => Self::LeaveStop(unsafe { std::mem::transmute(function) }),
            NotifyEventGimmick => {
                Self::NotifyEventGimmick(unsafe { std::mem::transmute(function) })
            }
            CalcParam => Self::CalcParam(unsafe { std::mem::transmute(function) }),
            _ => unreachable!(),
        }
    }
}

pub struct StatusScript {
    pub id: i32,
    pub function: StatusScriptFunction,
    pub costume: Costume,
}

#[derive(Copy, Clone)]
pub struct AcmdScript {
    pub function: unsafe extern "C" fn(&mut L2CAgentBase),
    pub priority: Priority,
    pub costume: Costume,
}

type AcmdScriptSet = HashMap<Hash40, AcmdScript>;

#[derive(Default)]
pub struct AcmdScripts {
    game: AcmdScriptSet,
    effect: AcmdScriptSet,
    sound: AcmdScriptSet,
    expression: AcmdScriptSet,
}

impl AcmdScripts {
    pub fn remove_by_module_range(&mut self, start: usize, end: usize) {
        for set in [
            &mut self.game,
            &mut self.effect,
            &mut self.sound,
            &mut self.expression,
        ] {
            let working = std::mem::take(set);
            *set = working
                .into_iter()
                .filter(|(_, script)| {
                    !(start..end).contains(&(script.function as *const () as usize))
                })
                .collect();
        }
    }

    pub fn set_script(&mut self, name: Hash40, category: Acmd, script: AcmdScript) {
        let _ = match category {
            Acmd::Game => check_installed_script(self.game.borrow_mut(), name, category, script),
            Acmd::Effect => check_installed_script(self.effect.borrow_mut(), name, category, script),
            Acmd::Sound => check_installed_script(self.sound.borrow_mut(), name, category, script),
            Acmd::Expression => check_installed_script(self.expression.borrow_mut(), name, category, script),
        };
    }

    pub fn get_scripts(&self, category: Acmd) -> impl Iterator<Item = (&Hash40, &AcmdScript)> {
        match category {
            Acmd::Game => self.game.iter(),
            Acmd::Effect => self.effect.iter(),
            Acmd::Sound => self.sound.iter(),
            Acmd::Expression => self.expression.iter(),
        }
    }
}

fn check_installed_script(acmd_map: &mut AcmdScriptSet, name: Hash40, category: Acmd, script: AcmdScript) -> Option<AcmdScript> {
    if let Some((_, script_old)) = acmd_map.get_key_value(&name) {
        if script_old.priority < script.priority {
            println!("[smashline] Script {:#x} of {} {} replaced by {}", name.0, category, script_old.priority, script.priority);
            acmd_map.insert(name, script)
        }
        else {
            if script_old.priority == script.priority {
                println!("[smashline] Script {:#x} of {} already exists with {}! Check for duplicates!", name.0, category, script_old.priority);
            } else {
                println!("[smashline] Script {:#x} of {} with {} will be bypassed. Already installed with {}.", name.0, category, script.priority, script_old.priority);
            }
            None
        }
    }
    else {
        acmd_map.insert(name, script)
    }
}

fn install_script(
    acmd_scripts: &RwLock<BTreeMap<AgentEntry, AcmdScripts>>,
    agent_hash: Hash40,
    acmd: Acmd,
    agent: &mut L2CAgentBase,
    user_scripts: &mut HashMap<Hash40, UserScript>,
) {
    let costume = crate::utils::get_agent_costume(agent.battle_object as *const BattleObject).unwrap_or(0);
    let has_costume = crate::utils::has_costume(agent_hash, costume);
    let entry = AgentEntry::new(
        agent_hash.0,
        crate::utils::get_costume_data(agent_hash, costume)
    );

    let acmd_scripts = acmd_scripts.read();
    if let Some(scripts) = acmd_scripts.get(&entry) {
        for (hash, script) in scripts.get_scripts(acmd) {
            let c = script.costume.as_slice();

            if has_costume && !c.contains(&(costume as usize)) {
                continue;
            }

            if !has_costume && !c.is_empty() {
                continue;
            }

            agent.sv_set_function_hash(
                unsafe { std::mem::transmute(unreachable_smashline_script as *const ()) },
                *hash,
            );

            user_scripts.insert(*hash, UserScript::Function(script.function));
        }
    }
}

pub static ACMD_SCRIPTS: RwLock<BTreeMap<AgentEntry, AcmdScripts>> = RwLock::new(BTreeMap::new());
pub static ACMD_SCRIPTS_DEV: RwLock<BTreeMap<AgentEntry, AcmdScripts>> = RwLock::new(BTreeMap::new());
pub static STATUS_SCRIPTS: RwLock<BTreeMap<Hash40, Vec<StatusScript>>> =
    RwLock::new(BTreeMap::new());
pub static STATUS_SCRIPTS_DEV: RwLock<BTreeMap<Hash40, Vec<StatusScript>>> =
    RwLock::new(BTreeMap::new());

pub static COSTUMES: RwLock<BTreeMap<Hash40, Vec<Costume>>> = RwLock::new(BTreeMap::new());

pub const LOWERCASE_FIGHTER_NAMES: StaticArrayAccessor<&'static str> =
    StaticArrayAccessor::new(0x4f81e20, 118);

pub const LOWERCASE_WEAPON_NAMES: StaticArrayAccessor<&'static str> =
    StaticArrayAccessor::new(0x5186bd0, 0x267);

pub const LOWERCASE_WEAPON_OWNER_NAMES: StaticArrayAccessor<&'static str> =
    StaticArrayAccessor::new(0x5189240, 0x267);

enum OriginalFunc {
    CreateAgentShare {
        agent: i32,
        function: extern "C" fn(
            i32,
            &mut BattleObject,
            &mut BattleObjectModuleAccessor,
            *mut lua_State,
        ) -> Option<&'static mut L2CAgentBase>,
    },
    CreateAgent(
        extern "C" fn(
            &mut BattleObject,
            &mut BattleObjectModuleAccessor,
            *mut lua_State,
        ) -> Option<&'static mut L2CAgentBase>,
    ),
}

impl OriginalFunc {
    fn call(
        &self,
        object: &mut BattleObject,
        boma: &mut BattleObjectModuleAccessor,
        lua_state: *mut lua_State,
    ) -> Option<&'static mut L2CAgentBase> {
        match self {
            Self::CreateAgentShare { agent, function } => function(*agent, object, boma, lua_state),
            Self::CreateAgent(function) => function(object, boma, lua_state),
        }
    }
}

struct RecursionGuard(AtomicBool);

impl RecursionGuard {
    const fn new() -> Self {
        RecursionGuard(AtomicBool::new(false))
    }
}

struct RecursionStackGuard<'a>(&'a AtomicBool);

impl<'a> Drop for RecursionStackGuard<'a> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl RecursionGuard {
    fn acquire(&self) -> Option<RecursionStackGuard<'_>> {
        if self.0.swap(true, Ordering::AcqRel) {
            return None;
        }

        Some(RecursionStackGuard(&self.0))
    }
}

impl Deref for L2CAnimcmdWrapper {
    type Target = L2CAnimcmdWrapperVTable;

    fn deref(&self) -> &Self::Target {
        unsafe { std::mem::transmute(*(self as *const _ as *const *const u64)) }
    }
}

impl DerefMut for L2CAnimcmdWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::mem::transmute(*(self as *mut _ as *mut *mut u64)) }
    }
}

#[repr(transparent)]
struct L2CAnimcmdWrapper(L2CAgentBase);

#[no_mangle]
pub(crate) extern "C" fn unreachable_smashline_script(
    _fighter: &mut L2CAgentBase,
    _variadic: &mut Variadic,
) {
    panic!("unreachable smashline script called, this is an implementation error");
}

pub enum UserScript {
    Function(unsafe extern "C" fn(&mut L2CAgentBase)),
    Script(Arc<locks::RwLock<Arc<SmashlineScript>>>),
}

#[derive(Default)]
struct L2CAnimcmdWrapperData {
    original_deleter: Option<extern "C" fn(&mut L2CAnimcmdWrapper)>,
    additional_module: Option<i32>,
    original_scripts: HashMap<Hash40, AcmdFunction>,
    user_scripts: HashMap<Hash40, UserScript>,
    loaded_script_arc: Option<Arc<Vec<LoadedScript>>>,
}

impl VirtualClass for L2CAnimcmdWrapper {
    const DYNAMIC_MODULE: Option<&'static str> = Some("lu2cpp_common");
    const VTABLE_OFFSET: usize = 0x800148;
    const DISABLE_OFFSET_CHECK: bool = true;

    type Accessor = L2CAnimcmdWrapperVTableAccessor;
    type CustomData = L2CAnimcmdWrapperData;

    fn vtable_accessor(&self) -> &Self::Accessor {
        unsafe { std::mem::transmute(self) }
    }

    fn vtable_accessor_mut(&mut self) -> &mut Self::Accessor {
        unsafe { std::mem::transmute(self) }
    }
}

#[vtables::vtable]
mod l2c_animcmd_wrapper {
    fn destructor(&mut self);
    fn deleter(&mut self);
    fn coroutine_yield(&mut self);
    fn start_coroutine(&mut self, coroutine_index: i32, name: Hash40, state: &mut i32) -> u32;
    fn resume_coroutine(&mut self, coroutine_index: i32, state: &mut i32) -> u32;
    fn get_unused_coroutine_index(&self, max: i32) -> i32;
    fn clean_coroutine(&mut self, index: i32) -> bool;
    fn set_coroutine_release_control(&mut self, release_control: bool);
    fn is_coroutine_release_control(&self) -> bool;
    fn added_function1(&self);
    fn added_function2(&self);
    fn added_function3(&self);
}

extern "C" fn wrap_deleter_animcmd(agent: &mut L2CAnimcmdWrapper) {
    let data = vtables::vtable_custom_data::<_, L2CAnimcmdWrapper>(agent.deref()).unwrap();

    let original_deleter = data.original_deleter;
    let additional_fighter = data.additional_module;

    let agent: &mut L2CAnimcmdWrapper = unsafe {
        std::mem::transmute(vtables::vtable_restore_vtable::<
            L2CAnimcmdWrapperVTable,
            L2CAnimcmdWrapper,
        >(std::mem::transmute(agent)))
    };

    if let Some(original) = original_deleter {
        original(agent);
    }

    if let Some(additional_fighter) = additional_fighter {
        crate::utils::unload_fighter_module(additional_fighter);
    }
}

fn create_agent_hook(
    object: &mut BattleObject,
    boma: &mut BattleObjectModuleAccessor,
    lua_state: *mut lua_State,
    acmd: Acmd,
    original: OriginalFunc,
) -> Option<&'static mut L2CAgentBase> {
    static RECURSION_GUARD: RecursionGuard = RecursionGuard::new();
    let Some(_guard) = RECURSION_GUARD.acquire() else {
        return original.call(object, boma, lua_state);
    };

    let Some(category) = BattleObjectCategory::from_battle_object_id(object.battle_object_id)
    else {
        // TODO: Warn
        return original.call(object, boma, lua_state);
    };

    // TODO: Log

    match category {
        BattleObjectCategory::Fighter => {
            let kind = match &original {
                OriginalFunc::CreateAgentShare { agent, .. } => *agent,
                _ => object.kind,
            };

            let Some(name) = LOWERCASE_FIGHTER_NAMES.get(kind as usize) else {
                // TODO: Warn
                return original.call(object, boma, lua_state);
            };

            let agent = if let Some(agent) = original.call(object, boma, lua_state) {
                agent
            } else {
                match acmd {
                    Acmd::Game => {
                        let mut agent = Box::new(std::mem::MaybeUninit::zeroed());
                        unsafe {
                            fighter_game_ctor(agent.as_mut_ptr(), object, boma, lua_state);
                            Box::leak(agent.assume_init()) as _
                        }
                    }
                    Acmd::Effect => {
                        let mut agent = Box::new(std::mem::MaybeUninit::zeroed());
                        unsafe {
                            fighter_effect_ctor(agent.as_mut_ptr(), object, boma, lua_state);
                            Box::leak(agent.assume_init()) as _
                        }
                    }
                    Acmd::Sound => {
                        let mut agent = Box::new(std::mem::MaybeUninit::zeroed());
                        unsafe {
                            fighter_sound_ctor(agent.as_mut_ptr(), object, boma, lua_state);
                            Box::leak(agent.assume_init()) as _
                        }
                    }
                    Acmd::Expression => {
                        let mut agent = Box::new(std::mem::MaybeUninit::zeroed());
                        unsafe {
                            fighter_expression_ctor(agent.as_mut_ptr(), object, boma, lua_state);
                            Box::leak(agent.assume_init()) as _
                        }
                    }
                }
            };

            let original_scripts: HashMap<Hash40, AcmdFunction> = agent
                .function_map
                .iter()
                .map(|(name, function)| (*name, unsafe { std::mem::transmute(*function) }))
                .collect();

            let hash = Hash40::new(name);

            let mut user_scripts = HashMap::new();

            let smashline_scripts = crate::interpreter::get_or_load_scripts(name, None);
            for script in smashline_scripts
                .iter()
                .filter(|script| acmd == script.script.read().category)
            {
                agent.sv_set_function_hash(
                    unsafe { std::mem::transmute(unreachable_smashline_script as *const ()) },
                    script.script.read().replace.as_hash40(),
                );
                user_scripts.insert(
                    script.script.read().replace.as_hash40(),
                    UserScript::Script(script.script.clone()),
                );
            }

            install_script(&ACMD_SCRIPTS, hash, acmd, agent, &mut user_scripts);
            install_script(&ACMD_SCRIPTS_DEV, hash, acmd, agent, &mut user_scripts);

            let agent: &'static mut L2CAgentBase = unsafe {
                let wrapper: &'static mut L2CAnimcmdWrapper = std::mem::transmute(agent);
                let deleter = wrapper.vtable_accessor_mut().get_deleter();
                wrapper
                    .vtable_accessor_mut()
                    .set_deleter(wrap_deleter_animcmd);
                let data =
                    vtables::vtable_custom_data_mut::<_, L2CAnimcmdWrapper>(wrapper.deref_mut());
                data.additional_module = None;
                data.original_deleter = Some(deleter);
                data.user_scripts = user_scripts;
                data.loaded_script_arc = Some(smashline_scripts);
                data.original_scripts = original_scripts;
                std::mem::transmute(wrapper)
            };

            Some(agent)
        }
        BattleObjectCategory::Weapon => {
            let Some(name) = crate::utils::get_weapon_name(object.kind) else {
                // TODO: Warn
                return original.call(object, boma, lua_state);
            };

            let Some(owner) = crate::utils::get_weapon_owner_name(object.kind) else {
                // TODO: Warn
                return original.call(object, boma, lua_state);
            };

            let (agent, additional_module) = if let Some(agent) =
                original.call(object, boma, lua_state)
            {
                (agent, None)
            } else if let Some(fighter_id) = crate::utils::get_weapon_code_dependency(object.kind) {
                crate::utils::load_fighter_module(fighter_id);
                while !crate::utils::is_fighter_module_loaded(fighter_id) {
                    std::thread::sleep(Duration::from_millis(1));
                }

                IGNORE_NEW_AGENTS.store(true, Ordering::Relaxed);
                let result = original.call(object, boma, lua_state);
                IGNORE_NEW_AGENTS.store(false, Ordering::Relaxed);

                if let Some(agent) = result {
                    (agent, Some(fighter_id))
                } else {
                    let mut agent = Box::new(std::mem::MaybeUninit::zeroed());
                    unsafe {
                        fighter_animcmd_base_ctor(agent.as_mut_ptr(), object, boma, lua_state);
                        (Box::leak(agent.assume_init()), None)
                    }
                }
            } else {
                let mut agent = Box::new(std::mem::MaybeUninit::zeroed());
                unsafe {
                    fighter_animcmd_base_ctor(agent.as_mut_ptr(), object, boma, lua_state);
                    (Box::leak(agent.assume_init()), None)
                }
            };

            let qualified_name = format!("{owner}_{name}");

            let hash = Hash40::new(&qualified_name);

            let mut user_scripts = HashMap::new();

            let original_scripts: HashMap<Hash40, AcmdFunction> = agent
                .function_map
                .iter()
                .map(|(name, function)| (*name, unsafe { std::mem::transmute(*function) }))
                .collect();

            let smashline_scripts = crate::interpreter::get_or_load_scripts(&owner, Some(&name));
            for script in smashline_scripts
                .iter()
                .filter(|script| acmd == script.script.read().category)
            {
                agent.sv_set_function_hash(
                    unsafe { std::mem::transmute(unreachable_smashline_script as *const ()) },
                    script.script.read().replace.as_hash40(),
                );
                user_scripts.insert(
                    script.script.read().replace.as_hash40(),
                    UserScript::Script(script.script.clone()),
                );
            }

            install_script(&ACMD_SCRIPTS, hash, acmd, agent, &mut user_scripts);
            install_script(&ACMD_SCRIPTS_DEV, hash, acmd, agent, &mut user_scripts);

            let agent: &'static mut L2CAgentBase = unsafe {
                let wrapper: &'static mut L2CAnimcmdWrapper = std::mem::transmute(agent);
                let deleter = wrapper.vtable_accessor_mut().get_deleter();
                wrapper
                    .vtable_accessor_mut()
                    .set_deleter(wrap_deleter_animcmd);
                let data =
                    vtables::vtable_custom_data_mut::<_, L2CAnimcmdWrapper>(wrapper.deref_mut());
                data.additional_module = additional_module;
                data.original_deleter = Some(deleter);
                data.user_scripts = user_scripts;
                data.loaded_script_arc = Some(smashline_scripts);
                data.original_scripts = original_scripts;
                std::mem::transmute(wrapper)
            };

            Some(agent)
        }
        _ => original.call(object, boma, lua_state),
    }
}

macro_rules! create_agent_hook {
    ($($offset:expr => ($category:ident, $class:ident);)*) => {
        paste::paste! {
            pub fn install_create_agent_hooks() {
                skyline::install_hooks! {
                    $(
                        [<create_agent_ $category:lower _ $class>],
                    )*
                }
            }

            $(
                #[skyline::hook(offset = $offset)]
                fn [<create_agent_ $category:lower _ $class>](
                    object: &mut BattleObject,
                    boma: &mut BattleObjectModuleAccessor,
                    lua_state: *mut lua_State,
                ) -> Option<&'static mut L2CAgentBase>
                {
                    create_agent_hook(object, boma, lua_state, Acmd::$category, OriginalFunc::CreateAgent(original!()))
                }
            )*
        }
    };
    (share; $($offset:expr => ($category:ident, $class:ident);)*) => {
        paste::paste! {
            pub fn install_create_agent_share_hooks() {
                skyline::install_hooks! {
                    $(
                        [<create_agent_ $category:lower _ $class>],
                    )*
                }
            }

            $(
                #[skyline::hook(offset = $offset)]
                fn [<create_agent_ $category:lower _ $class>](
                    kind: i32,
                    object: &mut BattleObject,
                    boma: &mut BattleObjectModuleAccessor,
                    lua_state: *mut lua_State,
                ) -> Option<&'static mut L2CAgentBase>
                {
                    create_agent_hook(object, boma, lua_state, Acmd::$category, OriginalFunc::CreateAgentShare { agent: kind, function: original!() })
                }
            )*
        }
    };
}

extern "C" fn wrap_deleter(agent: &mut L2CFighterWrapper) {
    let data = vtables::vtable_custom_data::<_, L2CFighterWrapper>(agent.deref()).unwrap();

    let additional_fighter = data.additional_fighter_module;
    let original_deleter = data.original_deleter;
    let is_weapon = data.is_weapon;

    let agent: &mut L2CFighterWrapper = unsafe {
        std::mem::transmute(vtables::vtable_restore_vtable::<
            L2CFighterWrapperVTable,
            L2CFighterWrapper,
        >(std::mem::transmute(agent)))
    };

    if let Some(original) = original_deleter {
        original(agent);
    } else if is_weapon {
        drop(unsafe { Box::from_raw(agent.as_weapon_mut()) })
    } else {
        drop(unsafe { Box::from_raw(agent.as_fighter_mut()) })
    }

    if let Some(additional_fighter) = additional_fighter {
        crate::utils::unload_fighter_module(additional_fighter);
    }
}

fn install_status_scripts(
    old_total: i32,
    list: &[StatusScript],
    agent: &mut L2CFighterWrapper,
) -> i32 {
    let data = vtables::vtable_custom_data::<_, L2CFighterWrapper>(agent.deref()).unwrap();
    let costume = crate::utils::get_agent_costume(agent.0.battle_object as *const BattleObject).unwrap_or(0);
    let has_costume = crate::utils::has_costume(data.hash, costume);

    let mut max_new = old_total;

    for status in list.iter() {
        let c = status.costume.as_slice();

        if has_costume && !c.contains(&(costume as usize)) {
            continue;
        }

        if !has_costume && !c.is_empty() {
            continue;
        }

        max_new = max_new.max(status.id + 1);

        use StatusScriptFunction::*;

        macro_rules! set {
            ($($i:ident),*) => {
                match status.function {
                    $(
                        $i(f) => {
                            let id = smash::lib::L2CValue::new(status.id);
                            let condition = smash::lib::L2CValue::new(StatusLine::$i as i32);
                            agent.0.sv_set_status_func(&id, &condition, unsafe { std::mem::transmute(f) })
                        },
                    )*
                }
            }
        }

        set!(
            Pre,
            Main,
            End,
            Init,
            Exec,
            ExecStop,
            Post,
            Exit,
            MapCorrection,
            FixCamera,
            FixPosSlow,
            CheckDamage,
            CheckAttack,
            OnChangeLr,
            LeaveStop,
            NotifyEventGimmick,
            CalcParam
        );
    }

    max_new
}

extern "C" fn set_status_scripts(agent: &mut L2CFighterWrapper) {
    let data = vtables::vtable_custom_data::<_, L2CFighterWrapper>(agent.deref()).unwrap();
    let hash = data.hash;
    let is_weapon = data.is_weapon;

    let is_new = data.original_set_status_scripts.is_none();

    if let Some(original) = data.original_set_status_scripts {
        original(agent);
    } else if is_weapon {
        agent.as_weapon_mut().sub_weapon_common_settings();
    } else {
        agent.as_fighter_mut().sub_set_fighter_common_table();
        agent.as_fighter_mut().sub_fighter_common_settings();
    }

    let statuses = STATUS_SCRIPTS.read();
    let statuses_dev = STATUS_SCRIPTS_DEV.read();

    let old_total = if is_new {
        0
    } else {
        agent.0.global_table.try_table().unwrap()[0xC]
            .try_integer()
            .unwrap_or_default() as i32
    };

    let mut original_statuses = HashMap::new();

    for id in 0..old_total {
        macro_rules! get {
            ($($i:ident),*) => {
                $(
                    {
                        let lua_id = smash::lib::L2CValue::new(id);
                        let condition = smash::lib::L2CValue::new(StatusLine::$i as i32);
                        let function = agent.0.sv_get_status_func(&lua_id, &condition);
                        if let Some(ptr) = function.try_pointer() {
                            original_statuses.insert((StatusLine::$i, id), ptr.cast::<()>() as _);
                        }
                    }
                )*
            }
        }

        get! {
            Pre,
            Main,
            End,
            Init,
            Exec,
            ExecStop,
            Post,
            Exit,
            MapCorrection,
            FixCamera,
            FixPosSlow,
            CheckDamage,
            CheckAttack,
            OnChangeLr,
            LeaveStop,
            NotifyEventGimmick,
            CalcParam
        }
    }

    let costume = crate::utils::get_agent_costume(agent.0.battle_object as *const BattleObject).unwrap_or(0);
    let has_costume = crate::utils::has_costume(hash, costume);

    let callbacks = CALLBACKS.read();
    let mut status_callbacks = Vec::new();

    for callback in callbacks.iter() {
        if callback.hash == Some(hash) {
            let c = callback.costume.as_slice();

            if has_costume && !c.contains(&(costume as usize)) {
                continue;
            }

            if !has_costume && !c.is_empty() {
                continue;
            }

            status_callbacks.push(callback.function);
        }
        else if is_weapon {
            if callback.hash == Some(Hash40::new("weapon")) {
                status_callbacks.push(callback.function);
            }
        }
        else {
            if callback.hash == Some(Hash40::new("fighter")) {
                status_callbacks.push(callback.function);
            }
        }
    }

    let data = vtables::vtable_custom_data_mut::<_, L2CFighterWrapper>(agent.deref_mut());

    data.status_callbacks = status_callbacks;

    data.original_statuses = original_statuses;

    let mut new_total = old_total;

    let hashes: &[Hash40] = if is_weapon {
        &[Hash40::new("weapon"), hash]
    } else {
        &[Hash40::new("fighter"), hash]
    };

    for h in hashes {
        if let Some(common) = statuses.get(h) {
            new_total = new_total.max(install_status_scripts(old_total, common, agent));
        }
        if let Some(common) = statuses_dev.get(h) {
            new_total = new_total.max(install_status_scripts(old_total, common, agent));
        }
    }

    agent.0.global_table.try_table_mut().unwrap()[0xC] = smash::lib::L2CValue::new(new_total);
}

#[repr(transparent)]
struct L2CFighterWrapper(L2CFighterBase);

impl L2CFighterWrapper {
    #[allow(dead_code)]
    fn as_fighter(&self) -> &L2CFighterCommon {
        unsafe { std::mem::transmute(self) }
    }

    fn as_fighter_mut(&mut self) -> &mut L2CFighterCommon {
        unsafe { std::mem::transmute(self) }
    }

    #[allow(dead_code)]
    fn as_weapon(&self) -> &L2CWeaponCommon {
        unsafe { std::mem::transmute(self) }
    }

    fn as_weapon_mut(&mut self) -> &mut L2CWeaponCommon {
        unsafe { std::mem::transmute(self) }
    }
}

impl Deref for L2CFighterWrapper {
    type Target = L2CFighterWrapperVTable;

    fn deref(&self) -> &Self::Target {
        unsafe { std::mem::transmute(*(self as *const _ as *const *const u64)) }
    }
}

impl DerefMut for L2CFighterWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::mem::transmute(*(self as *mut _ as *mut *mut u64)) }
    }
}

#[derive(Default)]
struct L2CFighterWrapperData {
    kind: i32,
    hash: Hash40,
    is_weapon: bool,
    additional_fighter_module: Option<i32>,
    original_statuses: HashMap<(StatusLine, i32), *const ()>,
    original_deleter: Option<extern "C" fn(&mut L2CFighterWrapper)>,
    original_set_status_scripts: Option<extern "C" fn(&mut L2CFighterWrapper)>,
    status_callbacks: Vec<StatusCallbackFunction>
}

impl VirtualClass for L2CFighterWrapper {
    const DYNAMIC_MODULE: Option<&'static str> = Some("lu2cpp_common");
    const VTABLE_OFFSET: usize = 0x800148;
    const DISABLE_OFFSET_CHECK: bool = true;

    type Accessor = L2CFighterWrapperVTableAccessor;
    type CustomData = L2CFighterWrapperData;

    fn vtable_accessor(&self) -> &Self::Accessor {
        unsafe { std::mem::transmute(self) }
    }

    fn vtable_accessor_mut(&mut self) -> &mut Self::Accessor {
        unsafe { std::mem::transmute(self) }
    }
}

#[vtables::vtable]
mod l2c_fighter_wrapper {
    fn destructor(&mut self);
    fn deleter(&mut self);
    fn coroutine_yield(&mut self);
    fn start_coroutine(&mut self, coroutine_index: i32, name: Hash40, state: &mut i32) -> u32;
    fn resume_coroutine(&mut self, coroutine_index: i32, state: &mut i32) -> u32;
    fn get_unused_coroutine_index(&self, max: i32) -> i32;
    fn clean_coroutine(&mut self, index: i32) -> bool;
    fn set_coroutine_release_control(&mut self, release_control: bool);
    fn is_coroutine_release_control(&self) -> bool;
    fn set_status_scripts(&mut self);
    fn sys_line_system_init(&mut self);
    fn sub_begin_added_lines(&mut self);
    fn sys_line_status_end_control(&mut self);
    fn sub_end_added_lines(&mut self);
    fn RESET(&mut self);
}

#[skyline::hook(offset = 0x64bbd0)]
fn create_agent_status_fighter(
    object: &mut BattleObject,
    boma: &mut BattleObjectModuleAccessor,
    lua_state: *mut lua_State,
) -> Option<&'static mut L2CFighterBase> {
    let Some(name) = LOWERCASE_FIGHTER_NAMES.get(object.kind as usize) else {
        // TODO: warn
        return call_original!(object, boma, lua_state);
    };

    let (is_new, agent) = if let Some(agent) = call_original!(object, boma, lua_state) {
        (false, agent)
    } else {
        let mut fighter = Box::new(std::mem::MaybeUninit::zeroed());
        unsafe {
            fighter_common_ctor(fighter.as_mut_ptr(), object, boma, lua_state);
            (true, Box::leak(fighter.assume_init()) as _)
        }
    };

    // SAFETY: This is safe because L2CFighterWrapper's memory layout is transparent
    let wrapper: &'static mut L2CFighterWrapper = unsafe { std::mem::transmute(agent) };

    let original_deleter = if is_new {
        None
    } else {
        Some(wrapper.vtable_accessor().get_deleter())
    };

    let original_set_status_scripts = if is_new {
        None
    } else {
        Some(wrapper.vtable_accessor().get_set_status_scripts())
    };

    wrapper.vtable_accessor_mut().set_deleter(wrap_deleter);
    wrapper
        .vtable_accessor_mut()
        .set_set_status_scripts(set_status_scripts);

    let data = vtables::vtable_custom_data_mut::<_, L2CFighterWrapper>(wrapper.deref_mut());
    data.hash = Hash40::new(name);
    data.kind = object.kind;
    data.is_weapon = false;
    data.original_deleter = original_deleter;
    data.original_set_status_scripts = original_set_status_scripts;

    Some(unsafe { std::mem::transmute(wrapper) })
}

#[skyline::hook(offset = 0x33ac150)]
fn create_agent_status_weapon(
    object: &mut BattleObject,
    boma: &mut BattleObjectModuleAccessor,
    lua_state: *mut lua_State,
) -> Option<&'static mut L2CFighterBase> {
    let Some(name) = crate::utils::get_weapon_name(object.kind) else {
        return call_original!(object, boma, lua_state);
    };

    let Some(owner_name) = crate::utils::get_weapon_owner_name(object.kind) else {
        return call_original!(object, boma, lua_state);
    };

    let (is_new, agent, additional_fighter) =
        if let Some(agent) = call_original!(object, boma, lua_state) {
            (false, agent, None)
        } else if let Some(fighter_id) = crate::utils::get_weapon_code_dependency(object.kind) {
            crate::utils::load_fighter_module(fighter_id);
            while !crate::utils::is_fighter_module_loaded(fighter_id) {
                std::thread::sleep(Duration::from_millis(1));
            }

            IGNORE_NEW_AGENTS.store(true, Ordering::Relaxed);
            let result = call_original!(object, boma, lua_state);
            IGNORE_NEW_AGENTS.store(false, Ordering::Relaxed);

            if let Some(agent) = result {
                (false, agent, Some(fighter_id))
            } else {
                crate::utils::unload_fighter_module(fighter_id);
                let mut agent = Box::new(std::mem::MaybeUninit::zeroed());
                unsafe {
                    weapon_common_ctor(agent.as_mut_ptr(), object, boma, lua_state);
                    (true, Box::leak(agent.assume_init()) as _, None)
                }
            }
        } else {
            let mut weapon = Box::new(std::mem::MaybeUninit::zeroed());
            unsafe {
                weapon_common_ctor(weapon.as_mut_ptr(), object, boma, lua_state);
                (true, Box::leak(weapon.assume_init()) as _, None)
            }
        };

    let wrapper: &'static mut L2CFighterWrapper = unsafe { std::mem::transmute(agent) };

    let original_deleter = if is_new {
        None
    } else {
        Some(wrapper.vtable_accessor().get_deleter())
    };

    let original_set_status_scripts = if is_new {
        None
    } else {
        Some(wrapper.vtable_accessor().get_set_status_scripts())
    };

    wrapper.vtable_accessor_mut().set_deleter(wrap_deleter);
    wrapper
        .vtable_accessor_mut()
        .set_set_status_scripts(set_status_scripts);

    let data = vtables::vtable_custom_data_mut::<_, L2CFighterWrapper>(wrapper.deref_mut());
    data.hash = Hash40::new(&format!("{owner_name}_{name}"));
    data.kind = object.kind;
    data.is_weapon = true;
    data.original_deleter = original_deleter;
    data.additional_fighter_module = additional_fighter;
    data.original_set_status_scripts = original_set_status_scripts;

    Some(unsafe { std::mem::transmute(wrapper) })
}

pub(crate) fn original_scripts<'a>(
    agent: &'a L2CAgentBase,
) -> Option<&HashMap<Hash40, AcmdFunction>> {
    let wrapper: &'static L2CAnimcmdWrapper = unsafe { std::mem::transmute(agent) };
    match vtables::vtable_custom_data::<_, L2CAnimcmdWrapper>(wrapper.deref()) {
        Ok(data) => Some(&data.original_scripts),
        Err(CustomDataAccessError::NotRelocated) => None,
        Err(e) => panic!("failed to get original scripts: {e}"),
    }
}

pub(crate) fn user_scripts<'a>(agent: &'a L2CAgentBase) -> Option<&HashMap<Hash40, UserScript>> {
    let wrapper: &'static L2CAnimcmdWrapper = unsafe { std::mem::transmute(agent) };
    match vtables::vtable_custom_data::<_, L2CAnimcmdWrapper>(wrapper.deref()) {
        Ok(data) => Some(&data.user_scripts),
        Err(CustomDataAccessError::NotRelocated) => None,
        Err(e) => {
            panic!("failed to get user scripts: {e}");
        }
    }
}

pub(crate) fn original_status<'a>(
    fighter: &'a L2CFighterBase,
) -> Option<&HashMap<(StatusLine, i32), *const ()>> {
    let wrapper: &'static L2CFighterWrapper = unsafe { std::mem::transmute(fighter) };
    match vtables::vtable_custom_data::<_, L2CFighterWrapper>(wrapper.deref()) {
        Ok(data) => Some(&data.original_statuses),
        Err(CustomDataAccessError::NotRelocated) => None,
        Err(e) => panic!("failed to get status scripts: {e}"),
    }
}

pub(crate) fn status_callbacks<'a>(
    fighter: &'a L2CFighterBase,
) -> Vec<StatusCallbackFunction> {
    let wrapper: &'static L2CFighterWrapper = unsafe { std::mem::transmute(fighter) };
    vtables::vtable_custom_data::<_, L2CFighterWrapper>(wrapper.deref())
        .unwrap().status_callbacks.clone()
}

pub(crate) fn agent_hash(fighter: &L2CFighterBase) -> Hash40 {
    let wrapper: &'static L2CFighterWrapper = unsafe { std::mem::transmute(fighter) };
    vtables::vtable_custom_data::<_, L2CFighterWrapper>(wrapper.deref())
        .unwrap()
        .hash
}

#[skyline::hook(offset = 0x33b6820, inline)]
unsafe fn enable_lua_module(ctx: &mut InlineCtx) {
    *ctx.registers[8].x.as_mut() =
        skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 + 0x33b90b0;
}

pub fn install_status_create_agent_hooks() {
    skyline::install_hooks! {
        create_agent_status_fighter,
        create_agent_status_weapon,
        enable_lua_module
    }
}

create_agent_hook! {
    0x64c310 => (Game, fighter);
    0x64c930 => (Effect, fighter);
    0x64cf50 => (Expression, fighter);
    0x64d570 => (Sound, fighter);
    0x33ad050 => (Game, weapon);
    0x33adfb0 => (Effect, weapon);
    0x33aef10 => (Sound, weapon);
}

create_agent_hook! {
    share;
    0x64db90 => (Game, share_fighter);
    0x64e2a0 => (Effect, share_fighter);
    0x64e8b0 => (Expression, share_fighter);
    0x64eec0 => (Sound, share_fighter);
}
