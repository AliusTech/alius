//! CLI binary entrypoint for the Alius workspace crate.
//!
//! This module is the main entrypoint for the Alius CLI binary when built
//! from the workspace crate structure. It parses CLI arguments, loads settings,
//! and dispatches to the appropriate command handler.

rust_i18n::i18n!("locales", fallback = "en");

mod formula;
mod mcp;
mod plugin;
mod repl;
mod tui;
mod ui;
mod workflow;

use alius_cli::{
    Cli, Command, ConfigCommand, CoreCommand, CredentialCommand, McpCommand, PluginCommand,
    SoulCommand, WorkflowCommand,
};
use anyhow::Result;
use clap::Parser;
use core_runtime::CoreRuntimeManager;
use protocol_interface::core::{CoreEventKind, CoreEventPayload, RuntimeMode};
use runtime_config::Settings;
use rust_i18n::t;
use std::io::Write;

fn set_locale(locale: &str) {
    rust_i18n::set_locale(locale);
}

/// Main application logic.
///
/// Parses CLI arguments, loads configuration, and dispatches to the
/// appropriate command handler. Returns an error if any operation fails.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Load settings from default location or specified config file
    let settings = Settings::load()?;

    // Apply saved locale before any UI output
    set_locale(&settings.ui.locale);

    match cli.command {
        // No subcommand or explicit REPL: start interactive mode
        None | Some(Command::Repl) => {
            crate::repl::run_repl(settings).await?;
        }
        // Run a single prompt in non-interactive mode
        Some(Command::Run { prompt, model }) => {
            let mut settings = settings;
            // Override the default model if specified via --model flag
            if let Some(m) = model {
                settings.llm.model = m;
            }

            let workspace_root = std::env::current_dir().unwrap_or_default();
            let manager = CoreRuntimeManager::new_local(workspace_root, settings)?;
            let (_run_ref, mut rx) = manager.start_streaming(&prompt, RuntimeMode::Chat)?;
            let mut failed: Option<String> = None;
            let mut printed_delta = false;

            while let Some(event) = rx.recv().await {
                match (&event.kind, &event.payload) {
                    (CoreEventKind::ModelDelta, CoreEventPayload::Text { text }) => {
                        printed_delta = true;
                        print!("{}", text);
                        let _ = std::io::stdout().flush();
                    }
                    (CoreEventKind::ErrorRaised, CoreEventPayload::Error { message, .. }) => {
                        if !message.is_empty() {
                            failed = Some(message.clone());
                        }
                    }
                    (
                        CoreEventKind::FinalResult,
                        CoreEventPayload::Final {
                            success: false,
                            content,
                        },
                    ) => {
                        failed = Some(if content.is_empty() {
                            "Run failed".to_string()
                        } else {
                            content.clone()
                        });
                    }
                    (
                        CoreEventKind::FinalResult,
                        CoreEventPayload::Final {
                            success: true,
                            content,
                        },
                    ) if !printed_delta && !content.is_empty() => {
                        // Some providers may only emit the final text.
                        print!("{}", content);
                        let _ = std::io::stdout().flush();
                    }
                    _ => {}
                }
            }

            if let Some(message) = failed {
                if !message.is_empty() {
                    eprintln!("{}", message);
                }
            }

            println!();
        }
        // Configuration management subcommands
        Some(Command::Config { command }) => {
            handle_config(&settings, command)?;
        }
        // Display version information
        Some(Command::Version) => {
            println!("alius {}", env!("ALIUS_VERSION"));
        }
        // Official Soul repository management
        Some(Command::Core { command }) => {
            handle_core(command)?;
        }
        // Soul management
        Some(Command::Soul { command }) => {
            handle_soul(command)?;
        }
        // Plugin management
        Some(Command::Plugin { command }) => {
            handle_plugin(command)?;
        }
        // MCP server management
        Some(Command::Mcp { command }) => {
            handle_mcp(command)?;
        }
        // Workflow management
        Some(Command::Workflow { command }) => {
            handle_workflow(command).await?;
        }
        // Initialize project configuration via TUI wizard
        Some(Command::Init) => {
            if core_runtime::config::project_config_exists() {
                println!("{}", t!("init.exists_warning"));
                println!("{}", t!("init.confirm_reset"));
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if !answer.trim().eq_ignore_ascii_case("y") {
                    println!("{}", t!("init.cancelled"));
                    return Ok(());
                }
            }

            let locale = settings.ui.locale.clone();
            core_runtime::config::reset_project_config(Some(&locale))?;

            match crate::tui::run_init_wizard().await {
                Ok(Some(settings)) => {
                    println!();
                    println!("{}", t!("cli.config_saved"));
                    println!("  Provider: {:?}", settings.llm.provider);
                    println!("  Model:    {}", settings.llm.model);
                    println!("  Soul:     {}", settings.soul.role);
                    println!();
                    println!("{}", t!("cli.run_to_start"));
                }
                Ok(None) => println!("{}", t!("init.cancelled")),
                Err(e) => eprintln!("{}", t!("init.error", error = e.to_string())),
            }
        }
    }

    Ok(())
}

