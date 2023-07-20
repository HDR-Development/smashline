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
        status: i32,
        line: i32,
        function: extern "C" fn(&mut T) -> L2CValue,
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
    }

    pub fn install_one_arg_status_script<T>(
        agent: Hash40,
        status: i32,
        line: i32,
        function: extern "C" fn(&mut T, L2CValue) -> L2CValue,
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
    }

    pub fn install_two_arg_status_script<T>(
        agent: Hash40,
        status: i32,
        line: i32,
        function: extern "C" fn(&mut T, L2CValue, L2CValue) -> L2CValue,
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
    }

    pub fn install_check_attack_status_script<T>(
        agent: Hash40,
        status: i32,
        line: i32,
        function: extern "C" fn(
            &mut T,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
            L2CValue,
        ) -> L2CValue,
    ) where
        T: DerefMut<Target = L2CFighterBase>,
    {
    }
}
