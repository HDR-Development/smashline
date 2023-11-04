use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
#[derive(Debug, Clone, Default)]
pub struct LuaConst {
    pub name: String,
    #[cfg_attr(feature = "bevy_reflect", reflect(ignore))]
    pub value: i32,
}

impl Serialize for LuaConst {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.name.serialize(serializer)
    }
}

#[cfg_attr(feature = "bevy_reflect", allow(unused_mut))]
impl<'de> Deserialize<'de> for LuaConst {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let name = String::deserialize(deserializer)?;

        let mut value = 0i32;
        #[cfg(not(feature = "bevy_reflect"))]
        {
            if unsafe {
                !smash::lib::lua_bind_get_value(lua_bind_hash::lua_bind_hash_str(&name), &mut value)
            } {
                return Err(<D::Error as serde::de::Error>::custom(format!(
                    "failed to get lua bind value for {name}"
                )));
            }
        }

        Ok(Self { name, value })
    }
}
