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
mod updater;
mod workflow;

use alius_cli::{
    Cli, Command, ConfigCommand, CoreCommand, CredentialCommand, McpCommand, PluginCommand,
    SoulCommand, UpdateCommand, WorkflowCommand,
};
use anyhow::Result;
use clap::Parser;
use core_runtime::CoreRuntimeManager;
use protocol_interface::core::{CoreEventKind, CoreEventPayload, RuntimeMode};
use runtime_config::{Settings, SoulRole};
use rust_i18n::t;
use std::io::Write;

fn set_locale(locale: &str) {
    rust_i18n::set_locale(locale);
}

/// Parse a provider string into a ProviderType.
///
/// Accepts case-insensitive names: "openai", "anthropic", "google",
/// "bigmodel", "deepseek", "xiaomi_mimo", "custom".
fn parse_provider(s: &str) -> Result<protocol_interface::ProviderType> {
    match s.to_lowercase().as_str() {
        "openai" => Ok(protocol_interface::ProviderType::Openai),
        "anthropic" => Ok(protocol_interface::ProviderType::Anthropic),
        "google" => Ok(protocol_interface::ProviderType::Google),
        "bigmodel" => Ok(protocol_interface::ProviderType::BigModel),
        "deepseek" => Ok(protocol_interface::ProviderType::DeepSeek),
        "xiaomi_mimo" | "xiaomi-mimo" => Ok(protocol_interface::ProviderType::XiaomiMimo),
        "custom" => Ok(protocol_interface::ProviderType::Custom),
        _ => Err(anyhow::anyhow!(
            "Unknown provider '{}'. Valid: openai, anthropic, google, bigmodel, deepseek, xiaomi_mimo, custom",
            s
        )),
    }
}

/// Apply CLI global parameter overrides to loaded settings.
///
/// Overrides are applied in order: config path, workspace, provider, model.
/// The resolved workspace root is returned for use by all commands.
fn apply_cli_overrides(cli: &Cli) -> Result<(Settings, std::path::PathBuf)> {
    // Load settings: use custom config path if --config is provided
    let mut settings = match &cli.config {
        Some(path) => Settings::load_from_path(path)?,
        None => Settings::load()?,
    };

    // Resolve workspace root: --workspace flag > current directory.
    // This must happen before project config hydration so that the
    // project config is loaded from the correct workspace.
    let workspace_root = cli
        .workspace
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Hydrate from project config and soul activation using resolved workspace
    settings.hydrate_from_project_config(&workspace_root);
    if let Some(soul_id) = crate::formula::current_project_soul() {
        settings.soul.role = SoulRole::new(soul_id);
    }

    // Apply --provider override
    if let Some(ref provider_str) = cli.provider {
        settings.llm.provider = parse_provider(provider_str)?;
    }

    // Apply --model override (global flag, applies to all commands)
    if let Some(ref model) = cli.model {
        settings.llm.model = model.clone();
    }

    // Apply saved locale and TUI theme before any UI output
    set_locale(&settings.ui.locale);
    crate::tui::theme::set_theme(&settings.ui.theme);

    Ok((settings, workspace_root))
}

