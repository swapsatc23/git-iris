use crate::commands;
use crate::llm::get_available_provider_names;
use crate::log_debug;
use crate::ui;
use clap::builder::{styling::AnsiColor, Styles};
use clap::{crate_version, Parser, Subcommand};

/// CLI structure defining the available commands and global arguments
#[derive(Parser)]
#[command(
    author,
    version = crate_version!(),
    about = "AI-assisted Git commit message generator",
    long_about = None,
    disable_version_flag = true,
    after_help = get_dynamic_help(),
    styles = get_styles(),
)]
pub struct Cli {
    /// Subcommands available for the CLI
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Log debug messages to a file
    #[arg(
        short = 'l',
        long = "log",
        global = true,
        help = "Log debug messages to a file"
    )]
    pub log: bool,

    /// Display the version
    #[arg(
        short = 'v',
        long = "version",
        global = true,
        help = "Display the version"
    )]
    pub version: bool,
}

/// Enumeration of available subcommands
#[derive(Subcommand)]
pub enum Commands {
    /// Generate a commit message using AI
    #[command(
        about = "Generate a commit message using AI",
        long_about = "Generate a commit message using AI based on the current Git context.",
        after_help = get_dynamic_help()
    )]
    Gen {
        /// Automatically commit with the generated message
        #[arg(short, long, help = "Automatically commit with the generated message")]
        auto_commit: bool,

        /// Custom instructions for this commit
        #[arg(short, long, help = "Custom instructions for this commit")]
        instructions: Option<String>,

        /// Override default LLM provider
        #[arg(long, help = "Override default LLM provider", value_parser = available_providers_parser)]
        provider: Option<String>,

        /// Disable Gitmoji for this commit
        #[arg(long, help = "Disable Gitmoji for this commit")]
        no_gitmoji: bool,

        /// Select an instruction preset
        #[arg(long, help = "Select an instruction preset")]
        preset: Option<String>,

        /// Print the generated message to stdout and exit
        #[arg(short, long, help = "Print the generated message to stdout and exit")]
        print: bool,
    },
    /// Configure the AI-assisted Git commit message generator
    #[command(about = "Configure the AI-assisted Git commit message generator")]
    Config {
        /// Set default LLM provider
        #[arg(long, help = "Set default LLM provider", value_parser = available_providers_parser)]
        provider: Option<String>,

        /// Set API key for the specified provider
        #[arg(long, help = "Set API key for the specified provider")]
        api_key: Option<String>,

        /// Set model for the specified provider
        #[arg(long, help = "Set model for the specified provider")]
        model: Option<String>,

        /// Set token limit for the specified provider
        #[arg(long, help = "Set token limit for the specified provider")]
        token_limit: Option<usize>,

        /// Set additional parameters for the specified provider
        #[arg(
            long,
            help = "Set additional parameters for the specified provider (key=value)"
        )]
        param: Option<Vec<String>>,

        /// Set Gitmoji usage preference
        #[arg(long, help = "Enable or disable Gitmoji")]
        gitmoji: Option<bool>,

        /// Set instructions for the commit message generation
        #[arg(
            short,
            long,
            help = "Set instructions for the commit message generation"
        )]
        instructions: Option<String>,

        /// Set default instruction preset
        #[arg(long, help = "Set default instruction preset")]
        preset: Option<String>,
    },
    /// List available instruction presets
    #[command(about = "List available instruction presets")]
    ListPresets,
    /// Generate a changelog
    #[command(
        about = "Generate a changelog",
        long_about = "Generate a changelog between two specified Git references."
    )]
    Changelog {
        /// Starting Git reference (commit hash, tag, or branch name)
        #[arg(long, required = true)]
        from: String,

        /// Ending Git reference (commit hash, tag, or branch name). Defaults to HEAD if not specified.
        #[arg(long)]
        to: Option<String>,

        /// Custom instructions for changelog generation
        #[arg(short, long, help = "Custom instructions for changelog generation")]
        instructions: Option<String>,

        /// Select an instruction preset for changelog generation
        #[arg(long, help = "Select an instruction preset for changelog generation")]
        preset: Option<String>,

        /// Set the detail level for the changelog
        #[arg(long, help = "Set the detail level (minimal, standard, detailed)", default_value = "standard")]
        detail_level: String,

        /// Enable or disable Gitmoji in the changelog
        #[arg(long, help = "Enable or disable Gitmoji in the changelog")]
        gitmoji: Option<bool>,
    },
    /// Generate release notes
    #[command(
        about = "Generate release notes",
        long_about = "Generate comprehensive release notes between two specified Git references."
    )]
    ReleaseNotes {
        /// Starting Git reference (commit hash, tag, or branch name)
        #[arg(long, required = true)]
        from: String,

        /// Ending Git reference (commit hash, tag, or branch name). Defaults to HEAD if not specified.
        #[arg(long)]
        to: Option<String>,

        /// Custom instructions for release notes generation
        #[arg(short, long, help = "Custom instructions for release notes generation")]
        instructions: Option<String>,

        /// Select an instruction preset for release notes generation
        #[arg(long, help = "Select an instruction preset for release notes generation")]
        preset: Option<String>,

        /// Set the detail level for the release notes
        #[arg(long, help = "Set the detail level (minimal, standard, detailed)", default_value = "standard")]
        detail_level: String,

        /// Enable or disable Gitmoji in the release notes
        #[arg(long, help = "Enable or disable Gitmoji in the release notes")]
        gitmoji: Option<bool>,
    },
}

