use anyhow::Result;
use git_iris::{cli, config, git, llm, prompt};

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::parse_args();
    let mut config = config::Config::load()?;

    match args.command {
        cli::Commands::Gen { verbose, gitmoji } => {
            if let Err(e) = config::Config::check_environment() {
                cli::print_error(&format!("Error: {}", e));
                cli::print_info("\nPlease ensure the following:");
                cli::print_info("1. Git is installed and accessible from the command line.");
                cli::print_info("2. You are running this command from within a Git repository.");
                cli::print_info("3. You have set up your configuration using 'git-iris config'.");
                return Ok(());
            }

            if config.api_key.is_empty() {
                cli::print_error("API key is not set. Please run 'git-iris config --api-key YOUR_API_KEY' to set it.");
                return Ok(());
            }

            let git_info = git::get_git_info()?;

            if git_info.staged_files.is_empty() {
                cli::print_warning("No staged changes. Please stage your changes before generating a commit message.");
                cli::print_info("You can stage changes using 'git add <file>' or 'git add .'");
                return Ok(());
            }

            let use_gitmoji = gitmoji.unwrap_or(config.use_gitmoji);
            let prompt = prompt::create_prompt(&git_info, &config, verbose)?;
            let generated_message = llm::get_refined_message(&prompt, use_gitmoji, verbose).await?;

            cli::print_success("Generated commit message:");
            println!("{}", generated_message);

            if args.auto_commit {
                git::commit(&generated_message)?;
                cli::print_success("Changes committed successfully.");
            } else {
                cli::print_info("\nTo commit with this message, run:");
                println!(
                    "git commit -m \"{}\"",
                    generated_message.replace("\"", "\\\"")
                );
            }
        }
        cli::Commands::Config {
            api_key,
            gitmoji,
            custom_instructions,
        } => {
            config.update(api_key, gitmoji, custom_instructions);
            config.save()?;
            cli::print_success("Configuration updated successfully.");
            cli::print_info(&format!(
                "Current configuration:\nAPI Key: {}\nUse Gitmoji: {}\nCustom Instructions: {}",
                if config.api_key.is_empty() {
                    "Not set"
                } else {
                    "Set"
                },
                config.use_gitmoji,
                if config.custom_instructions.is_empty() {
                    "None".to_string()
                } else {
                    config.custom_instructions.replace('\n', ", ")
                }
            ));
        }
    }

    Ok(())
}
