use acmd_engine::action::Action;
use smash::app::lua_bind::WorkModule;

#[cfg(feature = "bevy_reflect")]
use bevy_reflect::prelude::*;

use crate::decl_action;
use crate::lua_const::LuaConst;

decl_action!(
    #[derive(Default)]
    pub struct OnFlag(LuaConst);
);
decl_action!(
    #[derive(Default)]
    pub struct OffFlag(LuaConst);
);
decl_action!(
    #[derive(Default)]
    pub struct SetInt {
        variable: LuaConst,
        value: i32,
    }
);
decl_action!(
    #[derive(Default)]
    pub struct SetFloat {
        variable: LuaConst,
        value: f32,
    }
);
decl_action!(
    #[derive(Default)]
    pub struct SetFlag {
        variable: LuaConst,
        value: bool,
    }
);

impl Action for OnFlag {
    const NAME: &'static str = "Work.on_flag";

    #[cfg(feature = "bevy_reflect")]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        unimplemented!()
    }

    #[cfg(not(feature = "bevy_reflect"))]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        WorkModule::on_flag(context.module_accessor, self.0.value);
    }
}

impl Action for OffFlag {
    const NAME: &'static str = "Work.off_flag";

    #[cfg(feature = "bevy_reflect")]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        unimplemented!()
    }

    #[cfg(not(feature = "bevy_reflect"))]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        WorkModule::off_flag(context.module_accessor, self.0.value);
    }
}

impl Action for SetInt {
    const NAME: &'static str = "Work.set_int";

    #[cfg(feature = "bevy_reflect")]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        unimplemented!()
    }

    #[cfg(not(feature = "bevy_reflect"))]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        WorkModule::set_int(context.module_accessor, self.value, self.variable.value);
    }
}

impl Action for SetFloat {
    const NAME: &'static str = "Work.set_float";

    #[cfg(feature = "bevy_reflect")]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        unimplemented!()
    }

    #[cfg(not(feature = "bevy_reflect"))]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        WorkModule::set_float(context.module_accessor, self.value, self.variable.value);
    }
}

impl Action for SetFlag {
    const NAME: &'static str = "Work.set_flag";

    #[cfg(feature = "bevy_reflect")]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        unimplemented!()
    }

    #[cfg(not(feature = "bevy_reflect"))]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        WorkModule::set_flag(context.module_accessor, self.value, self.variable.value);
    }
}
