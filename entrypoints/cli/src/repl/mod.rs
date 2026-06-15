//! Interactive REPL

pub mod commands;
pub mod completion;
pub mod keymap;
pub mod loop_request;
pub mod mode;
pub mod prompt;
pub mod protocol_bridge;
pub mod render;
pub mod session;

use anyhow::Result;
use rust_i18n::t;
use std::borrow::Cow;
use std::io::Write;
use std::sync::Arc;

use protocol_interface::core::SessionRef;
use protocol_interface::{Message, SessionMetadata};
use runtime_config::{system_prompt_for_role, Settings};
use runtime_store::{ConversationStore, SessionStore};
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::HistoryHinter;
use rustyline::validate::{MatchingBracketValidator, Validator};
use rustyline::{Config, Context, Helper};

const GREEN: &str = "\x1b[32m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

struct ReplCompleter {
    models: Vec<String>,
}

impl Completer for ReplCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let cursor = line[..pos].chars().count();
        let Some(result) = completion::complete(line, cursor, &self.models) else {
            return Ok((pos, Vec::new()));
        };

        let start = char_to_byte_index(line, result.start);
        let completions = result
            .matches
            .into_iter()
            .map(|item| Pair {
                display: item.display,
                replacement: item.replacement,
            })
            .collect();

        Ok((start, completions))
    }
}

#[derive(Helper)]
struct ReplHelper {
    completer: ReplCompleter,
    hinter: HistoryHinter,
    highlighter: MatchingBracketHighlighter,
    validator: MatchingBracketValidator,
}

impl rustyline::hint::Hinter for ReplHelper {
    type Hint = String;
    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        let line_to_pos = &line[..pos];
        if line_to_pos.starts_with('/') && !line_to_pos.contains(' ') {
            let matches = completion::root_matches(line_to_pos);
            if matches.len() == 1 {
                let hint = matches[0].command[line_to_pos.len()..].to_string();
                return Some(hint);
            }
        }
        self.hinter.hint(line, pos, ctx)
    }
}

impl Completer for ReplHelper {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        self.completer.complete(line, pos, ctx)
    }
}

impl Highlighter for ReplHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }
    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}

impl Validator for ReplHelper {
    fn validate(
        &self,
        ctx: &mut rustyline::validate::ValidationContext<'_>,
    ) -> rustyline::Result<rustyline::validate::ValidationResult> {
        self.validator.validate(ctx)
    }
}

fn char_to_byte_index(value: &str, cursor: usize) -> usize {
    value
        .char_indices()
        .nth(cursor)
        .map(|(index, _)| index)
        .unwrap_or_else(|| value.len())
}

pub(crate) fn init_required_message(missing: &[String]) -> String {
    let missing = if missing.is_empty() {
        "configuration".to_string()
    } else {
        missing.join(", ")
    };

    format!(
        "\x1b[33m{}\x1b[0m {}\n\x1b[32m{}\x1b[0m",
        t!("repl.not_initialized"),
        t!("repl.missing", items = missing),
        t!("repl.init_required_hint")
    )
}

pub(crate) fn missing_runtime_requirements(settings: &Settings) -> Vec<String> {
    let mut missing = settings.missing_chat_requirements();
    if crate::formula::current_project_soul().is_some() {
        missing.retain(|item| item != "soul");
    }
    missing
}

pub(crate) struct LocalConversation {
    messages: Vec<Message>,
    _system_prompt: Option<String>,
}

impl LocalConversation {
    fn new(system_prompt: Option<String>) -> Self {
        Self {
            messages: Vec::new(),
            _system_prompt: system_prompt,
        }
    }

    fn from_messages(system_prompt: Option<String>, messages: Vec<Message>) -> Self {
        Self {
            messages,
            _system_prompt: system_prompt,
        }
    }

    pub(crate) fn add_user_message(&mut self, text: String) {
        self.messages.push(Message::new_user(text));
    }

    pub(crate) fn add_assistant_message(&mut self, text: String) {
        self.messages.push(Message::new_assistant(text));
    }

    pub(crate) fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub(crate) fn clear(&mut self) {
        self.messages.clear();
    }

