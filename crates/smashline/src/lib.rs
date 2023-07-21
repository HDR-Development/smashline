use std::str::Utf8Error;

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
    ExecPost,
    Exit,
    MapCorrection,
    FixCamera,
    FixPosSlow,
    CheckDamage,
    CheckAttack,
    OnChangeLr,
    LeaveStop,
    NotifyEventGimmick,
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
    }

    pub fn install_basic_status_script<T>(
        agent: Hash40,
        status: LuaConst,
        line: i32,
        function: extern "C" fn(&mut T) -> L2CValue,
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
    }

    pub fn install_one_arg_status_script<T>(
        agent: Hash40,
        status: LuaConst,
        line: i32,
        function: extern "C" fn(&mut T, L2CValue) -> L2CValue,
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
    }

    pub fn install_two_arg_status_script<T>(
        agent: Hash40,
        status: LuaConst,
        line: i32,
        function: extern "C" fn(&mut T, L2CValue, L2CValue) -> L2CValue,
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
    }
}
