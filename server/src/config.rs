//! `.incipit/config.json`: written by the grill session, read by server and guard.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Deserialize, Default, Clone, Debug)]
pub struct RoleRules {
    /// Path prefixes (relative to the project root) the role may write below.
    #[serde(default)]
    pub write_allow: Vec<String>,
    /// Path prefixes the role may never write below. Wins over `write_allow`.
    #[serde(default)]
    pub write_deny: Vec<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
pub struct Config {
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default)]
    pub roles: BTreeMap<String, RoleRules>,
    /// Roles beyond the five core ones that this project uses: `security`, `researcher`.
    #[serde(default)]
    pub optional_roles: Vec<String>,
}

fn default_backend() -> String {
    "files".to_string()
}

pub fn load(project_root: &Path) -> Result<Config, String> {
    let path = project_root.join(".incipit").join("config.json");
    let raw = fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("invalid {}: {e}", path.display()))
}