    pub(crate) fn len(&self) -> usize {
        self.messages.len()
    }
}

pub struct ReplSession {
    pub(crate) settings: Arc<std::sync::RwLock<Settings>>,
    pub(crate) conversation: LocalConversation,
    pub(crate) session_metadata: SessionMetadata,
    pub(crate) session_store: SessionStore,
    pub(crate) conversation_store: ConversationStore,
    pub(crate) auto_confirm: bool,
    pub(crate) auto_review: bool,
    pub(crate) mode: mode::ReplMode,
    pub(crate) models: Vec<String>,
    pub(crate) bridge: Option<protocol_bridge::ProtocolBridge>,
}

impl ReplSession {
    pub fn new(settings: Settings) -> Result<Self> {
        let system_prompt = crate::formula::current_project_soul()
            .and_then(|id| crate::formula::load_soul_prompts(&id))
            .unwrap_or_else(|| system_prompt_for_role(&settings.soul.role));
        let conversation = LocalConversation::new(Some(system_prompt));

        let session_metadata = SessionMetadata::new(settings.llm.model.clone());
        let session_store = SessionStore::new()?;
        let conversation_store = ConversationStore::new()?;

        session_store.save(&session_metadata)?;

        // Build ProtocolBridge through CoreRuntimeManager.
        let bridge = {
            let ws_root = std::env::current_dir().unwrap_or_default();
            protocol_bridge::ProtocolBridge::new(ws_root, settings.clone()).ok()
        };

        Ok(Self {
            settings: Arc::new(std::sync::RwLock::new(settings)),
            conversation,
            session_metadata,
            session_store,
            conversation_store,
            auto_confirm: true,
            auto_review: false,
            mode: mode::ReplMode::Chat,
            models: Vec::new(),
            bridge,
        })
    }

    pub fn model(&self) -> String {
        let model = self.settings.read().unwrap().llm.model.clone();
        if model.trim().is_empty() {
            "unconfigured".to_string()
        } else {
            model
        }
    }

    pub fn soul(&self) -> String {
        if let Some(id) = crate::formula::current_project_soul() {
            return id;
        }

        let soul = self.settings.read().unwrap().soul.role.to_string();
        if soul.trim().is_empty() {
            t!("common.not_configured").to_string()
        } else {
            soul
        }
    }

    fn build_system_prompt(&self) -> String {
        let base = crate::formula::current_project_soul()
            .and_then(|id| crate::formula::load_soul_prompts(&id))
            .unwrap_or_else(|| system_prompt_for_role(&self.settings.read().unwrap().soul.role));

        let mut parts = vec![base];
        if let Ok(global) = runtime_store::MemoryStore::global() {
            let text = global.all_text();
            if !text.is_empty() {
                parts.push(format!("User memories:\n{}", text));
            }
        }
        if let Ok(project) = runtime_store::MemoryStore::project() {
            let text = project.all_text();
            if !text.is_empty() {
                parts.push(format!("Project memories:\n{}", text));
            }
        }
        parts.join("\n\n")
    }

    fn fetch_models(&mut self) {
        if let Some(bridge) = &self.bridge {
            if let Ok(models) = bridge.model_list() {
                if !models.is_empty() {
                    self.models = models.into_iter().map(|m| m.id).collect();
                }
            }
        }
    }

    pub(crate) fn rebuild_runtime_bridge(&mut self) {
        let settings = self.settings.read().unwrap().clone();

        // Rebuild ProtocolBridge through CoreRuntimeManager.
        self.bridge = {
            let ws_root = std::env::current_dir().unwrap_or_default();
            protocol_bridge::ProtocolBridge::new(ws_root, settings.clone()).ok()
        };
    }

