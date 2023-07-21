use std::num::{NonZeroU64, NonZeroUsize};
use std::str::Utf8Error;
use std::sync::atomic::{AtomicUsize, Ordering};

pub use smashline_macro::*;

#[cfg(feature = "skyline_smash")]
pub use smash::{
    lib::{utility::Variadic, L2CValue},
    lua2cpp::{L2CAgentBase, L2CFighterBase},
    phx::Hash40,
};

#[cfg(feature = "smash-rs")]
pub use smash::{
    lib::L2CValueHack as L2CValue,
    lua2cpp::{L2CAgentBase, L2CFighterBase},
    phx::Hash40,
};

pub use locks;

#[repr(C)]
pub enum Priority {
    Default,
    Low,
    High,
}

#[repr(C)]
pub enum Acmd {
    Game,
    Effect,
    Sound,
    Expression,
}

#[repr(i32)]
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

    MainLoop = -1,
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

impl AsHash40 for u64 {
    fn as_hash40(self) -> Hash40 {
        Hash40::new_raw(self)
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
        LuaConst::UnresolvedHash(self.get_lua_hash())
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
    fn smashline_install_acmd_script(
        agent: Hash40,
        script: Hash40,
        category: Acmd,
        priority: Priority,
        function: extern "C" fn(&mut L2CAgentBase, &mut Variadic)
    );

    fn smashline_install_status_script(
        agent: Hash40,
        status: LuaConst,
        line: i32,
        function: *const (),
        original: &'static locks::RwLock<*const ()>
    );

    fn smashline_install_line_callback(
        agent: Option<NonZeroU64>,
        line: i32,
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
}

pub mod api {
    use super::*;
    use std::ops::DerefMut;

    pub fn install_acmd_script(
        agent: Hash40,
        script: Hash40,
        category: Acmd,
        priority: Priority,
        function: extern "C" fn(&mut L2CAgentBase, &mut Variadic),
    ) {
        smashline_install_acmd_script(agent, script, category, priority, function);
    }

    pub fn install_basic_status_script<T>(
        agent: Hash40,
        status: LuaConst,
        line: i32,
        function: extern "C" fn(&mut T) -> L2CValue,
        original: &'static locks::RwLock<extern "C" fn(&mut T) -> L2CValue>,
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
        unsafe {
            smashline_install_status_script(
                agent,
                status,
                line,
                std::mem::transmute(function),
                std::mem::transmute(original),
            );
        }
    }

    pub fn install_one_arg_status_script<T>(
        agent: Hash40,
        status: LuaConst,
        line: i32,
        function: extern "C" fn(&mut T, L2CValue) -> L2CValue,
        original: &'static locks::RwLock<extern "C" fn(&mut T, L2CValue) -> L2CValue>,
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
        unsafe {
            smashline_install_status_script(
                agent,
                status,
                line,
                std::mem::transmute(function),
                std::mem::transmute(original),
            );
        }
    }

    pub fn install_two_arg_status_script<T>(
        agent: Hash40,
        status: LuaConst,
        line: i32,
        function: extern "C" fn(&mut T, L2CValue, L2CValue) -> L2CValue,
        original: &'static locks::RwLock<extern "C" fn(&mut T, L2CValue, L2CValue) -> L2CValue>,
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
        unsafe {
            smashline_install_status_script(
                agent,
                status,
                line,
                std::mem::transmute(function),
                std::mem::transmute(original),
            );
        }
    }

    pub fn install_basic_line_callback<T>(
        agent: Option<Hash40>,
        line: i32,
        function: extern "C" fn(&mut T),
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
        smashline_install_line_callback(
            agent.and_then(|x| NonZeroU64::new(x.hash)),
            line,
            function as *const (),
        );
    }

    pub fn install_one_arg_line_callback<T>(
        agent: Option<Hash40>,
        line: i32,
        function: extern "C" fn(&mut T, L2CValue),
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
        smashline_install_line_callback(
            agent.and_then(|x| NonZeroU64::new(x.hash)),
            line,
            function as *const (),
        );
    }

    pub fn install_two_arg_line_callback<T>(
        agent: Option<Hash40>,
        line: i32,
        function: extern "C" fn(&mut T, L2CValue, L2CValue),
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
        smashline_install_line_callback(
            agent.and_then(|x| NonZeroU64::new(x.hash)),
            line,
            function as *const (),
        );
    }

    pub fn get_target_function(module_name: impl Into<String>, offset: usize) -> Option<usize> {
        smashline_get_target_function(StringFFI::from_str(module_name), offset).map(|x| x.get())
    }

    pub fn install_symbol_hook<T>(
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

    #[doc(hidden)]
    pub extern "C" fn __basic_status_stub<T>(_: &mut T) -> L2CValue {
        panic!("basic status stub called")
    }

    #[doc(hidden)]
    pub extern "C" fn __one_arg_status_stub<T>(_: &mut T, _: L2CValue) -> L2CValue {
        panic!("one arg stub called")
    }

    #[doc(hidden)]
    pub extern "C" fn __two_arg_status_stub<T>(_: &mut T, _: L2CValue, _: L2CValue) -> L2CValue {
        panic!("two arg stub called")
    }
}
