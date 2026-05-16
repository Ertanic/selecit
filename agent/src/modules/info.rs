use crate::modules::{Args, ExecuteResult, Module, ModuleArg, ModulesRegistry};
use std::{collections::HashMap, sync::Arc};

pub struct GetInfoModule(ModulesRegistry);

unsafe impl Send for GetInfoModule {}

impl GetInfoModule {
    pub fn new(registry: ModulesRegistry) -> Self {
        Self(registry)
    }
}

#[async_trait::async_trait]
impl Module for GetInfoModule {
    fn name(&self) -> &'static str {
        "get-info"
    }

    fn description(&self) -> &'static str {
        "get information about the agent modules"
    }

    fn args(&self) -> Vec<ModuleArg> {
        vec![
            ModuleArg {
                name: "type",
                description: "type of information to retrieve (modules, etc.)",
                required: true,
                default: None,
            },
            ModuleArg {
                name: "name",
                description: "name of the module to retrieve information about",
                required: false,
                default: None,
            },
        ]
    }

    async fn execute(&self, args: Args) -> ExecuteResult {
        let ty = args.get("type");
        if let Some(ty) = ty {
            match ty.as_str() {
                "modules" => {
                    let modules = self.0.get_all().await;
                    let json = serde_json::to_string(&modules).unwrap();
                    ExecuteResult { code: 0, output: json }
                }
                _ => ExecuteResult {
                    code: 1,
                    output: "unknown type".to_string(),
                },
            }
        }
        else {
            ExecuteResult {
                code: 1,
                output: "missing type parameter".to_string(),
            }
        }
    }
}
