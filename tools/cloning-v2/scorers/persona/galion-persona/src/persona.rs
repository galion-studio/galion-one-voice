use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn builtin_aliases(id: &str) -> &'static [&'static str] {
    match id {
        "joyful" | "happy" => &["joyful", "happy"],
        "logic" | "reason" => &["logic", "reason"],
        _ => &[],
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersonaFile {
    pub schema: String,
    pub id: String,
    pub display_name: String,
    #[serde(default = "default_gate")]
    pub gate: f64,
    #[serde(default = "default_presence")]
    pub presence: f64,
    #[serde(default)]
    pub persona: PersonaMeta,
    #[serde(default)]
    pub docs_cited: Vec<toml::Value>,
    #[serde(default)]
    pub assumptions: Vec<toml::Value>,
    #[serde(default)]
    pub emotions: Vec<EmotionSpec>,
}

fn default_gate() -> f64 {
    0.83
}
fn default_presence() -> f64 {
    0.20
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PersonaMeta {
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmotionSpec {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub target: Option<f64>,
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub detector: Option<String>,
    #[serde(default)]
    pub source_detail: Option<String>,
    #[serde(default)]
    pub docs: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

impl EmotionSpec {
    pub fn names(&self) -> Vec<String> {
        let mut out = vec![self.id.clone()];
        out.extend(self.aliases.iter().cloned());
        let mut extras: Vec<&str> = builtin_aliases(&self.id).to_vec();
        for a in &self.aliases {
            extras.extend(builtin_aliases(a).iter().copied());
        }
        for extra in extras {
            if !out.iter().any(|x| x == extra) {
                out.push(extra.to_string());
            }
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct Persona {
    pub schema: String,
    pub id: String,
    pub display_name: String,
    pub gate: f64,
    pub presence: f64,
    pub summary: String,
    pub emotions: Vec<EmotionSpec>,
    pub path: PathBuf,
}

impl Persona {
    pub fn in_set(&self, category: &str) -> bool {
        self.spec_for(category).is_some()
    }

    pub fn spec_for(&self, category: &str) -> Option<&EmotionSpec> {
        self.emotions
            .iter()
            .find(|e| e.names().iter().any(|n| n == category))
    }

    pub fn judged_ids(&self) -> Vec<String> {
        self.emotions.iter().map(|e| e.id.clone()).collect()
    }
}

pub fn load_persona(path: &Path) -> Result<Persona, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    let raw: PersonaFile =
        toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    Ok(Persona {
        schema: raw.schema,
        id: raw.id,
        display_name: raw.display_name,
        gate: raw.gate,
        presence: raw.presence,
        summary: raw.persona.summary,
        emotions: raw.emotions,
        path: path.to_path_buf(),
    })
}
