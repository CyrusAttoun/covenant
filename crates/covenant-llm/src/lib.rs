//! LLM integration for Covenant
//!
//! This crate provides:
//! - LLM client for OpenAI and Anthropic APIs
//! - Explanation generation from Covenant AST
//! - Caching layer for explanations

mod cache;
pub mod explain;
mod prompts;
mod types;

pub use cache::ExplanationCache;
pub use explain::{ExplainGenerator, format_explanation};
pub use types::*;

use serde::{Deserialize, Serialize};
use std::env;

/// LLM API client for generating Covenant code and explanations
pub struct LlmClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
    provider: Provider,
}

#[derive(Clone, Copy, Debug)]
pub enum Provider {
    OpenAI,
    Anthropic,
}

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    system: String,
    messages: Vec<Message>,
    max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: String,
}

/// Error type for LLM operations
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("No API key found. Set ANTHROPIC_API_KEY or OPENAI_API_KEY")]
    NoApiKey,
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON parse error: {0}")]
    Json(String),
    #[error("No response from LLM")]
    NoResponse,
    #[error("Generation failed: {0}")]
    Generation(String),
}

impl LlmClient {
    /// Create a new client, auto-detecting provider from environment
    pub fn new() -> Result<Self, LlmError> {
        // Try Anthropic first (preferred)
        if let Ok(api_key) = env::var("ANTHROPIC_API_KEY") {
            return Ok(Self {
                client: reqwest::Client::new(),
                api_key,
                model: "claude-sonnet-4-20250514".to_string(),
                provider: Provider::Anthropic,
            });
        }

        // Try OpenAI
        if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            return Ok(Self {
                client: reqwest::Client::new(),
                api_key,
                model: "gpt-4o".to_string(),
                provider: Provider::OpenAI,
            });
        }

        Err(LlmError::NoApiKey)
    }

    /// Create a client with explicit configuration
    pub fn with_config(provider: Provider, api_key: String, model: Option<String>) -> Self {
        let default_model = match provider {
            Provider::Anthropic => "claude-sonnet-4-20250514".to_string(),
            Provider::OpenAI => "gpt-4o".to_string(),
        };

        Self {
            client: reqwest::Client::new(),
            api_key,
            model: model.unwrap_or(default_model),
            provider,
        }
    }

    /// Get the provider being used
    pub fn provider(&self) -> Provider {
        self.provider
    }

    /// Generate Covenant code from a description
    pub async fn generate_code(&self, description: &str) -> Result<String, LlmError> {
        let system_prompt = prompts::CODE_GENERATION_PROMPT;
        let user_prompt = format!(
            "Generate Covenant code for the following:\n\n{}\n\nOutput only the Covenant code, no explanations.",
            description
        );

        self.call(&system_prompt, &user_prompt).await
    }

    /// Generate explanation for Covenant code
    pub async fn generate_explanation(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, LlmError> {
        self.call(system_prompt, user_prompt).await
    }

    async fn call(&self, system: &str, user: &str) -> Result<String, LlmError> {
        match self.provider {
            Provider::OpenAI => self.call_openai(system, user).await,
            Provider::Anthropic => self.call_anthropic(system, user).await,
        }
    }

    async fn call_openai(&self, system: &str, user: &str) -> Result<String, LlmError> {
        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user.to_string(),
                },
            ],
            temperature: 0.0,
            max_tokens: 4096,
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        let response: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Json(e.to_string()))?;

        response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or(LlmError::NoResponse)
    }

    async fn call_anthropic(&self, system: &str, user: &str) -> Result<String, LlmError> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            system: system.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: user.to_string(),
            }],
            max_tokens: 4096,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| LlmError::Json(e.to_string()))?;

        response
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or(LlmError::NoResponse)
    }
}

/// Extract code blocks from markdown-formatted response
pub fn extract_code(response: &str) -> String {
    // Try to find code block
    if let Some(start) = response.find("```") {
        let after_backticks = &response[start + 3..];
        // Skip optional language identifier
        let code_start = after_backticks.find('\n').map(|i| i + 1).unwrap_or(0);
        let code = &after_backticks[code_start..];
        if let Some(end) = code.find("```") {
            return code[..end].to_string();
        }
    }
    // Return as-is if no code block found
    response.to_string()
}
