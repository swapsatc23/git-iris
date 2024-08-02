use crate::llm_provider::LLMProvider;
use crate::log_debug;
use crate::provider_registry::ProviderRegistry;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Configuration structure for the Git-Iris application
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Config {
    /// Default LLM provider
    pub default_provider: String,
    /// Provider-specific configurations
    pub providers: HashMap<String, ProviderConfig>,
    /// Flag indicating whether to use Gitmoji
    #[serde(default)]
    pub use_gitmoji: bool,
    /// Custom instructions for commit messages
    #[serde(default)]
    pub custom_instructions: String,
}

/// Provider-specific configuration structure
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ProviderConfig {
    /// API key for the provider
    pub api_key: String,
    /// Model to be used with the provider
    pub model: String,
    /// Additional parameters for the provider
    #[serde(default)]
    pub additional_params: HashMap<String, String>,
    /// Custom token limit, if set by the user
    pub custom_token_limit: Option<usize>,
}

impl Config {
    /// Load the configuration from the file
    pub fn load() -> Result<Self> {
        let config_path = Config::get_config_path()?;
        if !config_path.exists() {
            return Ok(Config::default());
        }
        let config_content = fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&config_content)?;
        log_debug!("Configuration loaded: {:?}", config);
        Ok(config)
    }

    /// Save the configuration to the file
    pub fn save(&self) -> Result<()> {
        let config_path = Config::get_config_path()?;
        let config_content = toml::to_string(self)?;
        fs::write(config_path, config_content)?;
        log_debug!("Configuration saved: {:?}", self);
        Ok(())
    }

    /// Get the path to the configuration file
    fn get_config_path() -> Result<PathBuf> {
        dirs::home_dir()
            .ok_or_else(|| anyhow!("Unable to determine home directory"))
            .map(|path| path.join(".git-iris"))
    }

    /// Check the environment for necessary prerequisites
    pub fn check_environment() -> Result<()> {
        crate::git::check_environment()?;

        // Check if we're in a git repository
        if !crate::git::is_inside_work_tree()? {
            return Err(anyhow!(
                "Not in a Git repository. Please run this command from within a Git repository."
            ));
        }

        Ok(())
    }

    /// Update the configuration with new values
    pub fn update(
        &mut self,
        provider: Option<String>,
        api_key: Option<String>,
        model: Option<String>,
        additional_params: Option<HashMap<String, String>>,
        use_gitmoji: Option<bool>,
        custom_instructions: Option<String>,
        token_limit: Option<usize>,
    ) {
        if let Some(provider) = provider {
            self.default_provider = provider.clone();
            if !self.providers.contains_key(&provider) {
                self.providers
                    .insert(provider.clone(), ProviderConfig::default_for(&provider));
            }
        }

        let provider_config = self.providers.get_mut(&self.default_provider).unwrap();

        if let Some(key) = api_key {
            provider_config.api_key = key;
        }
        if let Some(model) = model {
            provider_config.model = model;
        }
        if let Some(params) = additional_params {
            provider_config.additional_params.extend(params);
        }
        if let Some(gitmoji) = use_gitmoji {
            self.use_gitmoji = gitmoji;
        }
        if let Some(instructions) = custom_instructions {
            self.custom_instructions = instructions;
        }
        if let Some(limit) = token_limit {
            provider_config.custom_token_limit = Some(limit);
        }

        log_debug!("Configuration updated: {:?}", self);
    }

    /// Get the configuration for a specific provider
    pub fn get_provider_config(&self, provider: &str) -> Option<&ProviderConfig> {
        self.providers.get(provider)
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut providers = HashMap::new();
        providers.insert("openai".to_string(), ProviderConfig::default_for("openai"));
        providers.insert("claude".to_string(), ProviderConfig::default_for("claude"));

        Config {
            default_provider: "openai".to_string(),
            providers,
            use_gitmoji: false,
            custom_instructions: String::new(),
        }
    }
}

impl ProviderConfig {
    /// Create a default provider configuration for a given provider
    pub fn default_for(provider: &str) -> Self {
        let default_model = ProviderRegistry::default()
            .get_default_model(provider)
            .unwrap_or_else(|| {
                panic!(
                    "Default model for provider '{}' not found in registry",
                    provider
                );
            });

        ProviderConfig {
            api_key: String::new(),
            model: default_model.to_string(),
            additional_params: HashMap::new(),
            custom_token_limit: None,
        }
    }

    /// Get the token limit for this provider configuration
    pub fn get_token_limit(&self, provider: &dyn LLMProvider) -> usize {
        self.custom_token_limit
            .unwrap_or_else(|| provider.default_token_limit())
    }

    /// Convert to LLMProviderConfig
    pub fn to_llm_provider_config(&self) -> crate::llm_provider::LLMProviderConfig {
        crate::llm_provider::LLMProviderConfig {
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            additional_params: self.additional_params.clone(),
        }
    }
}