    pub async fn handle_input(&mut self, input: &str) -> Result<String> {
        if input.starts_with('/') {
            return self.handle_command(input).await;
        }
        if input == "exit" || input == "quit" {
            return Ok(t!("repl.bye").to_string());
        }

        let missing = missing_runtime_requirements(&self.settings.read().unwrap());
        if !missing.is_empty() {
            println!("{}", init_required_message(&missing));
            return Ok(String::new());
        }

        // Both Chat and Plan: use ProtocolBridge when available
        if let Some(bridge) = &self.bridge {
            self.conversation.add_user_message(input.to_string());
            let mut stdout = std::io::stdout();

            let runtime_mode: protocol_interface::core::RuntimeMode = self.mode.into();
            let result = bridge.send_message_streaming_with_mode(input, runtime_mode, |delta| {
                let _ = stdout.write_all(delta.as_bytes());
                let _ = stdout.flush();
            });

            println!();

            match result {
                Ok(full_response) => {
                    if !full_response.is_empty() {
                        self.conversation
                            .add_assistant_message(full_response.clone());
                    }

                    self.conversation_store
                        .save_messages(&self.session_metadata.id, self.conversation.messages())?;
                    let _ = self.session_store.update(&mut self.session_metadata);
                }
                Err(e) => {
                    eprintln!("\nError: {}", e);
                }
            }

            return Ok(String::new());
        }

        Err(anyhow::anyhow!(
            "Runtime manager unavailable. Run /init to configure the workspace."
        ))
    }

    pub(crate) async fn handle_command(&mut self, input: &str) -> Result<String> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts.first().copied().unwrap_or("");