/// Handle configuration subcommands.
///
/// Dispatches to the appropriate handler for show, validate, or soul commands.
fn handle_config(settings: &Settings, cmd: ConfigCommand) -> Result<()> {
    match cmd {
        // Display current configuration
        ConfigCommand::Show => {
            let soul_display = match crate::formula::current_project_soul() {
                Some(id) => {
                    let installed = crate::formula::load_soul_prompts(&id)
                        .map(|_| " (installed)")
                        .unwrap_or(" (NOT INSTALLED)");
                    format!("{}{}", id, installed)
                }
                None => {
                    let configured = settings.soul.role.to_string();
                    if configured.trim().is_empty() {
                        "not configured".to_string()
                    } else {
                        configured
                    }
                }
            };
            println!("Configuration:");
            println!("  Provider: {:?}", settings.llm.provider);
            println!(
                "  Model: {}",
                if settings.llm.model.trim().is_empty() {
                    "not configured"
                } else {
                    &settings.llm.model
                }
            );
            println!("  Soul: {}", soul_display);

            // Display new-system config if available
            let cwd = std::env::current_dir().unwrap_or_default();
            if let Ok(snapshot) = runtime_config::config_manager::load_project_config(&cwd) {
                println!();
                println!("Routing:");
                println!(
                    "  Default: {} / {}",
                    if snapshot.model.default_provider.is_empty() {
                        "-"
                    } else {
                        &snapshot.model.default_provider
                    },
                    if snapshot.model.default_model.is_empty() {
                        "-"
                    } else {
                        &snapshot.model.default_model
                    }
                );
                let enabled: Vec<String> = snapshot
                    .providers
                    .providers
                    .iter()
                    .filter(|(_, v)| v.enabled)
                    .map(|(k, _)| k.clone())
                    .collect();
                println!("  Providers: {}", enabled.join(", "));

                let tiers = [
                    ("light", &snapshot.providers.tiers.light),
                    ("medium", &snapshot.providers.tiers.medium),
                    ("high", &snapshot.providers.tiers.high),
                ];
                for (name, tier) in tiers {
                    let model_display = if tier.model.is_empty() {
                        "(inherit)"
                    } else {
                        &tier.model
                    };
                    println!("  Tier {}: {} ({})", name, tier.provider, model_display);
                }
            }
        }
        // Validate configuration file
        ConfigCommand::Validate => {
            let mut missing = settings.missing_chat_requirements();
            if let Some(soul_id) = crate::formula::current_project_soul() {
                missing.retain(|item| item != "soul");
                if crate::formula::load_soul_prompts(&soul_id).is_none() {
                    missing.push("soul prompts (re-install required)".to_string());
                }
            }
            if missing.is_empty() {
                println!("{}", t!("cli.configuration_valid"));
            } else {
                println!(
                    "{}",
                    t!("cli.configuration_incomplete", items = missing.join(", "))
                );
                println!("{}", t!("cli.run_init_hint"));
            }
        }
        // Set the soul role
        ConfigCommand::Soul { role } => match crate::formula::install_and_activate_soul(&role) {
            Ok(formula) => {
                println!(
                    "Soul '{}' v{} installed and activated.",
                    formula.id, formula.version
                );
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                eprintln!("Run 'alius soul update' to sync local souls.");
            }
        },
        // Manage credentials in the OS keyring
        ConfigCommand::Credential { command } => handle_credential(command)?,
    }
    Ok(())
}

/// Handle credential management subcommands.
fn handle_credential(cmd: CredentialCommand) -> Result<()> {
    match cmd {
        CredentialCommand::Set { key, value } => {
            runtime_model::credential::store_secret(&key, &value)?;
            println!("Stored credential '{}' in keyring.", key);
        }
        CredentialCommand::Delete { key } => {
            runtime_model::credential::delete_secret(&key)?;
            println!("Deleted credential '{}' from keyring.", key);
        }
        CredentialCommand::Check => {
            if runtime_model::credential::check_keyring_available() {
                println!("Keyring is available.");
            } else {
                println!("Keyring is NOT available in this environment.");
                println!("Credentials will fall back to environment variables.");
            }
        }
    }
    Ok(())
}

