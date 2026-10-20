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
    #[serde(default)]
    pub application: Option<ApplicationAssociation>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct ApplicationIdentity {
    pub platform: String,
    pub value: String,
}
impl ApplicationIdentity {
    pub fn validate(&self) -> Result<(), String> {
        if !["macos", "windows", "linux"].contains(&self.platform.as_str())
            || self.value.trim().is_empty()
            || self.value != self.value.trim()
            || self.value.len() > 4096
            || self.value.chars().any(char::is_control)
        {
            return Err("Invalid application identity".into());
        }
        Ok(())
    }
    pub fn key(&self) -> (String, String) {
        // Windows paths are case insensitive; other native identities retain case.
        let value = if self.platform == "windows" {
            self.value.replace('/', "\\").to_lowercase()
        } else {
            self.value.clone()
        };
        (self.platform.clone(), value)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationAssociation {
    pub name: String,
    pub identities: Vec<ApplicationIdentity>,
    #[serde(default)]
    pub image: Option<String>,
}
/// A context is an exact set of native identities, independent of its display
/// name, icon, identity ordering, and the page's position in the flat collection.
pub type ApplicationContextKey = Vec<(String, String)>;
impl ApplicationAssociation {
    pub fn context_key(&self) -> ApplicationContextKey {
        let mut identities: Vec<_> = self
            .identities
            .iter()
            .map(ApplicationIdentity::key)
            .collect();
        identities.sort();
        identities
    }
}
impl Page {
    /// All unassociated pages share the ordinary/default navigation context.
    pub fn context_key(&self) -> Option<ApplicationContextKey> {
        self.application
            .as_ref()
            .map(ApplicationAssociation::context_key)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageSwitching {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub default_page: Option<String>,
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
    #[serde(default)]
    pub page_switching: PageSwitching,
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
            page_switching: PageSwitching::default(),
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
        let mut identity_contexts = std::collections::HashMap::new();
        for page in &self.pages {
            if page.id.is_empty() || !pages.insert(&page.id) {
                return Err("Page IDs must be nonempty and unique".into());
            }
            for action in page.bindings.values() {
                action_shape(action, 0, &mut action_count)?;
            }
            if let Some(application) = &page.application {
                if application.name.trim().is_empty()
                    || application.name.len() > 512
                    || application.identities.is_empty()
                    || application.identities.len() > 16
                    || application
                        .image
                        .as_ref()
                        .is_some_and(|image| image.len() > 8_000_000)
                {
                    return Err("Invalid page application".into());
                }
                let context = application.context_key();
                let mut local_identities = std::collections::HashSet::new();
                for identity in &application.identities {
                    identity.validate()?;
                    let key = identity.key();
                    if !local_identities.insert(key.clone()) {
                        return Err("Application association repeats an identity".into());
                    }
                }
                for key in local_identities {
                    if identity_contexts
                        .get(&key)
                        .is_some_and(|existing| existing != &context)
                    {
                        return Err("Application contexts have partially overlapping identities; pages for the same application must use the same complete identity set".into());
                    }
                    identity_contexts.insert(key, context.clone());
                }
            }
        }
        if let Some(id) = &self.page_switching.default_page {
            let page = self
                .pages
                .iter()
                .find(|page| &page.id == id)
                .ok_or("Default page does not exist")?;
            if page.application.is_some() {
                return Err("Default page cannot have an application association".into());
            }
        } else if self.page_switching.enabled {
            return Err("Choose a Default page before enabling automatic switching".into());
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
    fn association(platform: &str, value: &str) -> ApplicationAssociation {
        ApplicationAssociation {
            name: "Fixture application".into(),
            identities: vec![ApplicationIdentity {
                platform: platform.into(),
                value: value.into(),
            }],
            image: None,
        }
    }
    #[test]
    fn application_page_defaults_and_round_trip_preserve_extensions_and_foreign_ids() {
        let mut original = serde_json::to_value(Config::default()).unwrap();
        original.as_object_mut().unwrap().remove("page_switching");
        original["pages"][0]
            .as_object_mut()
            .unwrap()
            .remove("application");
        let mut config = Config::decode(&serde_json::to_vec(&original).unwrap()).unwrap();
        assert!(!config.page_switching.enabled);
        assert!(config.pages[0].application.is_none());
        config.pages.push(Page {
            id: "chrome".into(),
            name: "Chrome".into(),
            bindings: BTreeMap::from([(
                "lcd1".into(),
                Action {
                    kind: "vendor_extension".into(),
                    value: "preserved".into(),
                    ..Default::default()
                },
            )]),
            application: Some(association("windows", "C:\\Apps\\Chrome.exe")),
        });
        config.page_switching = PageSwitching {
            enabled: true,
            default_page: Some("home".into()),
        };
        let bytes = serde_json::to_vec(&config).unwrap();
        let decoded = Config::decode(&bytes).unwrap();
        assert_eq!(serde_json::to_vec(&decoded).unwrap(), bytes);
    }
    #[test]
    fn invalid_application_configuration_is_rejected() {
        let mut config = Config::default();
        config.page_switching.enabled = true;
        assert!(config.validate().unwrap_err().contains("Default"));
        config.page_switching.default_page = Some("missing".into());
        assert!(config.validate().is_err());
        config.page_switching.default_page = Some("home".into());
        config.pages[0].application = Some(association("macos", "com.example.Chrome"));
        assert!(config.validate().unwrap_err().contains("association"));
        config.page_switching = PageSwitching::default();
        for identity in [
            ApplicationIdentity {
                platform: "unsupported".into(),
                value: "x".into(),
            },
            ApplicationIdentity {
                platform: "macos".into(),
                value: " ".into(),
            },
            ApplicationIdentity {
                platform: "macos".into(),
                value: "id\nline".into(),
            },
            ApplicationIdentity {
                platform: "macos".into(),
                value: "x".repeat(4097),
            },
        ] {
            config.pages[0].application.as_mut().unwrap().identities = vec![identity];
            assert!(config.validate().is_err());
        }
        config.pages[0].application = Some(association("windows", "C:\\Apps\\Chrome.exe"));
        let mut second = config.pages[0].clone();
        second.id = "second".into();
        second.application = Some(association("windows", "c:/apps/chrome.EXE"));
        config.pages.push(second);
        // Identical normalized associations are pages in the same context.
        assert!(config.validate().is_ok());
        config.pages[1]
            .application
            .as_mut()
            .unwrap()
            .identities
            .push(ApplicationIdentity {
                platform: "macos".into(),
                value: "org.example.chrome".into(),
            });
        assert!(config
            .validate()
            .unwrap_err()
            .contains("partially overlapping"));
        config.pages.pop();
        config.pages[0]
            .application
            .as_mut()
            .unwrap()
            .identities
            .clear();
        assert!(config.validate().is_err());
    }
    #[test]
    fn application_context_identity_is_an_exact_normalized_set_independent_of_metadata() {
        let mut config = Config::default();
        let mut first = config.pages[0].clone();
        first.id = "chrome-main".into();
        let mut application = association("windows", "C:\\Apps\\Chrome.exe");
        application.identities.push(ApplicationIdentity {
            platform: "macos".into(),
            value: "org.example.Chrome".into(),
        });
        first.application = Some(application);
        let mut second = first.clone();
        second.id = "chrome-tools".into();
        second.name = "Tools".into();
        let metadata = second.application.as_mut().unwrap();
        metadata.name = "A different display name".into();
        metadata.image = Some("data:image/png;base64,fixture".into());
        metadata.identities.reverse();
        metadata.identities[1].value = "c:/apps/chrome.EXE".into();
        assert_eq!(first.context_key(), second.context_key());
        config.pages.extend([first, second]);
        config.page_switching = PageSwitching {
            enabled: true,
            default_page: Some("home".into()),
        };
        assert!(config.validate().is_ok());
        let bytes = serde_json::to_vec(&config).unwrap();
        let decoded = Config::decode(&bytes).unwrap();
        assert_eq!(serde_json::to_vec(&decoded).unwrap(), bytes);
        assert_ne!(
            decoded.pages[0].context_key(),
            decoded.pages[1].context_key()
        );
        // Duplicate identities inside a single association remain malformed.
        let identities = &mut config.pages[2].application.as_mut().unwrap().identities;
        identities.push(identities[0].clone());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("repeats an identity"));
    }

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
