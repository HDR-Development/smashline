use std::num::{NonZeroU64, NonZeroUsize};
use std::ptr::NonNull;
use std::str::Utf8Error;
use std::sync::atomic::{AtomicUsize, Ordering};

pub use smashline_macro::*;

mod builder;

pub use builder::*;

#[cfg(all(not(feature = "smash-rs"), feature = "skyline_smash"))]
pub use smash::{
    lib::{utility::Variadic, L2CValue},
    lua2cpp::{L2CAgentBase, L2CFighterBase, L2CFighterCommon, L2CWeaponCommon},
    phx::Hash40,
};

#[cfg(feature = "skyline_smash")]
pub use smash as skyline_smash;

#[cfg(feature = "smash-rs")]
pub use smash_rs::{
    self,
    lib::{utility::Variadic, L2CValueHack as L2CValue},
    lua2cpp::{L2CAgentBase, L2CFighterBase, L2CFighterCommon, L2CWeaponCommon},
    phx::Hash40,
};

pub use locks;

#[repr(C)]
#[derive(Copy, Clone, PartialEq, PartialOrd)]
pub enum Priority {
    Low,
    Default,
    High,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
       match self {
           Priority::Low => write!(f, "Low Priority"),
           Priority::Default => write!(f, "Default Priority"),
           Priority::High => write!(f, "High Priority"),
       }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum Acmd {
    Game,
    Effect,
    Sound,
    Expression,
}

impl std::fmt::Display for Acmd {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
       match self {
           Acmd::Game => write!(f, "Category Game"),
           Acmd::Effect => write!(f, "Category Effect"),
           Acmd::Sound => write!(f, "Category Sound"),
           Acmd::Expression => write!(f, "Category Expression"),
       }
    }
}

impl PartialEq<acmd_engine::asset::Category> for Acmd {
    fn eq(&self, other: &acmd_engine::asset::Category) -> bool {
        use acmd_engine::asset::Category;
        match other {
            Category::Effect => matches!(self, Acmd::Effect),
            Category::Game => matches!(self, Acmd::Game),
            Category::Sound => matches!(self, Acmd::Sound),
            Category::Expression => matches!(self, Acmd::Expression),
        }
    }
}

#[repr(i32)]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum StatusLine {
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
    CalcParam,

    MainLoop = -1,
}

#[repr(i32)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ObjectEvent {
    Initialize,
    Finalize,
    Start,
    End,
}

#[derive(Debug)]
pub enum StringFFIError {
    Null,
    UTF8(Utf8Error),
}

#[repr(C)]
pub struct StringFFI {
    ptr: *mut u8,
    len: usize,
}

impl StringFFI {
    pub fn as_str(&self) -> Result<&str, StringFFIError> {
        if self.len == 0 {
            return Ok("");
        }

        if self.ptr.is_null() {
            return Err(StringFFIError::Null);
        }

        unsafe {
            let slice = std::slice::from_raw_parts(self.ptr, self.len);
            std::str::from_utf8(slice).map_err(StringFFIError::UTF8)
        }
    }

    pub fn from_str(value: impl Into<String>) -> Self {
        let mut bytes: String = value.into();
        bytes.shrink_to_fit();
        let leaked = bytes.into_bytes().leak();
        Self {
            ptr: leaked.as_mut_ptr(),
            len: leaked.len(),
        }
    }
}

impl Drop for StringFFI {
    fn drop(&mut self) {
        if self.ptr.is_null() && self.len != 0 {
            panic!("cannot drop null string");
        }

        unsafe {
            drop(String::from_raw_parts(self.ptr, self.len, self.len));
        }
    }
}

#[repr(C)]
pub enum LuaConst {
    Resolved(i32),
    UnresolvedHash(u64),
    UnresolvedStr(StringFFI),
}

pub trait AsHash40 {
    fn as_hash40(self) -> Hash40;
}

impl AsHash40 for String {
    fn as_hash40(self) -> Hash40 {
        Hash40::new(self.as_str())
    }
}

impl AsHash40 for &str {
    fn as_hash40(self) -> Hash40 {
        Hash40::new(self)
    }
}

#[cfg(all(not(feature = "smash-rs"), feature = "skyline_smash"))]
impl AsHash40 for u64 {
    fn as_hash40(self) -> Hash40 {
        Hash40::new_raw(self)
    }
}