/// Handle official Soul repository subcommands.
fn handle_core(cmd: CoreCommand) -> Result<()> {
    match cmd {
        CoreCommand::Update => {
            println!("{}", t!("cli.updating_formula_repo"));
            let path = crate::formula::update_repo()?;
            println!("{}", t!("cli.updated", path = path.display().to_string()));
        }
        CoreCommand::List => {
            let souls = crate::formula::list_available_souls()?;
            if souls.is_empty() {
                println!("{}", t!("cli.no_formulas"));
            } else {
                println!("{}", t!("cli.available_souls"));
                for f in &souls {
                    println!("  {:<20} {} v{}", f.id, f.name, f.version);
                    println!("  {:<20} {}", "", f.description);
                }
            }
        }
        CoreCommand::Info { id } => {
            let repo = crate::formula::official_repo_path();
            if !repo.exists() {
                println!("{}", t!("cli.repository_not_found"));
                return Ok(());
            }
            match crate::formula::find_formula(&repo, "souls", &id)? {
                Some(f) => {
                    println!("{} (v{})", f.name, f.version);
                    println!("  ID:          {}", f.id);
                    println!("  Type:        {}", f.formula_type);
                    println!("  Description: {}", f.description);
                    if let Some(lic) = &f.license {
                        println!("  License:     {}", lic);
                    }
                    if let Some(model) = &f.model {
                        if let Some(p) = &model.preferred_provider {
                            println!("  Provider:    {}", p);
                        }
                        if let Some(m) = &model.preferred_main_model {
                            println!("  Model:       {}", m);
                        }
                        if let Some(r) = &model.preferred_review_model {
                            println!("  Review:      {}", r);
                        }
                    }
                }
                None => println!("Soul not found: {}", id),
            }
        }
    }
    Ok(())
}

/// Handle soul management subcommands.
fn handle_soul(cmd: SoulCommand) -> Result<()> {
    match cmd {
        SoulCommand::Update => {
            println!("Updating souls from alius-souls...");
            let souls = crate::formula::sync_all_souls()?;
            println!(
                "Synced {} souls to {}",
                souls.len(),
                crate::formula::soul_dir().display()
            );
        }
        SoulCommand::List => {
            let souls = crate::formula::list_installed_souls()?;
            if souls.is_empty() {
                println!("No local souls found. Run: alius soul update");
            } else {
                let current = crate::formula::current_project_soul();
                println!("Installed Souls:");
                for s in &souls {
                    let marker = if current.as_deref() == Some(&s.id) {
                        " (active)"
                    } else {
                        ""
                    };
                    println!("  {:<20} {} v{}{}", s.id, s.name, s.version, marker);
                }
            }
        }
        SoulCommand::Install { id } => {
            // Check if already installed
            let existing = crate::formula::list_installed_souls()?;
            if existing.iter().any(|s| s.id == id) {
                println!("Soul '{}' is already installed", id);
                return Ok(());
            }
            // Find formula in repo
            let repo = crate::formula::official_repo_path();
            if !repo.exists() {
                println!("Repository not found. Run: alius soul update");
                return Ok(());
            }
            match crate::formula::find_formula(&repo, "souls", &id)? {
                Some(formula) => {
                    let path = crate::formula::install_soul(&formula, &repo)?;
                    println!(
                        "Installed '{}' v{} to {}",
                        id,
                        formula.version,
                        path.display()
                    );
                }
                None => println!("Soul not found: {}. Run: alius soul update", id),
            }
        }
        SoulCommand::Current => match crate::formula::current_project_soul() {
            Some(id) => println!("Current soul: {}", id),
            None => println!("No soul activated. Run: alius init"),
        },
        SoulCommand::Remove { id } => {
            crate::formula::remove_soul(&id)?;
            println!("Removed soul '{}'", id);
        }
    }
    Ok(())
}

