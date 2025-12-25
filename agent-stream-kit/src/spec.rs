use std::ops::Not;
use std::sync::atomic::AtomicUsize;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::FnvIndexMap;
use crate::askit::ASKit;
use crate::config::AgentConfigs;
use crate::definition::{AgentConfigSpecs, AgentDefinition};
use crate::error::AgentError;

pub type AgentStreams = FnvIndexMap<String, AgentStream>;
pub type AgentStreamSpecs = FnvIndexMap<String, AgentStreamSpec>;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AgentStreamSpec {
    pub name: String,

    pub agents: im::Vector<AgentSpec>,

    pub channels: im::Vector<ChannelSpec>,

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub run_on_start: bool,

    #[serde(flatten)]
    pub extensions: im::HashMap<String, Value>,
}

impl AgentStreamSpec {
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    pub fn add_agent(&mut self, agent: AgentSpec) {
        self.agents.push_back(agent);
    }

    pub fn remove_agent(&mut self, agent_id: &str) {
        self.agents.retain(|agent| agent.id != agent_id);
    }

    // pub fn set_agents(&mut self, agents: im::Vector<AgentSpec>) {
    //     self.agents = agents;
    // }

    // pub fn channels(&self) -> &Vec<ChannelSpec> {
    //     &self.channels
    // }

    pub fn add_channels(&mut self, channel: ChannelSpec) {
        self.channels.push_back(channel);
    }

    pub fn remove_channel(&mut self, channel_id: &str) -> Option<ChannelSpec> {
        if let Some(channel) = self
            .channels
            .iter()
            .find(|channel| channel.id == channel_id)
            .cloned()
        {
            self.channels.retain(|e| e.id != channel_id);
            Some(channel)
        } else {
            None
        }
    }

    // pub fn set_channels(&mut self, channels: Vec<ChannelSpec>) {
    //     self.channels = channels;
    // }

    // pub fn run_on_start(&self) -> bool {
    //     self.run_on_start
    // }

    // pub fn set_run_on_start(&mut self, run_on_start: bool) {
    //     self.run_on_start = run_on_start;
    // }

    // pub fn disable_all_nodes(&mut self) {
    //     for node in self.agents.iter_mut() {
    //         node.enabled = false;
    //     }
    // }

    pub fn to_json(&self) -> Result<String, AgentError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        Ok(json)
    }

    pub fn from_json(json_str: &str) -> Result<Self, AgentError> {
        let stream: AgentStreamSpec = serde_json::from_str(json_str)
            .map_err(|e| AgentError::SerializationError(e.to_string()))?;
        Ok(stream)
    }
}

#[derive(Debug)]
pub struct AgentStream {
    id: String,

    running: bool,

    spec: AgentStreamSpec,
}

impl AgentStream {
    pub fn new(spec: AgentStreamSpec) -> Self {
        Self {
            id: new_id(),
            running: false,
            spec,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn spec(&self) -> &AgentStreamSpec {
        &self.spec
    }

    pub fn spec_mut(&mut self) -> &mut AgentStreamSpec {
        &mut self.spec
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub async fn start(&mut self, askit: &ASKit) -> Result<(), AgentError> {
        if self.running {
            // Already running
            return Ok(());
        }
        self.running = true;

        for agent in self.spec.agents.iter() {
            if agent.disabled {
                continue;
            }
            askit.start_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to start agent {}: {}", agent.id, e);
            });
        }

        Ok(())
    }

    pub async fn stop(&mut self, askit: &ASKit) -> Result<(), AgentError> {
        for agent in self.spec.agents.iter() {
            askit.stop_agent(&agent.id).await.unwrap_or_else(|e| {
                log::error!("Failed to stop agent {}: {}", agent.id, e);
            });
        }
        self.running = false;
        Ok(())
    }
}

pub fn copy_sub_stream(
    agents: &Vec<AgentSpec>,
    channels: &Vec<ChannelSpec>,
) -> (Vec<AgentSpec>, Vec<ChannelSpec>) {
    let mut new_agents = Vec::new();
    let mut agent_id_map = FnvIndexMap::default();
    for agent in agents {
        let new_id = new_id();
        agent_id_map.insert(agent.id.clone(), new_id.clone());
        let mut new_agent = agent.clone();
        new_agent.id = new_id;
        new_agents.push(new_agent);
    }

    let mut new_channels = Vec::new();
    for channel in channels {
        let Some(source) = agent_id_map.get(&channel.source) else {
            continue;
        };
        let Some(target) = agent_id_map.get(&channel.target) else {
            continue;
        };
        let mut new_channel = channel.clone();
        new_channel.id = new_id();
        new_channel.source = source.clone();
        new_channel.target = target.clone();
        new_channels.push(new_channel);
    }

    (new_agents, new_channels)
}

/// Information held by each agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub id: String,

    /// Name of the AgentDefinition.
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub def_name: String,

    /// List of input pin names.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inputs: Option<Vec<String>>,

    /// List of output pin names.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub outputs: Option<Vec<String>>,

    /// Config values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configs: Option<AgentConfigs>,

    /// Config specs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_specs: Option<AgentConfigSpecs>,

    #[deprecated(note = "Use `disabled` instead")]
    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub enabled: bool,

    #[serde(default, skip_serializing_if = "<&bool>::not")]
    pub disabled: bool,

    #[serde(flatten)]
    pub extensions: FnvIndexMap<String, serde_json::Value>,
}

impl AgentSpec {
    pub fn from_def(def: &AgentDefinition) -> Self {
        let mut spec = def.to_spec();
        spec.id = new_id();
        spec
    }
}

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn new_id() -> String {
    return ID_COUNTER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .to_string();
}

// ChannelSpec

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ChannelSpec {
    pub id: String,
    pub source: String,
    pub source_handle: String,
    pub target: String,
    pub target_handle: String,
}
