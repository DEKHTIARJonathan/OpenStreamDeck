//! Production orchestration separated from OS/USB/provider effects for exhaustive tests.
use crate::model::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
pub type Outcome<'a> = Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;
pub const ACTION_KINDS: &[&str] = &[
    "none",
    "open",
    "shortcut",
    "text",
    "media",
    "mouse",
    "shell",
    "clipboard_read",
    "clipboard_write",
    "file_read",
    "file_write",
    "ai",
    "workflow",
    "sequence",
    "condition",
    "cycle",
    "delay",
    "brightness",
    "sleep",
    "wake",
    "page",
    "agents",
    "agent_select",
    "agent_focus",
    "agent_prompt",
    "agent_launch",
];
pub trait Host: Sync {
    fn config(&self) -> Config;
    fn next_cycle(&self, key: &str, len: usize) -> usize;
    fn effect<'a>(
        &'a self,
        action: &'a Action,
        input: String,
        cancel: &'a Arc<AtomicBool>,
    ) -> Outcome<'a>;
}
pub fn brightness(current: u8, value: &str) -> Result<u8, String> {
    let n = value
        .parse::<i32>()
        .map_err(|_| "Brightness must be 0–100 or a signed adjustment such as +5")?;
    let n = if value.starts_with('+') || value.starts_with('-') {
        i32::from(current).saturating_add(n)
    } else {
        n
    };
    Ok(n.clamp(0, 100) as u8)
}
pub fn switch_page(w: &mut Config, value: &str) -> Result<(), String> {
    if w.pages.is_empty() {
        return Err("Config has no pages".into());
    }
    let index = w
        .pages
        .iter()
        .position(|p| p.id == w.active_page)
        .ok_or("Active page not found")?;
    let next = match value {
        "next" | "previous" => {
            let context = w.pages[index].context_key();
            let pages: Vec<_> = w
                .pages
                .iter()
                .filter(|page| page.context_key() == context)
                .collect();
            let current = pages
                .iter()
                .position(|page| page.id == w.active_page)
                .ok_or("Active page context not found")?;
            let next = if value == "next" {
                (current + 1) % pages.len()
            } else {
                (current + pages.len() - 1) % pages.len()
            };
            pages[next].id.clone()
        }
        _ => w
            .pages
            .iter()
            .find(|page| page.id == value)
            .ok_or("Page not found")?
            .id
            .clone(),
    };
    w.active_page = next;
    Ok(())
}
pub fn validate(a: &Action, w: &Config) -> Result<(), String> {
    if !ACTION_KINDS.contains(&a.kind.as_str()) {
        return Err(format!("Unsupported action: {}", a.kind));
    }
    let value = a.value.trim();
    if [
        "open",
        "shortcut",
        "shell",
        "file_read",
        "file_write",
        "agent_launch",
        "agent_prompt",
    ]
    .contains(&a.kind.as_str())
        && value.is_empty()
    {
        return Err(format!("{} requires a value", a.kind));
    }
    match a.kind.as_str() {
        "shell" if a.value.contains("{{input}}") => {
            return Err("Shell scripts cannot interpolate {{input}}. Read the OPENSTREAMDECK_INPUT environment variable as data instead.".into());
        }
        "brightness" => {
            brightness(w.brightness, value)?;
        }
        "media"
            if ![
                "play",
                "next",
                "previous",
                "volume_up",
                "volume_down",
                "mute",
            ]
            .contains(&value) =>
        {
            return Err("Unknown media control".into())
        }
        "mouse" if !["left", "right", "middle"].contains(&value) => {
            return Err("Unknown mouse button".into())
        }
        "agent_select" if !["next", "previous"].contains(&value) => {
            return Err("Agent selection must be next or previous".into())
        }
        "agent_launch"
            if !["codex", "claude"].contains(
                &a.options
                    .get("agent")
                    .map(String::as_str)
                    .unwrap_or("codex"),
            ) =>
        {
            return Err("Choose Codex or Claude Code".into())
        }
        "delay" => {
            let n = value
                .parse::<u64>()
                .map_err(|_| "Delay must be milliseconds")?;
            if n > 300_000 {
                return Err("Delay exceeds five minutes".into());
            }
        }
        "shortcut" => crate::actions::validate_shortcut(value)?,
        "ai" => {
            let p = w
                .providers
                .iter()
                .find(|p| Some(&p.id) == a.options.get("provider"))
                .ok_or("Select a provider for this step")?;
            if p.model.trim().is_empty() {
                return Err(format!("Choose a model for {} in Settings", p.name));
            }
        }
        "page" => switch_page(&mut w.clone(), value)?,
        _ => {}
    }
    Ok(())
}
pub fn execute<'a>(
    host: &'a dyn Host,
    a: &'a Action,
    input: String,
    cancel: &'a Arc<AtomicBool>,
    depth: usize,
) -> Outcome<'a> {
    Box::pin(async move {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        if depth > 32 {
            return Err("Action nesting exceeds 32 levels".into());
        }
        let w = host.config();
        let mut prepared = a.clone();
        // Model/file/clipboard output must never become executable shell syntax.
        // Shell steps receive their input through a child environment variable.
        if prepared.kind != "shell" {
            prepared.value = prepared.value.replace("{{input}}", &input);
        }
        validate(&prepared, &w)?;
        match prepared.kind.as_str() {
            "none" => Ok(input),
            "workflow" => {
                let f = w
                    .workflows
                    .iter()
                    .find(|f| f.id == prepared.value)
                    .ok_or("Workflow not found")?;
                let mut result = input;
                for step in &f.steps {
                    result = execute(host, step, result, cancel, depth + 1).await?
                }
                Ok(result)
            }
            "sequence" | "condition" => {
                if prepared.kind == "condition" && !input.contains(&prepared.value) {
                    return Ok(input);
                }
                let mut result = input;
                for step in &prepared.children {
                    result = execute(host, step, result, cancel, depth + 1).await?
                }
                Ok(result)
            }
            "cycle" => {
                if prepared.children.is_empty() {
                    return Err("Cycle has no actions".into());
                }
                let key = serde_json::to_string(a).map_err(|e| e.to_string())?;
                let i = host.next_cycle(&key, prepared.children.len());
                execute(host, &prepared.children[i], input, cancel, depth + 1).await
            }
            "delay" => {
                let duration = std::time::Duration::from_millis(
                    prepared.value.parse().map_err(|_| "Invalid delay")?,
                );
                let sleep = tokio::time::sleep(duration);
                tokio::pin!(sleep);
                loop {
                    tokio::select! {_=&mut sleep=>break,_=tokio::time::sleep(std::time::Duration::from_millis(20))=>{if cancel.load(Ordering::Relaxed){return Err("Cancelled".into())}}}
                }
                Ok(input)
            }
            _ => host.effect(&prepared, input, cancel).await,
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, sync::Mutex};
    struct Fake {
        w: Mutex<Config>,
        seen: Mutex<Vec<Action>>,
        cycles: Mutex<HashMap<String, usize>>,
        fail: bool,
    }
    impl Fake {
        fn new() -> Self {
            let mut w = Config::default();
            w.providers[0].model = "fixture".into();
            Self {
                w: Mutex::new(w),
                seen: Mutex::new(vec![]),
                cycles: Mutex::new(HashMap::new()),
                fail: false,
            }
        }
    }
    impl Host for Fake {
        fn config(&self) -> Config {
            self.w.lock().unwrap().clone()
        }
        fn next_cycle(&self, key: &str, len: usize) -> usize {
            let mut c = self.cycles.lock().unwrap();
            let n = c.entry(key.into()).or_default();
            let index = *n % len;
            *n += 1;
            index
        }
        fn effect<'a>(
            &'a self,
            a: &'a Action,
            input: String,
            _: &'a Arc<AtomicBool>,
        ) -> Outcome<'a> {
            Box::pin(async move {
                self.seen.lock().unwrap().push(a.clone());
                if self.fail {
                    return Err("fixture effect failed".into());
                }
                match a.kind.as_str() {
                    "clipboard_read" => Ok("source".into()),
                    "ai" => Ok(format!("AI:{}", a.value)),
                    "clipboard_write" => Ok(a.value.clone()),
                    "brightness" => {
                        let mut w = self.w.lock().unwrap();
                        w.brightness = brightness(w.brightness, &a.value)?;
                        Ok(input)
                    }
                    "page" => {
                        switch_page(&mut self.w.lock().unwrap(), &a.value)?;
                        Ok(input)
                    }
                    _ => Ok(input),
                }
            })
        }
    }
    fn example(kind: &str) -> Action {
        let mut a = Action {
            kind: kind.into(),
            value: match kind {
                "open" => "https://example.com",
                "shortcut" => "ctrl+shift+c",
                "media" => "play",
                "mouse" => "left",
                "shell" => "printf fixture",
                "file_read" | "file_write" => "/fixture/file",
                "brightness" => "+5",
                "page" => "next",
                "agent_select" => "next",
                "ai" | "agent_prompt" | "agent_launch" => "{{input}}",
                "delay" => "0",
                "workflow" => "summarize",
                _ => "",
            }
            .into(),
            ..Default::default()
        };
        if kind == "ai" {
            a.options.insert("provider".into(), "openai".into());
        }
        if ["sequence", "condition", "cycle"].contains(&kind) {
            a.children.push(Action {
                kind: "text".into(),
                value: "{{input}}".into(),
                ..Default::default()
            });
        }
        a
    }
    #[tokio::test]
    async fn fixed_navigation_cycles_empty_pages_and_ignores_reserved_assignments() {
        let h = Fake::new();
        {
            let mut w = h.w.lock().unwrap();
            w.pages = (0..3)
                .map(|n| Page {
                    id: format!("empty-{n}"),
                    name: format!("Page {n}"),
                    bindings: Default::default(),
                    application: None,
                })
                .collect();
            // Custom/disabled actions must not override either arrow.
            w.pages[1]
                .bindings
                .insert("button1".into(), example("shell"));
            w.pages[2]
                .bindings
                .insert("button2".into(), Action::default());
            w.active_page = w.pages[0].id.clone();
        }
        let before = serde_json::to_value(&h.w.lock().unwrap().pages).unwrap();
        for (button, expected) in [
            ("button2", "empty-1"),
            ("button2", "empty-2"),
            ("button2", "empty-0"),
            ("button1", "empty-2"),
            ("button1", "empty-1"),
            ("button1", "empty-0"),
        ] {
            let a = crate::actions::control_action(&h.config(), button).unwrap();
            assert_eq!(a.kind, "page");
            execute(&h, &a, String::new(), &Arc::new(AtomicBool::new(false)), 0)
                .await
                .unwrap();
            assert_eq!(h.config().active_page, expected);
        }
        assert_eq!(serde_json::to_value(&h.config().pages).unwrap(), before);
        assert!(crate::actions::control_action(&h.config(), "lcd1").is_none());
        let next = crate::actions::control_action(&h.config(), "button2").unwrap();
        assert!(execute(
            &h,
            &next,
            String::new(),
            &Arc::new(AtomicBool::new(true)),
            0
        )
        .await
        .is_err());
        assert_eq!(h.config().active_page, "empty-0");
        h.w.lock().unwrap().active_page = "missing".into();
        assert!(execute(
            &h,
            &next,
            String::new(),
            &Arc::new(AtomicBool::new(false)),
            0
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn reserved_navigation_cycles_only_its_context_in_saved_order() {
        let host = Fake::new();
        let make_page = |id: &str, application: Option<&str>| Page {
            id: id.into(),
            name: id.into(),
            bindings: std::collections::BTreeMap::from([
                ("button1".into(), example("shell")),
                ("button2".into(), example("shell")),
            ]),
            application: application.map(|value| ApplicationAssociation {
                name: value.into(),
                image: None,
                identities: vec![ApplicationIdentity {
                    platform: "macos".into(),
                    value: value.into(),
                }],
            }),
        };
        {
            let mut config = host.w.lock().unwrap();
            config.pages = vec![
                make_page("home", None),
                make_page("chrome-main", Some("chrome")),
                make_page("utilities", None),
                make_page("code-main", Some("code")),
                make_page("chrome-tools", Some("chrome")),
                make_page("chrome-debug", Some("chrome")),
            ];
            config.page_switching = PageSwitching {
                enabled: true,
                default_page: Some("home".into()),
            };
            config.active_page = "chrome-main".into();
            config.validate().unwrap();
        }
        for enabled in [true, false] {
            host.w.lock().unwrap().page_switching.enabled = enabled;
            for (control, expected) in [
                ("button2", "chrome-tools"),
                ("button2", "chrome-debug"),
                ("button2", "chrome-main"),
                ("button1", "chrome-debug"),
                ("button1", "chrome-tools"),
                ("button1", "chrome-main"),
            ] {
                let action = crate::actions::control_action(&host.config(), control).unwrap();
                assert_eq!(action.kind, "page");
                execute(
                    &host,
                    &action,
                    String::new(),
                    &Arc::new(AtomicBool::new(false)),
                    0,
                )
                .await
                .unwrap();
                assert_eq!(host.config().active_page, expected);
            }
        }
        // Explicit targets may cross contexts; automatic page cycling may not.
        let action = Action {
            kind: "page".into(),
            value: "home".into(),
            ..Default::default()
        };
        execute(
            &host,
            &action,
            String::new(),
            &Arc::new(AtomicBool::new(false)),
            0,
        )
        .await
        .unwrap();
        for (direction, expected) in [
            ("next", "utilities"),
            ("next", "home"),
            ("previous", "utilities"),
        ] {
            switch_page(&mut host.w.lock().unwrap(), direction).unwrap();
            assert_eq!(host.config().active_page, expected);
        }
        // Reorder only affects traversal; the current selected page remains valid.
        {
            let mut config = host.w.lock().unwrap();
            config.pages.swap(1, 5);
            switch_page(&mut config, "chrome-debug").unwrap();
            switch_page(&mut config, "next").unwrap();
            assert_eq!(config.active_page, "chrome-tools");
            switch_page(&mut config, "previous").unwrap();
            assert_eq!(config.active_page, "chrome-debug");
            switch_page(&mut config, "code-main").unwrap();
            for direction in ["next", "previous"] {
                switch_page(&mut config, direction).unwrap();
                assert_eq!(config.active_page, "code-main");
            }
            assert!(switch_page(&mut config, "missing").is_err());
            assert_eq!(config.active_page, "code-main");
            config.active_page = "stale".into();
            assert!(switch_page(&mut config, "next")
                .unwrap_err()
                .contains("Active page"));
            config.pages.clear();
            assert!(switch_page(&mut config, "previous")
                .unwrap_err()
                .contains("no pages"));
        }
    }

    #[tokio::test]
    async fn every_supported_action_success_and_precancel() {
        for kind in ACTION_KINDS {
            let h = Fake::new();
            let a = example(kind);
            let result = execute(
                &h,
                &a,
                "fixture".into(),
                &Arc::new(AtomicBool::new(false)),
                0,
            )
            .await;
            assert!(result.is_ok(), "{kind}: {result:?}");
            let before = h.seen.lock().unwrap().len();
            assert_eq!(
                execute(
                    &h,
                    &a,
                    "fixture".into(),
                    &Arc::new(AtomicBool::new(true)),
                    0
                )
                .await
                .unwrap_err(),
                "Cancelled"
            );
            assert_eq!(h.seen.lock().unwrap().len(), before);
        }
    }
    #[tokio::test]
    async fn every_external_action_propagates_effect_failure() {
        for kind in ACTION_KINDS.iter().filter(|k| {
            ![
                "none",
                "workflow",
                "sequence",
                "condition",
                "cycle",
                "delay",
            ]
            .contains(k)
        }) {
            let mut h = Fake::new();
            h.fail = true;
            let result = execute(
                &h,
                &example(kind),
                "fixture".into(),
                &Arc::new(AtomicBool::new(false)),
                0,
            )
            .await;
            assert_eq!(result.unwrap_err(), "fixture effect failed", "{kind}");
            assert_eq!(h.seen.lock().unwrap().len(), 1);
        }
    }
    #[test]
    fn invalid_settings() {
        let w = Fake::new().config();
        for (kind, value) in [
            ("brightness", "selected"),
            ("brightness", "99999999999"),
            ("delay", "-1"),
            ("delay", "300001"),
            ("media", "nope"),
            ("mouse", "fourth"),
            ("shortcut", "ctrl+unknown"),
            ("page", "missing"),
            ("agent_select", "invalid"),
            ("shell", ""),
            ("open", ""),
            ("file_read", ""),
            ("file_write", ""),
            ("agent_prompt", ""),
            ("agent_launch", ""),
        ] {
            let mut a = example(kind);
            a.value = value.into();
            assert!(validate(&a, &w).is_err(), "{kind}:{value}");
        }
        assert!(validate(&example("future-extension"), &w).is_err());
    }
    #[tokio::test]
    async fn untrusted_pipeline_input_never_becomes_shell_source() {
        let host = Fake::new();
        let cancel = Arc::new(AtomicBool::new(false));
        for script in ["echo {{input}}", "echo '{{input}}'", "{{input}}"] {
            let mut action = example("shell");
            action.value = script.into();
            let error = execute(&host, &action, "$(touch bad)".into(), &cancel, 0)
                .await
                .unwrap_err();
            assert!(error.contains("cannot interpolate"));
            assert!(host.seen.lock().unwrap().is_empty());
        }
        let action = example("shell");
        let result = execute(&host, &action, "$(touch bad)".into(), &cancel, 0)
            .await
            .unwrap();
        assert_eq!(host.seen.lock().unwrap()[0].value, "printf fixture");
        assert_eq!(result, "$(touch bad)");
    }
    #[tokio::test]
    async fn template_pipeline_and_failure_short_circuit() {
        let h = Fake::new();
        let result = execute(
            &h,
            &example("workflow"),
            "".into(),
            &Arc::new(AtomicBool::new(false)),
            0,
        )
        .await
        .unwrap();
        assert!(result.starts_with("AI:"));
        let calls = h.seen.lock().unwrap().clone();
        assert_eq!(
            calls.iter().map(|a| a.kind.as_str()).collect::<Vec<_>>(),
            ["clipboard_read", "ai", "clipboard_write"]
        );
        assert!(calls[1].value.contains("source"));
        assert_eq!(calls[2].value, result);
        drop(calls);
        let mut h = Fake::new();
        h.fail = true;
        assert!(execute(
            &h,
            &example("workflow"),
            "".into(),
            &Arc::new(AtomicBool::new(false)),
            0
        )
        .await
        .is_err());
        assert_eq!(h.seen.lock().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn branches_cycles_and_recursive_workflows() {
        let h = Fake::new();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut a = example("condition");
        a.value = "match".into();
        execute(&h, &a, "absent".into(), &cancel, 0).await.unwrap();
        assert!(h.seen.lock().unwrap().is_empty());
        execute(&h, &a, "match".into(), &cancel, 0).await.unwrap();
        assert_eq!(h.seen.lock().unwrap().len(), 1);
        let mut cycle = example("cycle");
        cycle.children.push(Action {
            kind: "text".into(),
            value: "second".into(),
            ..Default::default()
        });
        for _ in 0..3 {
            execute(&h, &cycle, "first".into(), &cancel, 0)
                .await
                .unwrap();
        }
        let calls = h.seen.lock().unwrap().clone();
        assert_eq!(
            (&calls[1].value, &calls[2].value, &calls[3].value),
            (&"first".into(), &"second".into(), &"first".into())
        );
        drop(calls);
        h.w.lock().unwrap().workflows[0].steps = vec![example("workflow")];
        assert!(execute(&h, &example("workflow"), "".into(), &cancel, 0)
            .await
            .unwrap_err()
            .contains("nesting"));
    }
    #[tokio::test]
    async fn delay_cancels_while_waiting() {
        let h = Fake::new();
        let mut a = example("delay");
        a.value = "300000".into();
        let c = Arc::new(AtomicBool::new(false));
        let signal = c.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            signal.store(true, Ordering::Relaxed);
        });
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            execute(&h, &a, "".into(), &c, 0),
        )
        .await
        .unwrap();
        assert_eq!(result.unwrap_err(), "Cancelled");
    }
    #[test]
    fn brightness_edges_and_page_wrap() {
        for (current, value, expected) in [
            (98, "+5", 100),
            (2, "-5", 0),
            (60, "30", 30),
            (100, "+2147483647", 100),
            (0, "-2147483648", 0),
        ] {
            assert_eq!(brightness(current, value).unwrap(), expected);
        }
        let mut w = Config::default();
        w.pages.push(Page {
            id: "second".into(),
            name: "second".into(),
            bindings: Default::default(),
            application: None,
        });
        switch_page(&mut w, "previous").unwrap();
        assert_eq!(w.active_page, "second");
        switch_page(&mut w, "next").unwrap();
        assert_eq!(w.active_page, "home");
    }
}