/// Handle plugin management subcommands.
fn handle_plugin(cmd: PluginCommand) -> Result<()> {
    match cmd {
        PluginCommand::List => {
            let plugins = crate::plugin::list_plugins()?;
            if plugins.is_empty() {
                println!("No plugins installed.");
            } else {
                println!("Installed Plugins:");
                for p in &plugins {
                    println!(
                        "  {:<20} {} v{}",
                        p.manifest.id, p.manifest.name, p.manifest.version
                    );
                    println!("  {:<20} {}", "", p.manifest.description);
                }
            }
        }
        PluginCommand::Install { path } => {
            let source = std::path::PathBuf::from(&path);
            let manifest = crate::plugin::install_plugin(&source)?;
            println!("Installed plugin '{}' v{}", manifest.id, manifest.version);
        }
        PluginCommand::Info { id } => match crate::plugin::find_plugin(&id)? {
            Some(p) => {
                println!("{} (v{})", p.manifest.name, p.manifest.version);
                println!("  ID:          {}", p.manifest.id);
                println!("  Description: {}", p.manifest.description);
                if let Some(author) = &p.manifest.author {
                    println!("  Author:      {}", author);
                }
                println!("  WASM:        {}", p.wasm_path.display());
            }
            None => println!("Plugin not found: {}", id),
        },
        PluginCommand::Remove { id } => {
            crate::plugin::remove_plugin(&id)?;
            println!("Removed plugin '{}'", id);
        }
    }
    Ok(())
}

/// Handle MCP server management subcommands.
fn handle_mcp(cmd: McpCommand) -> Result<()> {
    match cmd {
        McpCommand::List => {
            let servers = crate::mcp::list_configured_servers()?;
            if servers.is_empty() {
                println!("No MCP servers configured. Create .alius/mcp.json or ~/.alius/mcp.json");
            } else {
                println!("Configured MCP Servers:");
                for (name, config) in &servers {
                    println!(
                        "  {:<20} {} {}",
                        name,
                        config.command,
                        config.args.join(" ")
                    );
                }
            }
        }
        McpCommand::Start { name } => {
            let config = crate::mcp::load_config()?;
            let server_config = config
                .servers
                .get(&name)
                .ok_or_else(|| anyhow::anyhow!("Server '{}' not found in config", name))?;
            let mut server = crate::mcp::McpServer::start(&name, server_config)?;
            let tools = server.list_tools()?;
            println!("Started '{}': {} tools available", name, tools.len());
            for t in &tools {
                println!("  {:<30} {}", t.name, t.description);
            }
            server.stop()?;
        }
        McpCommand::Tools { name } => {
            let config = crate::mcp::load_config()?;
            let server_config = config
                .servers
                .get(&name)
                .ok_or_else(|| anyhow::anyhow!("Server '{}' not found in config", name))?;
            let mut server = crate::mcp::McpServer::start(&name, server_config)?;
            let tools = server.list_tools()?;
            println!("Tools from '{}':", name);
            for t in &tools {
                println!("  {:<30} {}", t.name, t.description);
            }
            server.stop()?;
        }
    }
    Ok(())
}

/// Handle workflow management subcommands.
async fn handle_workflow(cmd: WorkflowCommand) -> Result<()> {
    match cmd {
        WorkflowCommand::List => {
            let dir = crate::workflow::workflows_dir();
            let workflows = crate::workflow::load_workflows(&dir)?;
            if workflows.is_empty() {
                println!("No workflows found in {}", dir.display());
                println!("Create a .json workflow file in that directory.");
            } else {
                println!("Workflows:");
                for wf in &workflows {
                    println!(
                        "  {:<20} {} ({} steps)",
                        wf.name,
                        wf.description,
                        wf.steps.len()
                    );
                }
            }
        }
        WorkflowCommand::Run { name } => {
            // Try as path first, then as name in workflows dir
            let path = std::path::PathBuf::from(&name);
            let workflow = if path.exists() {
                crate::workflow::load_workflow(&path)?
            } else {
                let dir = crate::workflow::workflows_dir();
                let workflows = crate::workflow::load_workflows(&dir)?;
                workflows
                    .into_iter()
                    .find(|w| w.name == name)
                    .ok_or_else(|| anyhow::anyhow!("Workflow not found: {}", name))?
            };
            crate::workflow::execute_workflow(&workflow).await?;
        }
        WorkflowCommand::Validate { path } => {
            let path = std::path::PathBuf::from(&path);
            match crate::workflow::load_workflow(&path) {
                Ok(wf) => {
                    println!("Valid workflow: {}", wf.name);
                    println!("  Steps: {}", wf.steps.len());
                    for step in &wf.steps {
                        println!("    {} ({:?})", step.id, step.step_type);
                    }
                }
                Err(e) => {
                    println!("Invalid workflow: {}", e);
                }
            }
        }
    }
    Ok(())
}

/// Binary entrypoint.
///
/// Creates a Tokio async runtime and executes the main application logic.
fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to create runtime");
    rt.block_on(run()).expect("Failed to run");
}
