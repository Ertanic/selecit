use std::collections::HashMap;
use crate::modules::{Args, ExecuteResult, Module, ModuleArg};

pub struct AgentVersionModule;

unsafe impl Send for AgentVersionModule {}

#[async_trait::async_trait]
impl Module for AgentVersionModule {
    fn name(&self) -> &'static str {
        "agent-version"
    }

    fn description(&self) -> &'static str {
        "get agent version"
    }

    fn args(&self) -> Vec<ModuleArg> {
        vec![]
    }

    async fn execute(&self, _: Args) -> ExecuteResult {
        ExecuteResult {
            code: 0,
            output: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}
