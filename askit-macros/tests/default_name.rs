use agent_stream_kit::{async_trait, AgentConfigs, AgentError, AgentValue, AsAgent, AsAgentData};
use agent_stream_kit::AgentContext;
use askit_macros::askit_agent;

#[askit_agent(kind = "Test")]
struct MyAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for MyAgent {
    fn new(
        askit: agent_stream_kit::ASKit,
        id: String,
        def_name: String,
        configs: Option<AgentConfigs>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            data: AsAgentData::new(askit, id, def_name, configs),
        })
    }

    fn data(&self) -> &AsAgentData {
        &self.data
    }

    fn mut_data(&mut self) -> &mut AsAgentData {
        &mut self.data
    }

    async fn process(
        &mut self,
        _ctx: AgentContext,
        _pin: String,
        _value: AgentValue,
    ) -> Result<(), AgentError> {
        Ok(())
    }
}

#[test]
fn default_name_uses_module_path_and_ident() {
    let def = MyAgent::agent_definition();
    assert_eq!(def.name, concat!(module_path!(), "::", stringify!(MyAgent)));
}
