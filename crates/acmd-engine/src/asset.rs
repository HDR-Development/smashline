use hash40::Hash40;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::action::{ActionError, ActionRegistry};

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum Category {
    Game,
    Effect,
    Sound,
    Expression,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "version")]
pub enum VersionedSmashlineScript {
    #[serde(alias = "0.1.0")]
    #[serde(rename = "0.1.0")]
    V0(SmashlineScriptV0),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SmashlineScriptV0 {
    pub agent: Hash40,
    pub category: Category,
    pub replace: Hash40,
    pub actions: Vec<Value>,
}

impl VersionedSmashlineScript {
    pub fn from_latest(
        registry: &ActionRegistry,
        latest: &crate::SmashlineScript,
    ) -> Result<Self, ActionError> {
        let v0 = SmashlineScriptV0 {
            agent: latest.agent,
            category: latest.category,
            replace: latest.replace,
            actions: latest
                .actions
                .iter()
                .map(|action| registry.as_value(action))
                .collect::<Result<Vec<_>, _>>()?,
        };

        Ok(VersionedSmashlineScript::V0(v0))
    }
}
