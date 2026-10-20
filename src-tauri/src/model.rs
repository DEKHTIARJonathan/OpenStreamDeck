use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Action {
    pub kind: String,
    pub value: String,
    pub title: String,
    pub color: String,
    pub image: Option<String>,
    pub children: Vec<Action>,
    pub options: BTreeMap<String, String>,
}
impl Default for Action {
    fn default() -> Self {
        Self {
            kind: "none".into(),
            value: String::new(),
            title: "Unassigned".into(),
            color: "#292b38".into(),
            image: None,
            children: vec![],
            options: BTreeMap::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Page {
    pub id: String,
    pub name: String,
    pub bindings: BTreeMap<String, Action>,
    // Preserve application associations written by another version-2 build.
    // This build does not execute application-based page switching.
    #[serde(default)]
    pub application: Option<serde_json::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<Action>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub endpoint: String,
    pub model: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub pages: Vec<Page>,
    pub active_page: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_switching: Option<serde_json::Value>,
    pub workflows: Vec<Workflow>,
    pub providers: Vec<Provider>,
    pub brightness: u8,
    pub image_size: u32,
    pub rotation: u16,
    pub sleep_minutes: u32,
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default = "default_menu_bar_only")]
    pub menu_bar_only: bool,
}
fn default_menu_bar_only() -> bool {
    true
}
impl Default for Config {
    fn default() -> Self {
        let mut bindings = BTreeMap::new();
        for (key, title, kind, value, color) in [
            ("lcd1", "Terminal", "open", "Terminal", "#254d41"),
            ("lcd2", "Copy", "shortcut", "meta+c", "#35446b"),
            ("lcd3", "Paste", "shortcut", "meta+v", "#35446b"),
            ("lcd4", "Summarize", "workflow", "summarize", "#59427d"),
            ("lcd5", "Agents", "agents", "", "#315c67"),
            ("lcd6", "Play / pause", "media", "play", "#674f33"),
            ("button3", "Sleep", "sleep", "", "#292b38"),
            ("knob1.cw", "Volume +", "media", "volume_up", "#292b38"),
            ("knob1.ccw", "Volume −", "media", "volume_down", "#292b38"),
            ("knob1", "Mute", "media", "mute", "#292b38"),
            ("knob2.cw", "Brightness +", "brightness", "+5", "#292b38"),
            ("knob2.ccw", "Brightness −", "brightness", "-5", "#292b38"),
            ("knob3.cw", "Next agent", "agent_select", "next", "#292b38"),
            (
                "knob3.ccw",
                "Previous agent",
                "agent_select",
                "previous",
                "#292b38",
            ),
            ("knob3", "Focus agent", "agent_focus", "selected", "#292b38"),
        ] {
            bindings.insert(
                key.into(),
                Action {
                    title: title.into(),
                    kind: kind.into(),
                    value: value.into(),
                    color: color.into(),
                    ..Default::default()
                },
            );
        }
        Self {
            version: 2,
            pages: vec![Page {
                id: "home".into(),
                name: "Home".into(),
                bindings,
                application: None,
            }],
            active_page: "home".into(),
            page_switching: None,
            workflows: vec![Workflow {
                id: "summarize".into(),
                name: "Summarize clipboard".into(),
                description: "Turn copied text into a concise summary.".into(),
                steps: vec![
                    Action {
                        kind: "clipboard_read".into(),
                        title: "Read clipboard".into(),
                        ..Default::default()
                    },
                    Action {
                        kind: "ai".into(),
                        title: "Summarize".into(),
                        value: "Summarize the following clearly and concisely:\n\n{{input}}".into(),
                        options: BTreeMap::from([("provider".into(), "openai".into())]),
                        ..Default::default()
                    },
                    Action {
                        kind: "clipboard_write".into(),
                        title: "Copy result".into(),
                        value: "{{input}}".into(),
                        ..Default::default()
                    },
                ],
            }],
            providers: vec![
                Provider {
                    id: "openai".into(),
                    name: "OpenAI".into(),
                    kind: "openai".into(),
                    endpoint: "https://api.openai.com/v1".into(),
                    model: String::new(),
                },
                Provider {
                    id: "anthropic".into(),
                    name: "Anthropic".into(),
                    kind: "anthropic".into(),
                    endpoint: "https://api.anthropic.com/v1".into(),
                    model: String::new(),
                },
                Provider {
                    id: "gemini".into(),
                    name: "Google Gemini".into(),
                    kind: "gemini".into(),
                    endpoint: "https://generativelanguage.googleapis.com/v1beta".into(),
                    model: String::new(),
                },
                Provider {
                    id: "local".into(),
                    name: "Local endpoint".into(),
                    kind: "compatible".into(),
                    endpoint: "http://127.0.0.1:11434/v1".into(),
                    model: String::new(),
                },
            ],
            brightness: 60,
            image_size: 64,
            rotation: 90,
            sleep_minutes: 0,
            launch_at_login: false,
            menu_bar_only: true,
        }
    }
}
impl Config {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let config: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }
    pub fn page(&self) -> Option<&Page> {
        self.pages.iter().find(|p| p.id == self.active_page)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 2 {
            return Err("Unsupported config version".into());
        }
        if self.page().is_none() {
            return Err("Active page does not exist".into());
        }
        if self.brightness > 100
            || !(32..=128).contains(&self.image_size)
            || ![0, 90, 180, 270].contains(&self.rotation)
        {
            return Err("Invalid display settings".into());
        }
        if self.sleep_minutes > 1440 {
            return Err("Sleep delay exceeds 1440 minutes".into());
        }
        let mut action_count = 0;
        fn action_shape(action: &Action, depth: usize, count: &mut usize) -> Result<(), String> {
            *count += 1;
            if depth > 32 || *count > 10_000 {
                return Err("Too many nested actions".into());
            }
            if action
                .image
                .as_ref()
                .is_some_and(|image| image.len() > 8_000_000)
            {
                return Err("Action image exceeds 8 MB".into());
            }
            for child in &action.children {
                action_shape(child, depth + 1, count)?;
            }
            Ok(())
        }
        let mut pages = std::collections::HashSet::new();
        for page in &self.pages {
            if page.id.is_empty() || !pages.insert(&page.id) {
                return Err("Page IDs must be nonempty and unique".into());
            }
            for action in page.bindings.values() {
                action_shape(action, 0, &mut action_count)?;
            }
        }
        let mut workflows = std::collections::HashSet::new();
        for workflow in &self.workflows {
            if workflow.id.is_empty() || !workflows.insert(&workflow.id) {
                return Err("Workflow IDs must be nonempty and unique".into());
            }
            for action in &workflow.steps {
                action_shape(action, 0, &mut action_count)?;
            }
        }
        let mut providers = std::collections::HashSet::new();
        for provider in &self.providers {
            if provider.id.is_empty() || !providers.insert(&provider.id) {
                return Err("Provider IDs must be nonempty and unique".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub connected: bool,
    pub serial: String,
    pub product: String,
    pub message: String,
    pub packets_in: u64,
    pub packets_out: u64,
    pub sleeping: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceEvent {
    pub control: String,
    pub event: String,
    pub code: u8,
    pub state: u8,
    pub at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub source: String,
    pub agent: String,
    pub title: String,
    pub status: String,
    pub raw_status: String,
    pub session_id: Option<String>,
    pub target: String,
    pub updated_at: u64,
    #[serde(default)]
    pub parent_session_id: Option<String>,
    #[serde(default)]
    pub is_subagent: bool,
    pub can_focus: bool,
    pub can_prompt: bool,
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    #[test]
    fn rejects_duplicate_provider_and_workflow_identity() {
        let mut w = Config::default();
        w.providers.push(w.providers[0].clone());
        assert!(w.validate().unwrap_err().contains("Provider IDs"));
        w.providers.pop();
        w.workflows.push(w.workflows[0].clone());
        assert!(w.validate().unwrap_err().contains("Workflow IDs"));
    }
    #[test]
    fn bounds_nesting_and_keeps_unknown_actions() {
        let mut w = Config::default();
        let a = w.pages[0].bindings.get_mut("lcd1").unwrap();
        a.kind = "future-plugin".into();
        assert!(w.validate().is_ok());
        let mut nested = Action::default();
        for _ in 0..34 {
            nested = Action {
                children: vec![nested],
                ..Default::default()
            };
        }
        w.workflows[0].steps.push(nested);
        assert!(w.validate().unwrap_err().contains("nested"));
        w.workflows[0].steps.pop();
        w.sleep_minutes = 1441;
        assert!(w.validate().is_err());
    }
}
