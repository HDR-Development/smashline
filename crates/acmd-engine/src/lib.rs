use action::{ActionError, ActionRegistry, DynamicAction};
use asset::{Category, SmashlineScriptV0, VersionedSmashlineScript};
use hash40::Hash40;
use serde_json::Value;

pub mod action;
pub mod asset;

pub struct SmashlineScript {
    pub agent: Hash40,
    pub category: Category,
    pub replace: Hash40,
    pub actions: Vec<DynamicAction>,
}

impl SmashlineScript {
    fn from_v0(registry: &ActionRegistry, script: SmashlineScriptV0) -> Result<Self, ActionError> {
        Ok(Self {
            agent: script.agent,
            category: script.category,
            replace: script.replace,
            actions: script
                .actions
                .into_iter()
                .map(|action| registry.as_action(action))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub fn from_json(
        registry: &ActionRegistry,
        json: impl AsRef<str>,
    ) -> Result<Self, ActionError> {
        let versioned: VersionedSmashlineScript =
            serde_json::from_str(json.as_ref()).map_err(ActionError::ParseError)?;

        match versioned {
            VersionedSmashlineScript::V0(v0) => Self::from_v0(registry, v0),
        }
    }

    pub fn to_json(&self, registry: &ActionRegistry) -> Result<String, ActionError> {
        let versioned = VersionedSmashlineScript::from_latest(registry, self)?;
        serde_json::to_string_pretty(&versioned).map_err(ActionError::SerializeError)
    }

    pub fn to_json_value(&self, registry: &ActionRegistry) -> Result<Value, ActionError> {
        let versioned = VersionedSmashlineScript::from_latest(registry, self)?;
        serde_json::to_value(versioned).map_err(ActionError::SerializeError)
    }
}
