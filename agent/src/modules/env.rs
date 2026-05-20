use crate::modules::{Args, ExecuteResult, Module, ModuleArg};
use std::env;

pub struct EnvExplorer;

#[async_trait::async_trait]
impl Module for EnvExplorer {
    fn name(&self) -> &'static str {
        "env"
    }

    fn description(&self) -> &'static str {
        "Explore environment variables"
    }

    fn args(&self) -> Vec<ModuleArg> {
        vec![ModuleArg {
            name: "name",
            description: "env name",
            required: true,
            default: None,
        }]
    }

    async fn execute(&self, args: Args) -> ExecuteResult {
        let Some(name) = args.get("name")
        else {
            return ExecuteResult {
                code: 1,
                output: vec!["no 'name' argument".to_owned()],
            };
        };

        if let Ok(value) = env::var(name) {
            ExecuteResult {
                code: 0,
                output: vec![value],
            }
        }
        else {
            ExecuteResult {
                code: 1,
                output: vec![format!("env '{name}' variable not found")],
            }
        }
    }
}