/// Define custom styles for Clap
fn get_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Magenta.on_default().bold())
        .usage(AnsiColor::Cyan.on_default().bold())
        .literal(AnsiColor::Green.on_default().bold())
        .placeholder(AnsiColor::Yellow.on_default())
        .valid(AnsiColor::Blue.on_default().bold())
        .invalid(AnsiColor::Red.on_default().bold())
        .error(AnsiColor::Red.on_default().bold())
}

/// Parse the command-line arguments
pub fn parse_args() -> Cli {
    Cli::parse()
}

/// Generate dynamic help including available LLM providers
fn get_dynamic_help() -> String {
    let providers = get_available_provider_names().join(", ");
    format!("Available providers: {}", providers)
}

/// Validate provider input against available providers
fn available_providers_parser(s: &str) -> Result<String, String> {
    let available_providers = get_available_provider_names();
    if available_providers.contains(&s.to_lowercase()) && s.to_lowercase() != "test" {
        Ok(s.to_lowercase())
    } else {
        Err(format!(
            "Invalid provider. Available providers are: {}",
            available_providers.join(", ")
        ))
    }
}

/// Main function to parse arguments and handle the command
pub async fn main() -> anyhow::Result<()> {
    let cli = parse_args();

    if cli.version {
        ui::print_version(crate_version!());
        return Ok(());
    }

    if cli.log {
        crate::logger::enable_logging();
    } else {
        crate::logger::disable_logging();
    }

    match cli.command {
        Some(command) => handle_command(command).await?,
        None => {
            // If no subcommand is provided, print the help
            let _ = Cli::parse_from(&["git-iris", "--help"]);
        }
    }

    Ok(())
}

/// Handle the command based on parsed arguments
pub async fn handle_command(command: Commands) -> anyhow::Result<()> {
    match command {
        Commands::Gen {
            auto_commit,
            instructions,
            provider,
            no_gitmoji,
            preset,
            print,
        } => {
            log_debug!(
                "Handling 'gen' command with auto_commit: {}, instructions: {:?}, provider: {:?}, no_gitmoji: {}, preset: {:?}, print: {}",
                auto_commit,
                instructions,
                provider,
                no_gitmoji,
                preset,
                print
            );

            ui::print_version(crate_version!());
            println!();

            commands::handle_gen_command(
                !no_gitmoji,
                provider,
                auto_commit,
                instructions,
                preset,
                print,
            )
            .await?;
        }
        Commands::Config {
            provider,
            api_key,
            model,
            param,
            gitmoji,
            instructions,
            token_limit,
            preset,
        } => {
            log_debug!("Handling 'config' command with provider: {:?}, api_key: {:?}, model: {:?}, param: {:?}, gitmoji: {:?}, instructions: {:?}, token_limit: {:?}, preset: {:?}",
                       provider, api_key, model, param, gitmoji, instructions, token_limit, preset);
            commands::handle_config_command(
                provider,
                api_key,
                model,
                param,
                gitmoji,
                instructions,
                token_limit,
                preset,
            )?;
        }
        Commands::ListPresets => {
            log_debug!("Handling 'list_presets' command");
            commands::handle_list_presets_command()?;
        }
        Commands::Changelog { from, to, instructions, preset, detail_level, gitmoji } => {
            log_debug!(
                "Handling 'changelog' command with from: {}, to: {:?}, instructions: {:?}, preset: {:?}, detail_level: {}, gitmoji: {:?}",
                from, to, instructions, preset, detail_level, gitmoji
            );
            commands::handle_changelog_command(from, to, instructions, preset, detail_level, gitmoji).await?;
        }
        Commands::ReleaseNotes { from, to, instructions, preset, detail_level, gitmoji } => {
            log_debug!(
                "Handling 'release-notes' command with from: {}, to: {:?}, instructions: {:?}, preset: {:?}, detail_level: {}, gitmoji: {:?}",
                from, to, instructions, preset, detail_level, gitmoji
            );
            commands::handle_release_notes_command(from, to, instructions, preset, detail_level, gitmoji).await?;
        }
    }

    Ok(())
}