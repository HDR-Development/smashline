use std::{
    collections::{BTreeMap, HashMap},
    ops::{Deref, DerefMut},
};

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
    locks::RwLock, Acmd, BattleObjectCategory, Hash40, L2CAgentBase, L2CFighterBase, L2CValue,
    LuaConst, Priority, StatusLine, Variadic,
};
use vtables::VirtualClass;

use crate::static_accessor::StaticArrayAccessor;

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

pub enum StatusScriptId {
    Replace {
        id: LuaConst,
        original: &'static RwLock<*const ()>,
    },
    New(i32),
}

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
    pub id: StatusScriptId,
    pub function: StatusScriptFunction,
}

#[derive(Copy, Clone)]
pub struct AcmdScript {
    pub function: extern "C" fn(&mut L2CAgentBase, &mut Variadic),
    pub priority: Priority,
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
    pub fn set_script(&mut self, name: Hash40, category: Acmd, script: AcmdScript) {
        let _ = match category {
            Acmd::Game => self.game.insert(name, script),
            Acmd::Effect => self.effect.insert(name, script),
            Acmd::Sound => self.sound.insert(name, script),
            Acmd::Expression => self.expression.insert(name, script),
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

pub static ACMD_SCRIPTS: RwLock<BTreeMap<Hash40, AcmdScripts>> = RwLock::new(BTreeMap::new());
pub static STATUS_SCRIPTS: RwLock<BTreeMap<Hash40, Vec<StatusScript>>> =
    RwLock::new(BTreeMap::new());

pub const LOWERCASE_FIGHTER_NAMES: StaticArrayAccessor<&'static str> =
    StaticArrayAccessor::new(0x4f7fe20, 118);

pub const LOWERCASE_WEAPON_NAMES: StaticArrayAccessor<&'static str> =
    StaticArrayAccessor::new(0x5184bd0, 0x267);

pub const LOWERCASE_WEAPON_OWNER_NAMES: StaticArrayAccessor<&'static str> =
    StaticArrayAccessor::new(0x5187240, 0x267);

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

fn create_agent_hook(
    object: &mut BattleObject,
    boma: &mut BattleObjectModuleAccessor,
    lua_state: *mut lua_State,
    acmd: Acmd,
    original: OriginalFunc,
) -> Option<&'static mut L2CAgentBase> {
    let Some(category) = BattleObjectCategory::from_battle_object_id(object.battle_object_id)
    else {
        // TODO: Warn
        return original.call(object, boma, lua_state);
    };

    // TODO: Log

    match category {
        BattleObjectCategory::Fighter => {
            let Some(name) = LOWERCASE_FIGHTER_NAMES.get(object.kind as usize) else {
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

            let hash = Hash40::new(name);

            let acmd_scripts = ACMD_SCRIPTS.read();
            if let Some(scripts) = acmd_scripts.get(&hash) {
                for (hash, script) in scripts.get_scripts(acmd) {
                    agent.sv_set_function_hash(
                        unsafe { std::mem::transmute(script.function) },
                        *hash,
                    );
                }
            }

            Some(agent)
        }
        BattleObjectCategory::Weapon => {
            let Some(name) = LOWERCASE_WEAPON_NAMES.get(object.kind as usize) else {
                // TODO: Warn
                return original.call(object, boma, lua_state);
            };

            let Some(owner) = LOWERCASE_WEAPON_OWNER_NAMES.get(object.kind as usize) else {
                // TODO: Warn
                return original.call(object, boma, lua_state);
            };

            let agent = if let Some(agent) = original.call(object, boma, lua_state) {
                agent
            } else {
                let mut agent = Box::new(std::mem::MaybeUninit::zeroed());
                unsafe {
                    fighter_animcmd_base_ctor(agent.as_mut_ptr(), object, boma, lua_state);
                    Box::leak(agent.assume_init())
                }
            };

            let name = format!("{owner}_{name}");

            let hash = Hash40::new(&name);

            let acmd_scripts = ACMD_SCRIPTS.read();
            if let Some(scripts) = acmd_scripts.get(&hash) {
                for (hash, script) in scripts.get_scripts(acmd) {
                    agent.sv_set_function_hash(
                        unsafe { std::mem::transmute(script.function) },
                        *hash,
                    );
                }
            }

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

fn resolve_lua_const(luac: &LuaConst) -> i32 {
    extern "C" {
        #[link_name = "_ZN3lib18lua_bind_get_valueIiEEbmRT_"]
        fn get_lua_int(hash: u64, val: &mut i32);
    }

    match luac {
        LuaConst::Resolved(val) => *val,
        LuaConst::UnresolvedHash(hash) => {
            let mut val = 0;
            unsafe {
                get_lua_int(*hash, &mut val);
            }

            val
        }
        LuaConst::UnresolvedStr(name) => {
            let name = name.as_str().unwrap();
            let hash = lua_bind_hash::lua_bind_hash_str(name);

            let mut val = 0;
            unsafe {
                get_lua_int(hash, &mut val);
            }

            val
        }
    }
}

extern "C" fn wrap_deleter(agent: &mut L2CFighterWrapper) {
    let data = vtables::vtable_custom_data::<_, L2CFighterWrapper>(agent.deref());

    if let Some(original) = data.original_deleter {
        original(agent);
    } else if data.is_weapon {
        drop(unsafe { Box::from_raw(agent.as_weapon_mut()) })
    } else {
        drop(unsafe { Box::from_raw(agent.as_fighter_mut()) })
    }
}

extern "C" fn set_status_scripts(agent: &mut L2CFighterWrapper) {
    let data = vtables::vtable_custom_data::<_, L2CFighterWrapper>(agent.deref());
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
    let Some(list) = statuses.get(&hash) else {
        return;
    };

    let old_total = if is_new {
        0
    } else {
        agent.0.global_table.try_table().unwrap()[0xC]
            .try_integer()
            .unwrap_or_default() as i32
    };

    let mut max_new = old_total;
    for status in list.iter() {
        let StatusScriptId::New(new) = &status.id else {
            continue;
        };

        max_new = max_new.max(old_total + *new + 1);
    }

    agent.0.global_table.try_table_mut().unwrap()[0xC] = smash::lib::L2CValue::new(max_new);

    for status in list.iter() {
        use StatusScriptFunction::*;
        let StatusScriptId::New(new) = &status.id else {
            continue;
        };

        macro_rules! set {
            ($($i:ident),*) => {
                match status.function {
                    $(
                        $i(f) => {
                            let id = smash::lib::L2CValue::new(old_total + *new);
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

    for status in list.iter() {
        use StatusScriptFunction::*;
        let StatusScriptId::Replace { id, original } = &status.id else {
            continue;
        };

        macro_rules! set {
            ($($i:ident),*) => {
                match status.function {
                    $(
                        $i(f) => {
                            let id = smash::lib::L2CValue::new(resolve_lua_const(&id));
                            let condition = smash::lib::L2CValue::new(StatusLine::$i as i32);
                            let original_fn = agent.0.sv_get_status_func(&id, &condition).try_pointer().unwrap().cast();
                            *original.write() = original_fn;
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
    original_deleter: Option<extern "C" fn(&mut L2CFighterWrapper)>,
    original_set_status_scripts: Option<extern "C" fn(&mut L2CFighterWrapper)>,
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

#[skyline::hook(offset = 0x64bbb0)]
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

    let wrapper: &'static mut L2CFighterWrapper = unsafe { std::mem::transmute(agent) };

    let original_deleter = wrapper.vtable_accessor().get_deleter();
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
    data.original_deleter = Some(original_deleter);
    data.original_set_status_scripts = original_set_status_scripts;

    Some(unsafe { std::mem::transmute(wrapper) })
}

#[skyline::hook(offset = 0x33ab4b0)]
fn create_agent_status_weapon(
    object: &mut BattleObject,
    boma: &mut BattleObjectModuleAccessor,
    lua_state: *mut lua_State,
) -> Option<&'static mut L2CFighterBase> {
    let Some(name) = LOWERCASE_WEAPON_NAMES.get(object.kind as usize) else {
        return call_original!(object, boma, lua_state);
    };

    let Some(owner_name) = LOWERCASE_WEAPON_OWNER_NAMES.get(object.kind as usize) else {
        return call_original!(object, boma, lua_state);
    };

    let (is_new, agent) = if let Some(agent) = call_original!(object, boma, lua_state) {
        (false, agent)
    } else {
        let mut weapon = Box::new(std::mem::MaybeUninit::zeroed());
        unsafe {
            weapon_common_ctor(weapon.as_mut_ptr(), object, boma, lua_state);
            (true, Box::leak(weapon.assume_init()) as _)
        }
    };

    let wrapper: &'static mut L2CFighterWrapper = unsafe { std::mem::transmute(agent) };

    let original_deleter = wrapper.vtable_accessor().get_deleter();
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
    data.original_deleter = Some(original_deleter);
    data.original_set_status_scripts = original_set_status_scripts;

    Some(unsafe { std::mem::transmute(wrapper) })
}

pub(crate) fn agent_hash(fighter: &L2CFighterBase) -> Hash40 {
    let wrapper: &'static L2CFighterWrapper = unsafe { std::mem::transmute(fighter) };
    vtables::vtable_custom_data::<_, L2CFighterWrapper>(wrapper.deref()).hash
}

#[skyline::hook(offset = 0x33b5b80, inline)]
unsafe fn enable_lua_module(ctx: &mut InlineCtx) {
    *ctx.registers[8].x.as_mut() =
        skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 + 0x33b8410;
}

pub fn install_status_create_agent_hooks() {
    skyline::install_hooks! {
        create_agent_status_fighter,
        create_agent_status_weapon,
        enable_lua_module
    }
}

create_agent_hook! {
    0x64c2f0 => (Game, fighter);
    0x64c910 => (Effect, fighter);
    0x64cf30 => (Expression, fighter);
    0x65d550 => (Sound, fighter);
    0x33ac3b0 => (Game, weapon);
    0x33ad310 => (Effect, weapon);
    0x33ae270 => (Sound, weapon);
}

create_agent_hook! {
    share;
    0x64db70 => (Game, share_fighter);
    0x64e280 => (Effect, share_fighter);
    0x64e8b0 => (Expression, share_fighter);
    0x64eea0 => (Sound, share_fighter);
}
