use agent_stream_kit::AgentContext;
use agent_stream_kit::{AgentConfigs, AgentError, AgentValue, AsAgent, AsAgentData, async_trait};
use askit_macros::askit_agent;
use std::collections::HashMap;

const UNIT_KEY: &str = "unit";
const BOOL_KEY: &str = "bool";
const INT_KEY: &str = "int";
const NUM_KEY: &str = "num";
const STR_KEY: &str = "str";
const TXT_KEY: &str = "txt";
const OBJ_KEY: &str = "obj";

#[askit_agent(
    kind = "Test",
    title = "Config Agent",
    category = "Tests",
    unit_config(name = UNIT_KEY),
    boolean_config(name = BOOL_KEY, default = true, title = "Bool Title"),
    integer_config(name = INT_KEY, default = 7),
    number_config(name = NUM_KEY, default = 3.14, description = "pi"),
    string_config(name = STR_KEY, default = "hello"),
    text_config(name = TXT_KEY, default = "long"),
    object_config(
        name = OBJ_KEY,
        default = AgentValue::object_default(),
        title = "Obj",
        description = "Obj desc"
    )
)]
struct ConfigAgent {
    data: AsAgentData,
}

#[async_trait]
impl AsAgent for ConfigAgent {
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
fn config_entries_are_generated() {
    let def = ConfigAgent::agent_definition();
    let configs: HashMap<_, _> = def
        .default_configs
        .expect("default configs")
        .into_iter()
        .collect();

    assert_eq!(configs[UNIT_KEY].type_.as_deref(), Some("unit"));
    assert_eq!(configs[UNIT_KEY].value, AgentValue::unit());

    let bool_entry = &configs[BOOL_KEY];
    assert_eq!(bool_entry.type_.as_deref(), Some("boolean"));
    assert_eq!(bool_entry.value, AgentValue::boolean(true));
    assert_eq!(bool_entry.title.as_deref(), Some("Bool Title"));

    assert_eq!(configs[INT_KEY].value, AgentValue::integer(7));
    assert_eq!(configs[NUM_KEY].description.as_deref(), Some("pi"));
    assert_eq!(configs[STR_KEY].value, AgentValue::string("hello"));
    assert_eq!(configs[TXT_KEY].value, AgentValue::string("long"));

    let obj_entry = &configs[OBJ_KEY];
    assert_eq!(obj_entry.type_.as_deref(), Some("object"));
    assert_eq!(obj_entry.title.as_deref(), Some("Obj"));
    assert_eq!(obj_entry.description.as_deref(), Some("Obj desc"));
}