        match cmd {
            "/init" => self.cmd_init().await,
            "/mode" => self.cmd_mode(parts),
            "/model" => self.cmd_model(parts).await,
            "/config" => self.cmd_config(parts).await,
            "/session" => self.cmd_session(parts).await,
            "/history" => self.cmd_history(),
            "/confirm" => self.cmd_confirm(parts),
            "/review" => self.cmd_review(parts).await,
            "/memory" => self.cmd_memory(parts),
            "/doctor" => self.cmd_doctor(),
            "/trace" => self.cmd_trace(parts),
            "/tools" => {
                if let Some(bridge) = &self.bridge {
                    let tools = bridge.tool_list()?;

                    let mut output = String::new();

                    // Display built-in tools
                    if !tools.is_empty() {
                        output.push_str("Built-in Tools:\n");
                        for tool in &tools {
                            output.push_str(&format!("  🔧 {} - {}\n", tool.name, tool.description));
                        }
                    }

                    // MCP tools will be displayed here in future
                    // TODO: Add MCP tools display when MCP manager methods are available

                    Ok(output)
                } else {
                    Ok(t!("tools.available", tools = "").to_string())
                }
            }
            "/clear" => {
                self.conversation.clear();
                Ok(t!("clear.conversation").to_string())
            }
            "/help" => {
                crate::ui::show_help();
                Ok(String::new())
            }
            "/quit" | "/exit" => Ok(t!("repl.bye").to_string()),
            _ => Ok(t!("repl.unknown_command", command = cmd).to_string()),
        }
    }

    fn cmd_mode(&mut self, parts: Vec<&str>) -> Result<String> {
        match parts.get(1).copied() {
            None => Ok(format!("Mode: {}", self.mode.as_str())),
            Some("chat") => {
                self.mode = mode::ReplMode::Chat;
                Ok(render::mode_switched(self.mode))
            }
            Some("plan") => {
                self.mode = mode::ReplMode::Plan;
                Ok(render::mode_switched(self.mode))
            }
            Some("toggle") => {
                self.mode = self.mode.toggle();
                Ok(render::mode_switched(self.mode))
            }
            Some(other) => Ok(format!(
                "Unknown mode: {}. Use /mode chat or /mode plan.",
                other
            )),
        }
    }

    async fn cmd_init(&mut self) -> Result<String> {
        if core_runtime::config::project_config_exists() {
            println!("{}", t!("init.exists_warning"));
            println!("{}", t!("init.confirm_reset"));
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if !answer.trim().eq_ignore_ascii_case("y") {
                return Ok(t!("init.cancelled").to_string());
            }
        }

        let locale = self.settings.read().unwrap().ui.locale.clone();
        core_runtime::config::reset_project_config(Some(&locale))?;

        match crate::tui::run_init_wizard().await {
            Ok(Some(modified)) => {
                *self.settings.write().unwrap() = modified;
                self.rebuild_runtime_bridge();
                self.fetch_models();
                self.conversation = LocalConversation::new(Some(self.build_system_prompt()));

                let s = self.settings.read().unwrap();
                println!();
                println!("{}{}{}", GREEN, t!("config.saved"), RESET);
                println!("  Provider: {:?}", s.llm.provider);
                println!("  Model:    {}", s.llm.model);
                println!("  Soul:     {}", s.soul.role);
                println!();
                Ok(String::new())
            }
            Ok(None) => Ok(t!("config.cancelled").to_string()),
            Err(e) => Ok(t!("init.error", error = e.to_string()).to_string()),
        }
    }

    async fn cmd_model(&mut self, parts: Vec<&str>) -> Result<String> {
        if parts.len() > 1 {
            return Ok(
                "Use /model without arguments to manage the project model pool.".to_string(),
            );
        }

        Ok("Use /model in the workspace to manage the project model pool.".to_string())
    }

    async fn cmd_config(&mut self, parts: Vec<&str>) -> Result<String> {
        if parts.len() > 1 {
            return Ok("Use /config without arguments to configure Plan, Execute, Review, language, and SOUL.".to_string());
        }
        let settings = self.settings.read().unwrap().clone();
        let result =
            tokio::task::spawn_blocking(move || crate::tui::run_config_panel(settings)).await??;
        match result {
            Some(modified) => {
                modified.save_to_user_config()?;
                *self.settings.write().unwrap() = modified;
                self.rebuild_runtime_bridge();
                self.fetch_models();
                let s = self.settings.read().unwrap();
                println!();
                println!("{}{}{}", GREEN, t!("config.saved"), RESET);
                println!("  Provider: {:?}", s.llm.provider);
                println!("  Model:    {}", s.llm.model);
                println!();
                Ok(String::new())
            }
            None => Ok(t!("config.cancelled").to_string()),
        }
    }

    async fn cmd_session(&mut self, parts: Vec<&str>) -> Result<String> {
        let sub = parts.get(1).copied().unwrap_or("current");
        match sub {
            "current" => Ok(t!(
                "session.current",
                id = &self.session_metadata.id.as_str()[..8],
                model = self.session_metadata.model.as_str(),
                count = self.conversation.len()
            )
            .to_string()),
            "new" => {
                let model = self.settings.read().unwrap().llm.model.clone();
                self.session_metadata = SessionMetadata::new(model);
                self.conversation = LocalConversation::new(Some(self.build_system_prompt()));
                self.session_store.save(&self.session_metadata)?;
                Ok(t!("session.new", id = &self.session_metadata.id.as_str()[..8]).to_string())
            }
            "list" => {
                let sessions = self.session_store.list()?;
                if sessions.is_empty() {
                    return Ok(t!("session.none").to_string());
                }
                let mut out = format!("{}\n", t!("session.list_title"));
                for s in &sessions {
                    out.push_str(&format!(
                        "  {} | {} | {} | {}\n",
                        &s.id.as_str()[..8],
                        s.model,
                        s.created_at.format("%m-%d %H:%M"),
                        s.updated_at.format("%m-%d %H:%M"),
                    ));
                }
                Ok(out.trim_end().to_string())
            }
            "load" => {
                let id_str = parts
                    .get(2)
                    .ok_or_else(|| anyhow::anyhow!("{}", t!("session.load_usage")))?;
                let sessions = self.session_store.list()?;
                let session = sessions
                    .iter()
                    .find(|s| s.id.as_str().starts_with(id_str))
                    .ok_or_else(|| anyhow::anyhow!("{}", t!("session.not_found", id = id_str)))?;
                let messages = self.conversation_store.load_messages(&session.id)?;
                let system_prompt = messages
                    .iter()
                    .find(|m| m.role == protocol_interface::MessageRole::System)
                    .map(|m| m.content.clone());
                let non_system: Vec<_> = messages
                    .into_iter()
                    .filter(|m| m.role != protocol_interface::MessageRole::System)
                    .collect();
                self.session_metadata = session.clone();
                self.conversation = LocalConversation::from_messages(system_prompt, non_system);
                Ok(t!(
                    "session.loaded",
                    id = &self.session_metadata.id.as_str()[..8],
                    count = self.conversation.len()
                )
                .to_string())
            }
            "clear" => {
                if let Some(bridge) = &self.bridge {
                    let session_ref =
                        SessionRef::from_existing(self.session_metadata.id.as_str().to_string());
                    let _ = bridge.clear_conversation(&session_ref);
                }
                self.conversation.clear();
                Ok(t!("session.cleared").to_string())
            }
            _ => Ok(t!("session.usage").to_string()),
        }
    }

    fn cmd_history(&self) -> Result<String> {
        let msgs = self.conversation.messages();
        if msgs.is_empty() {
            return Ok(t!("history.none").to_string());
        }
        for (i, msg) in msgs.iter().enumerate() {
            let preview: String = msg.content.chars().take(80).collect();
            let role = match msg.role {
                protocol_interface::MessageRole::System => t!("history.role.system"),
                protocol_interface::MessageRole::User => t!("history.role.user"),
                protocol_interface::MessageRole::Assistant => t!("history.role.assistant"),
                protocol_interface::MessageRole::Tool => t!("history.role.tool"),
                protocol_interface::MessageRole::Summary => t!("history.role.summary"),
            };
            println!("  {:3}. [{}] {}", i + 1, role, preview);
            if msg.content.len() > 80 {
                println!("      ...");
            }
        }
        Ok(String::new())
    }

    fn cmd_confirm(&mut self, parts: Vec<&str>) -> Result<String> {
        if let Some(mode) = parts.get(1) {
            match *mode {
                "on" | "yes" | "true" => {
                    self.auto_confirm = true;
                    Ok(t!("confirm.enabled").to_string())
                }
                "off" | "no" | "false" => {
                    self.auto_confirm = false;
                    Ok(t!("confirm.interactive_enabled").to_string())
                }
                _ => Ok(t!("confirm.usage").to_string()),
            }
        } else {
            let status = if self.auto_confirm { "on" } else { "off" };
            Ok(t!("confirm.status", status = status).to_string())
        }
    }

    pub(crate) async fn cmd_review(&mut self, parts: Vec<&str>) -> Result<String> {
        if let Some(mode) = parts.get(1) {
            match *mode {
                "on" | "true" => {
                    self.auto_review = true;
                    return Ok(t!("review.auto_enabled").to_string());
                }
                "off" | "false" => {
                    self.auto_review = false;
                    return Ok(t!("review.auto_disabled").to_string());
                }
                _ => {}
            }
        }

        let last_assistant = self
            .conversation
            .messages()
            .iter()
            .rev()
            .find(|m| m.role == protocol_interface::MessageRole::Assistant);

        if last_assistant.is_none() {
            return Ok(t!("review.none").to_string());
        }

        // Prefer protocol path
        if let Some(bridge) = &self.bridge {
            let session_ref =
                SessionRef::from_existing(self.session_metadata.id.as_str().to_string());
            if let Ok(run_ref) = bridge.review_start(&session_ref) {
                let events = bridge.subscribe(&run_ref)?;
                let mut response = String::new();
                for envelope in &events {
                    if let (
                        protocol_interface::core::CoreEventKind::ModelDelta,
                        protocol_interface::core::CoreEventPayload::Text { text },
                    ) = (&envelope.payload.kind, &envelope.payload.payload)
                    {
                        response.push_str(text);
                    }
                }
                if response.is_empty() {
                    // Fallback: extract from FinalResult
                    for envelope in &events {
                        if let (
                            protocol_interface::core::CoreEventKind::FinalResult,
                            protocol_interface::core::CoreEventPayload::Final { content, .. },
                        ) = (&envelope.payload.kind, &envelope.payload.payload)
                        {
                            response = content.clone();
                        }
                    }
                }
                return Ok(response);
            }
        }

        Err(anyhow::anyhow!(
            "Runtime manager unavailable. Review must run through Core Runtime."
        ))
    }

    fn cmd_memory(&self, parts: Vec<&str>) -> Result<String> {
        let sub = parts.get(1).copied().unwrap_or("show");
        match sub {
            "save" => {
                let text = parts[2..].join(" ");
                if text.is_empty() {
                    return Ok(t!("memory.usage_save").to_string());
                }
                if let Some(bridge) = &self.bridge {
                    bridge.memory_save(&text, vec![])?;
                } else {
                    return Err(anyhow::anyhow!(
                        "Runtime manager unavailable. Memory writes must run through Core Runtime."
                    ));
                }
                Ok(t!("memory.saved", text = text).to_string())
            }
            "list" | "show" => {
                if let Some(bridge) = &self.bridge {
                    let entries = bridge.memory_list()?;
                    if entries.is_empty() {
                        return Ok(t!("memory.none").to_string());
                    }
                    let mut out = String::new();
                    for (i, e) in entries.iter().enumerate() {
                        out.push_str(&format!("  {}. {}\n", i + 1, e.content));
                    }
                    Ok(out.trim_end().to_string())
                } else {
                    Err(anyhow::anyhow!(
                        "Runtime manager unavailable. Memory reads must run through Core Runtime."
                    ))
                }
            }
            "clear" => {
                if let Some(bridge) = &self.bridge {
                    bridge.memory_clear()?;
                } else {
                    return Err(anyhow::anyhow!(
                        "Runtime manager unavailable. Memory clear must run through Core Runtime."
                    ));
                }
                Ok(t!("memory.cleared").to_string())
            }
            _ => Ok(t!("memory.usage").to_string()),
        }
    }

    fn cmd_doctor(&self) -> Result<String> {
        let s = self.settings.read().unwrap();
        let mut checks = Vec::new();

        let api_ok = s.api_key().is_ok();
        checks.push(format!(
            "  {} {}",
            if api_ok {
                t!("common.ok")
            } else {
                t!("common.fail")
            },
            t!("doctor.api_key")
        ));
        checks.push(format!(
            "  {} {}",
            t!("common.ok"),
            t!(
                "doctor.provider",
                provider = format!("{:?}", s.llm.provider)
            )
        ));
        checks.push(format!(
            "  {} {}",
            t!("common.ok"),
            t!("doctor.model", model = s.llm.model.as_str())
        ));

        if s.llm.review_model.is_some() {
            checks.push(format!(
                "  {} {}",
                t!("common.ok"),
                t!("doctor.review_model_configured")
            ));
        }

        match crate::formula::current_project_soul() {
            Some(id) => checks.push(format!(
                "  {} {}",
                t!("common.ok"),
                t!("doctor.active_soul", soul = id.as_str())
            )),
            None => checks.push(format!("  -- {}", t!("doctor.no_soul"))),
        }

        let repo = crate::formula::official_repo_path();
        if repo.exists() {
            checks.push(format!(
                "  {} {}",
                t!("common.ok"),
                t!("doctor.formula_repo", path = repo.display().to_string())
            ));
        } else {
            checks.push(format!("  -- {}", t!("doctor.formula_repo_missing")));
        }

        let global_mem = runtime_store::MemoryStore::global().ok();
        let mem_count = global_mem.map(|m| m.list().len()).unwrap_or(0);
        checks.push(format!(
            "  {} {}",
            t!("common.ok"),
            t!("doctor.memories", count = mem_count)
        ));

        let mcp_config = crate::mcp::load_config().ok();
        let mcp_count = mcp_config.map(|c| c.servers.len()).unwrap_or(0);
        checks.push(format!(
            "  {} {}",
            t!("common.ok"),
            t!("doctor.mcp_servers", count = mcp_count)
        ));

        let plugin_count = crate::plugin::list_plugins()
            .ok()
            .map(|p| p.len())
            .unwrap_or(0);
        checks.push(format!(
            "  {} {}",
            t!("common.ok"),
            t!("doctor.plugins", count = plugin_count)
        ));

        let wf_dir = crate::workflow::workflows_dir();
        let wf_count = crate::workflow::load_workflows(&wf_dir)
            .ok()
            .map(|w| w.len())
            .unwrap_or(0);
        checks.push(format!(
            "  {} {}",
            t!("common.ok"),
            t!("doctor.workflows", count = wf_count)
        ));

        let mut out = format!("{}{}{}\n", BOLD, t!("doctor.title"), RESET);
        out.push_str(&checks.join("\n"));
        Ok(out)
    }

    fn cmd_trace(&self, parts: Vec<&str>) -> Result<String> {
        let sub = parts.get(1).copied().unwrap_or("latest");
        match sub {
            "latest" | "show" => {
                let msgs = self.conversation.messages();
                if msgs.is_empty() {
                    return Ok(t!("trace.none").to_string());
                }
                let mut out = format!(
                    "{}{}{}\n",
                    BOLD,
                    t!("trace.title", count = msgs.len()),
                    RESET
                );
                for (i, msg) in msgs.iter().enumerate() {
                    let role = match msg.role {
                        protocol_interface::MessageRole::System => t!("history.role.system"),
                        protocol_interface::MessageRole::User => t!("history.role.user"),
                        protocol_interface::MessageRole::Assistant => t!("history.role.assistant"),
                        protocol_interface::MessageRole::Tool => t!("history.role.tool"),
                        protocol_interface::MessageRole::Summary => t!("history.role.summary"),
                    };
                    let preview = if msg.content.len() > 80 {
                        format!("{}...", &msg.content[..80])
                    } else {
                        msg.content.clone()
                    };
                    out.push_str(&format!("  {:3} [{}] {}\n", i + 1, role, preview));
                }
                Ok(out.trim_end().to_string())
            }
            _ => Ok(t!("trace.usage").to_string()),
        }
    }
}

