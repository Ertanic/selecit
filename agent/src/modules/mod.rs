use crate::proto::ExcavatorCommandArg;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

pub mod info;
pub mod version;

#[derive(Default, Clone)]
pub struct ModulesRegistry(Arc<RwLock<HashMap<&'static str, Arc<dyn Module + Send + Sync>>>>);

impl ModulesRegistry {
    pub async fn register(&mut self, module: impl Module + 'static + Send + Sync) -> &mut ModulesRegistry {
        self.0.write().await.insert(module.name(), Arc::new(module));
        self
    }

    pub async fn get(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        self.0.read().await.get(name).cloned()
    }

    pub async fn get_all(&self) -> Vec<&'static str> {
        self.0.read().await.values().map(|v| v.name()).collect()
    }
}

pub struct ExecuteResult {
    pub code: i64,
    pub output: String,
}

pub struct ModuleArg {
    pub name: &'static str,
    pub description: &'static str,
    pub required: bool,
    pub default: Option<String>,
}

pub struct Args(HashMap<String, String>);

impl Args {
    pub fn new(args: Vec<ExcavatorCommandArg>) -> Self {
        Self(args.into_iter().map(|arg| (arg.key, arg.value)).collect())
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.0.get(key)
    }
}

#[async_trait::async_trait]
pub trait Module {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn args(&self) -> Vec<ModuleArg>;
    async fn execute(&self, args: Args) -> ExecuteResult;
}
