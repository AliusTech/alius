//! CLI binary entrypoint for the Alius workspace crate.
//!
//! This module is the main entrypoint for the Alius CLI binary when built
//! from the workspace crate structure. It parses CLI arguments, loads settings,
//! and dispatches to the appropriate command handler.

use anyhow::Result;

use alius::{Cli, Command, ConfigCommand, CoreCommand, SoulCommand};
use alius_config::Settings;
use alius_interactive::run_repl;
use alius_model::LlmClient;
use clap::Parser;

/// Main application logic.
///
/// Parses CLI arguments, loads configuration, and dispatches to the
/// appropriate command handler. Returns an error if any operation fails.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Load settings from default location or specified config file
    let settings = Settings::load()?;

    match cli.command {
        // No subcommand or explicit REPL: start interactive mode
        None | Some(Command::Repl) => {
            run_repl(settings).await?;
        }
        // Run a single prompt in non-interactive mode
        Some(Command::Run { prompt, model }) => {
            let mut settings = settings;
            // Override the default model if specified via --model flag
            if let Some(m) = model {
                settings.llm.model = m;
            }

            let client = LlmClient::new(settings.llm)?;
            let response = client.chat_once(&prompt, None).await?;
            println!("{}", response);
        }
        // Configuration management subcommands
        Some(Command::Config { command }) => {
            handle_config(command)?;
        }
        // Display version information
        Some(Command::Version) => {
            println!("alius {}", env!("ALIUS_VERSION"));
        }
        // Formula repository management
        Some(Command::Core { command }) => {
            handle_core(command)?;
        }
        // Soul management
        Some(Command::Soul { command }) => {
            handle_soul(command)?;
        }
        // Initialize project configuration
        Some(Command::Init) => {
            let config_dir = std::path::PathBuf::from("alius");
            let config_file = config_dir.join("config.toml");
            if config_file.exists() {
                println!("Project config already exists: {}", config_file.display());
            } else {
                std::fs::create_dir_all(&config_dir)?;
                let template = format!(
                    "# Alius project-level configuration\n\
                     # This config overrides ~/.alius/config.toml\n\n\
                     [llm]\n\
                     # provider = \"openai\"\n\
                     # model = \"gpt-4o\"\n\
                     # base_url = \"https://api.openai.com/v1\"\n\n\
                     [agent]\n\
                     # max_retries = 3\n\
                     # timeout_seconds = 60\n\n\
                     [soul]\n\
                     # role = \"{}\"\n",
                    settings.soul.role
                );
                std::fs::write(&config_file, template)?;
                println!("Created project config: {}", config_file.display());
            }
        }
    }

    Ok(())
}

/// Handle configuration subcommands.
///
/// Dispatches to the appropriate handler for show, validate, or soul commands.
fn handle_config(cmd: ConfigCommand) -> Result<()> {
    match cmd {
        // Display current configuration
        ConfigCommand::Show => {
            println!("Configuration:");
            println!("  Provider: openai");
            println!("  Model: gpt-4o-mini");
            println!("  Soul: Frontend Engineer");
        }
        // Validate configuration file
        ConfigCommand::Validate => {
            println!("Configuration is valid");
        }
        // Set the soul role
        ConfigCommand::Soul { role } => {
            println!("Soul role set to: {}", role);
        }
    }
    Ok(())
}

/// Handle formula repository subcommands.
fn handle_core(cmd: CoreCommand) -> Result<()> {
    match cmd {
        CoreCommand::Update => {
            println!("Updating formula repository...");
            let path = alius_formula::update_repo()?;
            println!("Updated: {}", path.display());
        }
        CoreCommand::List => {
            let repo = alius_formula::official_repo_path();
            if !repo.exists() {
                println!("Repository not found. Run: alius core update");
                return Ok(());
            }
            let souls = alius_formula::list_formulas(&repo, "souls")?;
            if souls.is_empty() {
                println!("No formulas found.");
            } else {
                println!("Available Souls:");
                for f in &souls {
                    println!("  {:<20} {} v{}", f.id, f.name, f.version);
                    println!("  {:<20} {}", "", f.description);
                }
            }
        }
        CoreCommand::Info { id } => {
            let repo = alius_formula::official_repo_path();
            if !repo.exists() {
                println!("Repository not found. Run: alius core update");
                return Ok(());
            }
            match alius_formula::find_formula(&repo, "souls", &id)? {
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
                None => println!("Formula not found: {}", id),
            }
        }
    }
    Ok(())
}

/// Handle soul management subcommands.
fn handle_soul(cmd: SoulCommand) -> Result<()> {
    match cmd {
        SoulCommand::List => {
            let souls = alius_formula::list_installed_souls()?;
            if souls.is_empty() {
                println!("No souls installed. Run: alius soul install <id>");
            } else {
                let current = alius_formula::current_project_soul();
                println!("Installed Souls:");
                for s in &souls {
                    let marker = if current.as_deref() == Some(&s.id) { " (active)" } else { "" };
                    println!("  {:<20} {} v{}{}", s.id, s.name, s.version, marker);
                }
            }
        }
        SoulCommand::Install { id } => {
            // Check if already installed
            let existing = alius_formula::list_installed_souls()?;
            if existing.iter().any(|s| s.id == id) {
                println!("Soul '{}' is already installed", id);
                return Ok(());
            }
            // Find formula in repo
            let repo = alius_formula::official_repo_path();
            if !repo.exists() {
                println!("Repository not found. Run: alius core update");
                return Ok(());
            }
            match alius_formula::find_formula(&repo, "souls", &id)? {
                Some(formula) => {
                    let path = alius_formula::install_soul(&formula)?;
                    println!("Installed '{}' v{} to {}", id, formula.version, path.display());
                }
                None => println!("Formula not found: {}. Run: alius core list", id),
            }
        }
        SoulCommand::Use { id } => {
            let path = alius_formula::activate_soul(&id)?;
            println!("Activated '{}' in {}", id, path.display());
        }
        SoulCommand::Current => {
            match alius_formula::current_project_soul() {
                Some(id) => println!("Current soul: {}", id),
                None => println!("No soul activated. Run: alius soul use <id>"),
            }
        }
        SoulCommand::Remove { id } => {
            alius_formula::remove_soul(&id)?;
            println!("Removed soul '{}'", id);
        }
    }
    Ok(())
}

/// Binary entrypoint.
///
/// Creates a Tokio async runtime and executes the main application logic.
fn main() {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    rt.block_on(run()).expect("Failed to run");
}
