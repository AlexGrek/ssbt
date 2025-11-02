use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub output: Option<String>,
    pub config: Option<String>,
    pub format: Option<String>,
    pub authentication: Option<String>,
    pub protocol: Option<String>,
    pub dry: Option<bool>,
    pub max_size: Option<u64>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub paths: Option<Vec<String>>,
    pub skip: Option<Vec<String>>,
    pub compress: Option<bool>,
}
