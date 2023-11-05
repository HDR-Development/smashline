use std::fmt::Display;

use acmd_engine::action::Action;
pub use serde;

pub mod attack;
pub mod lua_const;
pub mod work;

#[cfg(feature = "bevy_reflect")]
use bevy_reflect::prelude::*;

#[macro_export]
macro_rules! decl_action {
    (
        $(#[$outer:meta])*
        $vis:vis struct $Name:ident $($t:tt)*
    ) => {
        #[cfg_attr(feature = "bevy_reflect", derive(::bevy_reflect::Reflect))]
        #[cfg_attr(feature = "bevy_reflect", reflect(Default))]
        #[derive($crate::serde::Deserialize, $crate::serde::Serialize, Debug, Clone)]
        $(#[$outer])*
        pub struct $Name $($t)*
    };
    (
        $(#[$outer:meta])*
        $vis:vis enum $Name:ident $($t:tt)*
    ) => {
        #[cfg_attr(feature = "bevy_reflect", derive(::bevy_reflect::Reflect))]
        #[cfg_attr(feature = "bevy_reflect", reflect(Default))]
        #[derive($crate::serde::Deserialize, $crate::serde::Serialize, Debug, Clone)]
        $(#[$outer])*
        pub enum $Name $($t)*
    }
}

#[macro_export]
macro_rules! impl_action {
    (
        $id:ident($name:expr) {
            $($t:tt)*
        }
    ) => {
        impl Action for $id {
            const NAME: &'static str = $name;

            unsafe fn execute(&self, fighter: &mut smash::lua2cpp::L2CAgentBase) {
                #[cfg(feature = "bevy_reflect")]
                {
                    unimplemented!()
                }
                #[cfg(not(feature = "bevy_reflect"))]
                {
                    $($t)*
                }
            }
        }
    }
}

decl_action! {
    #[derive(Copy, Default)]
    pub struct WaitUntil(f32);
}

impl Action for WaitUntil {
    const NAME: &'static str = "WaitUntil";

    #[cfg(feature = "bevy_reflect")]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        unimplemented!()
    }

    #[cfg(not(feature = "bevy_reflect"))]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        smash::app::sv_animcmd::frame(context.lua_state_agent, self.0);
    }
}

decl_action! {
    #[derive(Copy, Default)]
    pub struct WaitFor(f32);
}

impl Action for WaitFor {
    const NAME: &'static str = "WaitFor";

    #[cfg(feature = "bevy_reflect")]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        unimplemented!()
    }

    #[cfg(not(feature = "bevy_reflect"))]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        smash::app::sv_animcmd::wait(context.lua_state_agent, self.0);
    }
}

pub fn register_defaults() {
    macro_rules! register {
        ($($action:path),*) => {
            $(
                smashline::api::register_action::<$action>();
            )*
        }
    }

    register! {
        WaitUntil,
        WaitFor,
        work::OnFlag,
        work::OffFlag,
        work::SetInt,
        work::SetFloat,
        work::SetFlag,
        attack::Attack,
        attack::AttackClear,
        attack::AttackClearAll
    }
}

#[cfg(not(feature = "bevy_reflect"))]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SerdeHash40(pub hash40::Hash40);

#[cfg(feature = "bevy_reflect")]
#[derive(
    bevy_reflect::Reflect,
    serde::Serialize,
    serde::Deserialize,
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Default,
)]
#[reflect_value(Default)]
pub struct SerdeHash40(pub hash40::Hash40);

impl SerdeHash40 {
    pub const fn new(string: &str) -> Self {
        Self(hash40::hash40(string))
    }
}

impl Display for SerdeHash40 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}