/// Main application logic.
///
/// Parses CLI arguments, loads configuration, and dispatches to the
/// appropriate command handler. Returns an error if any operation fails.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing based on --verbose flag.
    // 0 = warn, 1 = info, 2 = debug, 3+ = trace.
    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_target(false)
        .init();

    let (settings, workspace_root) = apply_cli_overrides(&cli)?;

    match cli.command {
        // No subcommand or explicit REPL: start interactive mode
        None | Some(Command::Repl) => {
            if updater::should_auto_check(&settings) {
                let _ = updater::check_and_notify_silent().await;
            }
            crate::repl::run_repl(settings, workspace_root).await?;
        }
        // Run a single prompt in non-interactive mode
        Some(Command::Run { prompt, model }) => {
            let mut settings = settings;
            // Subcommand-level --model overrides global --model
            if let Some(m) = model {
                settings.llm.model = m;
            }
            let assignment_issues = crate::repl::model_assignment_readiness_issues(&workspace_root);
            if !assignment_issues.is_empty() {
                anyhow::bail!(
                    "{}",
                    crate::repl::model_assignment_required_message(&assignment_issues)
                );
            }

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
                std::process::exit(1);
            }

            println!();
        }
        // Configuration management subcommands
        Some(Command::Config { command }) => {
            handle_config(&settings, command, &workspace_root)?;
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
            handle_workflow(command, &settings, &workspace_root).await?;
        }
        // CLI self-update
        Some(Command::Update { command }) => {
            handle_update(command).await?;
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
fn handle_config(
    settings: &Settings,
    cmd: ConfigCommand,
    workspace_root: &std::path::Path,
) -> Result<()> {
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
            if let Ok(snapshot) =
                runtime_config::config_manager::load_project_config(workspace_root)
            {
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
            let mut all_issues = Vec::new();

            // Legacy settings validation
            let mut missing = settings.missing_chat_requirements();
            if let Some(soul_id) = crate::formula::current_project_soul() {
                missing.retain(|item| item != "soul");
                if crate::formula::load_soul_prompts(&soul_id).is_none() {
                    missing.push("soul prompts (re-install required)".to_string());
                }
            }
            if !missing.is_empty() {
                all_issues.extend(missing.iter().map(|m| format!("Legacy: {}", m)));
            }

            // Project config validation
            match runtime_config::config_manager::load_project_config(workspace_root) {
                Ok(snapshot) => {
                    // Validate model assignment
                    let model_issues = runtime_config::loaders::validate_model_assignment(
                        &snapshot.model_assignment,
                        &snapshot.providers,
                    );
                    all_issues.extend(model_issues);

                    // Validate permissions
                    if snapshot.permissions.shell.enabled
                        && snapshot.permissions.shell.denylist.is_empty()
                    {
                        all_issues.push(
                            "Permissions: Shell enabled but denylist is empty (security risk)"
                                .to_string(),
                        );
                    }
                }
                Err(e) => {
                    all_issues.push(format!("Project config: {}", e));
                }
            }

            if all_issues.is_empty() {
                println!("{}", t!("cli.configuration_valid"));
            } else {
                println!(
                    "{}",
                    t!(
                        "cli.configuration_incomplete",
                        items = all_issues.join(", ")
                    )
                );
                println!("{}", t!("cli.run_init_hint"));
                return Err(anyhow::anyhow!(
                    "Configuration validation failed with {} issue(s)",
                    all_issues.len()
                ));
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
            println!("Updating souls...");
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
            // Resolve repo: bundled first, then legacy cache
            let repo = crate::formula::bundled_souls_path().or_else(|| {
                let legacy = crate::formula::official_repo_path();
                if legacy.exists() {
                    Some(legacy)
                } else {
                    None
                }
            });
            let repo = match repo {
                Some(r) => r,
                None => {
                    println!("Soul sources not found. Run: alius soul update");
                    return Ok(());
                }
            };
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
                None => println!("Soul not found: {}", id),
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
        PluginCommand::Install { path, yes } => {
            let source = std::path::PathBuf::from(&path);

            // Phase 1: Plan — validate manifest, permissions, detect upgrade.
            // No files are copied yet; old plugin remains intact on denial.
            let plan = crate::plugin::plan_plugin_install(&source)?;

            // Show upgrade info if applicable
            if let Some(ref info) = plan.upgrade_info {
                println!(
                    "Upgrading '{}' from v{} to v{}",
                    plan.manifest.id, info.old_version, info.new_version
                );
                if info.permissions_changed {
                    println!("  WARNING: Permissions have changed!");
                }
            }

            // Show permissions and prompt for confirmation if needed.
            let needs_prompt = !plan.summary.is_empty() && !yes;
            if needs_prompt {
                if plan.upgrade_info.is_some()
                    && plan.upgrade_info.as_ref().unwrap().permissions_changed
                {
                    println!("New permissions:");
                } else if plan.upgrade_info.is_none() {
                    println!(
                        "Plugin '{}' v{} requests the following permissions:",
                        plan.manifest.id, plan.manifest.version
                    );
                }
                for line in &plan.summary {
                    println!("{}", line);
                }

                // Non-interactive detection: if stdin is not a TTY and --yes was not
                // provided, fail closed rather than silently defaulting to "no".
                use std::io::IsTerminal;
                if !std::io::stdin().is_terminal() {
                    anyhow::bail!(
                        "Plugin requires permissions but no terminal detected. \
                         Re-run with --yes to approve."
                    );
                }

                print!("Install this plugin? [y/N] ");
                use std::io::Write;
                std::io::stdout().flush().ok();
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();
                if !input.trim().eq_ignore_ascii_case("y") {
                    // No files were copied — old plugin is intact.
                    anyhow::bail!("Installation cancelled by user");
                }
            }

            // Phase 2: Apply — copy files after confirmation.
            crate::plugin::apply_plugin_install(&plan)?;

            println!(
                "Installed plugin '{}' v{}",
                plan.manifest.id, plan.manifest.version
            );
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
        PluginCommand::Publish { path, output } => {
            handle_plugin_publish(&path, output.as_deref())?;
        }
    }
    Ok(())
}

/// Handle plugin publish: validate and package for distribution.
fn handle_plugin_publish(path: &str, output: Option<&str>) -> Result<()> {
    use std::path::PathBuf;

    let source = PathBuf::from(path);

    // Phase 1: Validate manifest and WASM module
    println!("Validating plugin at {}...", source.display());

    let plan = crate::plugin::plan_plugin_install(&source)?;
    let manifest = &plan.manifest;

    // Validate WASM module
    let wasm_path = source.join("plugin.wasm");
    if !wasm_path.exists() {
        anyhow::bail!("plugin.wasm not found at {}", wasm_path.display());
    }
    let wasm_bytes = std::fs::read(&wasm_path)?;
    runtime_tools::wasm_host::validate_wasm_module(&wasm_bytes)?;

    // Discover tools in the WASM module
    let tools = runtime_tools::wasm_host::list_plugin_tools(&wasm_bytes)?;

    println!("Validation passed:");
    println!("  ID:          {}", manifest.id);
    println!("  Name:        {}", manifest.name);
    println!("  Version:     {}", manifest.version);
    println!("  Description: {}", manifest.description);
    if let Some(author) = &manifest.author {
        println!("  Author:      {}", author);
    }
    println!("  Tools:       {}", tools.len());
    for t in &tools {
        println!("    - {}: {}", t.name, t.description);
    }
    if !plan.summary.is_empty() {
        println!("  Permissions:");
        for line in &plan.summary {
            println!("    {}", line);
        }
    }

    // Phase 2: Package
    let output_dir = output
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let package_name = format!("{}-{}.tar.gz", manifest.id, manifest.version);
    let package_path = output_dir.join(&package_name);

    // Create tar.gz archive
    let tar_gz = std::fs::File::create(&package_path)?;
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);

    // Add plugin.toml
    let toml_path = source.join("plugin.toml");
    if toml_path.exists() {
        tar.append_path_with_name(&toml_path, "plugin.toml")?;
    }

    // Add plugin.wasm
    tar.append_path_with_name(&wasm_path, "plugin.wasm")?;

    // Add README if present
    let readme_path = source.join("README.md");
    if readme_path.exists() {
        tar.append_path_with_name(&readme_path, "README.md")?;
    }

    tar.finish()?;

    let size = std::fs::metadata(&package_path)?.len();
    println!();
    println!("Package created: {}", package_path.display());
    println!("  Size: {} bytes", size);
    println!();
    println!(
        "To install: alius plugin install {}",
        package_path.display()
    );

    Ok(())
}

/// Handle MCP server management subcommands.
fn handle_mcp(cmd: McpCommand) -> Result<()> {
    match cmd {
        McpCommand::List => {
            let servers = crate::mcp::list_configured_servers()?;
            if servers.is_empty() {
                println!("No MCP servers configured.");
                println!("Config locations (merged, later overrides earlier):");
                println!("  User:    ~/.alius/mcp/servers.toml");
                println!("  Project: .alius/config/mcp.json");
                println!("  Legacy:  .alius/mcp.json");
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
async fn handle_workflow(
    cmd: WorkflowCommand,
    settings: &Settings,
    workspace_root: &std::path::Path,
) -> Result<()> {
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

            // Build real CoreRuntimeManager and ToolRegistry for workflow execution.
            let manager =
                CoreRuntimeManager::new_local(workspace_root.to_path_buf(), settings.clone())
                    .map_err(|e| anyhow::anyhow!("Failed to build runtime: {}", e))?;
            let registry = manager
                .tool_registry()
                .ok_or_else(|| anyhow::anyhow!("No tool registry available"))?;
            let handle = crate::workflow::RuntimeWorkflowHandle::new(manager, registry);
            let (_ctx, record) =
                crate::workflow::execute_workflow(&workflow, &handle, None).await?;
            println!(
                "Run record: status={:?}, duration={}ms, steps={}",
                record.status,
                record.duration_ms,
                record.steps.len()
            );
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

/// Handle CLI self-update subcommands.
async fn handle_update(command: Option<UpdateCommand>) -> Result<()> {
    match command {
        Some(UpdateCommand::Install) | None => {
            let install = matches!(command, Some(UpdateCommand::Install));

            // Check install method first.
            let method = updater::platform::detect_install_method();
            match method {
                updater::platform::InstallMethod::Npm => {
                    println!("Installed via npm. Run: npm update -g @alius-tech/alius");
                    return Ok(());
                }
                updater::platform::InstallMethod::Homebrew => {
                    println!("Installed via Homebrew. Run: brew upgrade alius");
                    return Ok(());
                }
                updater::platform::InstallMethod::Development => {
                    println!("Development build detected. Self-update is not available.");
                    return Ok(());
                }
                updater::platform::InstallMethod::Standalone => {}
            }

            println!("{}", t!("update.checking"));
            match updater::check_for_update().await {
                Ok(Some(info)) => {
                    println!(
                        "{}",
                        t!(
                            "update.available",
                            current = info.current,
                            latest = info.latest
                        )
                    );

                    if install {
                        println!(
                            "{}",
                            t!("update.download_start", version = info.latest.as_str())
                        );
                        updater::perform_update(&info).await?;
                        println!("{}", t!("update.installed", version = info.latest.as_str()));
                        println!("{}", t!("update.installed_restart"));
                    } else {
                        println!("{}", t!("update.hint_install"));
                    }
                }
                Ok(None) => {
                    println!(
                        "{}",
                        t!("update.up_to_date", version = env!("ALIUS_VERSION"))
                    );
                }
                Err(e) => {
                    eprintln!("{}", t!("update.error_check", error = e.to_string()));
                }
            }
            let _ = updater::record_check_time();
        }
        Some(UpdateCommand::Check) => {
            let method = updater::platform::detect_install_method();
            match method {
                updater::platform::InstallMethod::Npm => {
                    println!("Installed via npm. Run: npm update -g @alius-tech/alius");
                    return Ok(());
                }
                updater::platform::InstallMethod::Homebrew => {
                    println!("Installed via Homebrew. Run: brew upgrade alius");
                    return Ok(());
                }
                updater::platform::InstallMethod::Development => {
                    println!("Development build detected. Self-update is not available.");
                    return Ok(());
                }
                updater::platform::InstallMethod::Standalone => {}
            }

            println!("{}", t!("update.checking"));
            match updater::check_for_update().await {
                Ok(Some(info)) => {
                    println!(
                        "{}",
                        t!(
                            "update.available",
                            current = info.current,
                            latest = info.latest
                        )
                    );
                    println!("{}", t!("update.hint_install"));
                }
                Ok(None) => {
                    println!(
                        "{}",
                        t!("update.up_to_date", version = env!("ALIUS_VERSION"))
                    );
                }
                Err(e) => {
                    eprintln!("{}", t!("update.error_check", error = e.to_string()));
                }
            }
            let _ = updater::record_check_time();
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
    if let Err(e) = rt.block_on(run()) {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}
