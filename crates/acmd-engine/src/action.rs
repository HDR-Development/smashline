use std::{any::Any, collections::BTreeMap};

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use smash::lua2cpp::L2CAgentBase;
use thiserror::Error;

#[repr(C)]
#[derive(Error, Debug)]
pub enum ActionError {
    #[error("{0}")]
    ParseError(serde_json::Error),

    #[error("{0}")]
    SerializeError(serde_json::Error),

    #[error("Action '{name}' is not registered")]
    NotRegistered { name: String },

    #[error("Maps must contain exactly 1 key-value pair")]
    InvalidMap,

    #[error("Action has invalid format")]
    InvalidFormat,

    #[error("Action '{have}' is not action '{expected}'")]
    InvalidType {
        have: String,
        expected: &'static str,
    },

    #[error("Failed to convert '{name}' into JSON value: {error}")]
    IntoValueError {
        name: &'static str,
        error: serde_json::Error,
    },

    #[error("Failed to convert JSON vlaue into '{name}': {error}")]
    FromValueError {
        name: &'static str,
        error: serde_json::Error,
    },
}

#[repr(C)]
pub struct DynamicAction {
    pub name: String,
    pub action: Box<dyn Any + Send + Sync + 'static>,
}

pub trait Action: Serialize + DeserializeOwned + Clone + Send + Sync + 'static {
    const NAME: &'static str;

    unsafe fn execute(&self, context: &mut L2CAgentBase);
}

fn into_value_impl<A: Action>(action: &DynamicAction) -> Result<Value, ActionError> {
    let real_action =
        action
            .action
            .downcast_ref::<A>()
            .ok_or_else(|| ActionError::InvalidType {
                have: action.name.clone(),
                expected: A::NAME,
            })?;

    serde_json::to_value(real_action.clone()).map_err(|e| ActionError::IntoValueError {
        name: A::NAME,
        error: e,
    })
}

fn from_value_impl<A: Action>(value: Value) -> Result<DynamicAction, ActionError> {
    let value: A = serde_json::from_value(value).map_err(|e| ActionError::FromValueError {
        name: A::NAME,
        error: e,
    })?;

    Ok(DynamicAction {
        name: A::NAME.to_string(),
        action: Box::new(value),
    })
}

fn execute_impl<A: Action>(
    action: &DynamicAction,
    context: &mut L2CAgentBase,
) -> Result<(), ActionError> {
    let value = action
        .action
        .downcast_ref::<A>()
        .ok_or_else(|| ActionError::InvalidType {
            have: action.name.clone(),
            expected: A::NAME,
        })?;

    Ok(unsafe { value.execute(context) })
}

#[repr(C)]
pub struct RegisteredAction {
    into_value: fn(&DynamicAction) -> Result<Value, ActionError>,
    from_value: fn(Value) -> Result<DynamicAction, ActionError>,
    execute: fn(&DynamicAction, &mut L2CAgentBase) -> Result<(), ActionError>,
}

#[repr(C)]
pub struct ActionRegistry {
    #[cfg(not(feature = "parking_lot"))]
    pub registry: locks::RwLock<BTreeMap<String, RegisteredAction>>,

    #[cfg(feature = "parking_lot")]
    pub registry: parking_lot::RwLock<BTreeMap<String, RegisteredAction>>,
}

impl ActionRegistry {
    pub const fn new() -> Self {
        #[cfg(not(feature = "parking_lot"))]
        {
            Self {
                registry: locks::RwLock::new(BTreeMap::new()),
            }
        }

        #[cfg(feature = "parking_lot")]
        {
            Self {
                registry: parking_lot::RwLock::new(BTreeMap::new()),
            }
        }
    }

    pub fn register<A: Action>(&self) -> &Self {
        self.registry.write().insert(
            A::NAME.to_string(),
            RegisteredAction {
                into_value: into_value_impl::<A>,
                from_value: from_value_impl::<A>,
                execute: execute_impl::<A>,
            },
        );
        self
    }

    pub fn as_action(&self, value: Value) -> Result<DynamicAction, ActionError> {
        let (name, value) = match value {
            Value::String(string) => (string, Value::Null),
            Value::Object(object) => {
                if object.len() != 1 {
                    return Err(ActionError::InvalidMap);
                }

                object.into_iter().next().unwrap()
            }
            _ => return Err(ActionError::InvalidFormat),
        };
        let reg = self.registry.read();

        let Some(action) = reg.get(&name) else {
            return Err(ActionError::NotRegistered { name });
        };

        (action.from_value)(value)
    }

    pub fn as_value(&self, action: &DynamicAction) -> Result<Value, ActionError> {
        let reg = self.registry.read();
        let Some(registered) = reg.get(&action.name) else {
            return Err(ActionError::NotRegistered { name: action.name.to_string() });
        };

        let value = (registered.into_value)(action)?;

        match value {
            Value::Null => Ok(Value::String(action.name.to_string())),
            other => {
                let mut map = serde_json::Map::new();
                map.insert(action.name.to_string(), other);
                Ok(Value::Object(map))
            }
        }
    }

    pub fn execute(
        &self,
        action: &DynamicAction,
        context: &mut L2CAgentBase,
    ) -> Result<(), ActionError> {
        let reg = self.registry.read();
        let Some(registered) = reg.get(&action.name) else {
            return Err(ActionError::NotRegistered { name: action.name.to_string() });
        };

        (registered.execute)(action, context)
    }
}