#[cfg(feature = "smash-rs")]
impl AsHash40 for u64 {
    fn as_hash40(self) -> Hash40 {
        Hash40(self)
    }
}

impl AsHash40 for Hash40 {
    fn as_hash40(self) -> Hash40 {
        self
    }
}

pub trait IntoLuaConst {
    fn into_lua_const(self) -> LuaConst;
}

impl IntoLuaConst for i32 {
    fn into_lua_const(self) -> LuaConst {
        LuaConst::Resolved(self)
    }
}

impl IntoLuaConst for u64 {
    fn into_lua_const(self) -> LuaConst {
        LuaConst::UnresolvedHash(self)
    }
}

impl IntoLuaConst for &str {
    fn into_lua_const(self) -> LuaConst {
        LuaConst::UnresolvedStr(StringFFI::from_str(self))
    }
}

impl IntoLuaConst for String {
    fn into_lua_const(self) -> LuaConst {
        LuaConst::UnresolvedStr(StringFFI::from_str(self))
    }
}

#[cfg(feature = "skyline_smash")]
impl IntoLuaConst for smash::lib::LuaConst {
    fn into_lua_const(self) -> LuaConst {
        LuaConst::Resolved(*self)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum BattleObjectCategory {
    Fighter = 0,
    Weapon,
    Enemy,
    Gimmick,
    Item,
}

impl BattleObjectCategory {
    pub fn from_battle_object_id(id: u32) -> Option<Self> {
        match id >> 0x1C {
            0x0 => Some(Self::Fighter),
            0x1 => Some(Self::Weapon),
            0x2 => Some(Self::Enemy),
            0x3 => Some(Self::Gimmick),
            0x4 => Some(Self::Item),
            _ => None,
        }
    }
}

macro_rules! decl_imports {
    ($($V:vis fn $name:ident($($arg:ident: $T:ty),*) $(-> $Ret:ty)?;)*) => {
        $(
            $V fn $name($($arg: $T),*) $(-> $Ret)? {
                static CACHED_ADDR: AtomicUsize = AtomicUsize::new(0);
                if CACHED_ADDR.load(Ordering::Acquire) == 0 {
                    let mut value = 0usize;
                    let res = unsafe { skyline::nn::ro::LookupSymbol(&mut value, concat!(stringify!($name), "\0").as_ptr() as _) };
                    if res != 0 {
                        panic!("Failed to lookup '{}': {:#x}", stringify!($name), res);
                    } else if value == 0 {
                        panic!("Could not find '{}', please install the smashline plugin!", stringify!($name));
                    }
                    CACHED_ADDR.store(value, Ordering::Release);
                }

                let addr = CACHED_ADDR.load(Ordering::Acquire);
                let callable: extern "C" fn($($T),*) $(-> $Ret)? = unsafe {
                    std::mem::transmute(addr)
                };

                callable($($arg),*)
            }
        )*
    }
}

decl_imports! {
    fn smashline_get_original_acmd(fighter: &mut L2CAgentBase, name: Hash40) -> Option<AcmdFunction>;
    fn smashline_get_original_status(fighter: &mut L2CFighterBase, line: StatusLine, status: i32) -> Option<NonNull<()>>;

    fn smashline_reload_script(fighter: StringFFI, weapon: StringFFI, file_name: StringFFI);

    fn smashline_get_action_registry() -> &'static acmd_engine::action::ActionRegistry;

    fn smashline_install_acmd_script(
        agent: Hash40,
        script: Hash40,
        category: Acmd,
        priority: Priority,
        function: unsafe extern "C" fn(&mut L2CAgentBase)
    );

    fn smashline_install_status_script(
        agent: Option<NonZeroU64>,
        status: i32,
        line: StatusLine,
        function: *const ()
    );

    fn smashline_install_line_callback(
        agent: Option<NonZeroU64>,
        line: StatusLine,
        callback: *const ()
    );

    fn smashline_get_target_function(
        name: StringFFI,
        offset: usize
    ) -> Option<NonZeroUsize>;

    fn smashline_install_symbol_hook(
        symbol: StringFFI,
        replacement: *const (),
        original: &'static locks::RwLock<*const ()>
    );

    fn smashline_install_state_callback(
        agent: Option<NonZeroU64>,
        event: ObjectEvent,
        callback: *const ()
    );

    fn smashline_clone_weapon(
        original_owner: StringFFI,
        original_name: StringFFI,
        new_owner: StringFFI,
        new_name: StringFFI,
        use_original_code: bool
    );

    fn smashline_add_param_object(
        fighter_name: StringFFI,
        object: StringFFI
    );
}

pub fn original_acmd(agent: &mut L2CAgentBase, name: Hash40) -> AcmdFunction {
    smashline_get_original_acmd(agent, name)
        .unwrap_or_else(|| panic!("Failed to get original ACMD for {name:#?}"))
}

pub fn original_status<L: StatusLineMarker, T>(
    _line: L,
    fighter: &mut T,
    kind: i32,
) -> L::Function<T> {
    unsafe {
        L::cast_ptr(
            smashline_get_original_status(std::mem::transmute(fighter), L::LINE, kind)
                .unwrap_or_else(|| panic!("Failed to get original status for {kind}"))
                .as_ptr(),
        )
    }
}

pub fn clone_weapon(
    original_owner: impl Into<String>,
    original_name: impl Into<String>,
    new_owner: impl Into<String>,
    new_name: impl Into<String>,
    use_original_code: bool,
) {
    smashline_clone_weapon(
        StringFFI::from_str(original_owner),
        StringFFI::from_str(original_name),
        StringFFI::from_str(new_owner),
        StringFFI::from_str(new_name),
        use_original_code,
    );
}

pub fn add_param_object(fighter: impl Into<String>, object: impl Into<String>) {
    smashline_add_param_object(StringFFI::from_str(fighter), StringFFI::from_str(object));
}

pub mod api {
    use super::*;

    #[cfg(all(not(feature = "smash-rs"), feature = "skyline_smash"))]
    fn extract_hash(hash: Hash40) -> u64 {
        hash.hash
    }

    use acmd_engine::action::Action;
    #[cfg(feature = "skyline_smash")]
    pub use smash as skyline_smash;

    #[cfg(feature = "smash-rs")]
    fn extract_hash(hash: Hash40) -> u64 {
        hash.0
    }

    pub fn reload_script(fighter: &str, weapon: Option<&str>, file: &str) {
        smashline_reload_script(
            StringFFI::from_str(fighter),
            StringFFI::from_str(weapon.unwrap_or("")),
            StringFFI::from_str(file),
        );
    }

    pub fn register_action<A: Action>() {
        smashline_get_action_registry().register::<A>();
    }

    pub fn install_status_script(
        agent: Option<Hash40>,
        line: StatusLine,
        kind: i32,
        function: *const (),
    ) {
        let agent = agent.and_then(|x| NonZeroU64::new(extract_hash(x)));
        smashline_install_status_script(agent, kind, line, function);
    }

    pub fn install_line_callback(agent: Option<Hash40>, line: StatusLine, function: *const ()) {
        let agent = agent.and_then(|x| NonZeroU64::new(extract_hash(x)));
        smashline_install_line_callback(agent, line, function);
    }

    pub fn install_acmd_script(
        agent: Hash40,
        script: Hash40,
        category: Acmd,
        priority: Priority,
        function: unsafe extern "C" fn(&mut L2CAgentBase),
    ) {
        smashline_install_acmd_script(agent, script, category, priority, function);
    }

    pub fn get_target_function(module_name: impl Into<String>, offset: usize) -> Option<usize> {
        smashline_get_target_function(StringFFI::from_str(module_name), offset).map(|x| x.get())
    }

    pub fn install_symbol_hook(
        module_name: impl Into<String>,
        replacement: *const (),
        original: &'static locks::RwLock<*const ()>,
    ) {
        unsafe {
            smashline_install_symbol_hook(
                StringFFI::from_str(module_name),
                std::mem::transmute(replacement),
                std::mem::transmute(original),
            );
        }
    }

    pub fn install_state_callback(agent: Option<Hash40>, event: ObjectEvent, function: *const ()) {
        smashline_install_state_callback(
            agent.and_then(|x| NonZeroU64::new(extract_hash(x))),
            event,
            function,
        );
    }
}