pub async fn run_repl(settings: Settings) -> Result<()> {
    if std::env::var_os("ALIUS_LEGACY_REPL").is_none() {
        let initial_missing = missing_runtime_requirements(&settings);
        let mut session = ReplSession::new(settings)?;
        session.fetch_models();
        return crate::tui::workspace::run_workspace(session, initial_missing).await;
    }

    run_legacy_repl(settings).await
}

async fn run_legacy_repl(settings: Settings) -> Result<()> {
    crate::ui::show_welcome(&settings);

    let missing = missing_runtime_requirements(&settings);
    if !missing.is_empty() {
        println!();
        println!("{}", init_required_message(&missing));
        println!();
    }

    let mut session = ReplSession::new(settings)?;

    session.fetch_models();

    let rl_config = Config::builder()
        .completion_type(rustyline::CompletionType::List)
        .build();

    let helper = ReplHelper {
        completer: ReplCompleter {
            models: session.models.clone(),
        },
        hinter: HistoryHinter::new(),
        highlighter: MatchingBracketHighlighter::new(),
        validator: MatchingBracketValidator::new(),
    };

    let mut rl: rustyline::Editor<ReplHelper, rustyline::history::DefaultHistory> =
        rustyline::Editor::with_config(rl_config)
            .map_err(|e| anyhow::anyhow!("REPL error: {}", e))?;
    rl.set_helper(Some(helper));

    loop {
        let prompt_str = prompt::build_prompt(session.mode, &session.model());
        let readline = rl.readline(&prompt_str);

        match readline {
            Ok(line) if !line.trim().is_empty() => {
                let _ = rl.add_history_entry(&line);
                match session.handle_input(&line).await {
                    Ok(result) if result == t!("repl.bye") => break,
                    Ok(result) if !result.is_empty() => println!("{}", result),
                    Ok(_) => {}
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            Ok(_) => continue,
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("^D");
                break;
            }
            Err(e) => return Err(anyhow::anyhow!("REPL error: {}", e)),
        }
    }

    println!("\n{}", t!("repl.goodbye"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_list_includes_init() {
        assert!(completion::command_names().any(|command| command == "/init"));
    }

    #[test]
    fn init_required_message_points_to_init_and_missing_fields() {
        let message = init_required_message(&["model".to_string(), "soul".to_string()]);

        assert!(message.contains("/init"));
        assert!(message.contains("model"));
        assert!(message.contains("soul"));
    }
}
