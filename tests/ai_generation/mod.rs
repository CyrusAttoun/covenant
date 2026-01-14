//! AI Generation Tests
//!
//! Tests that LLMs can generate valid Covenant code that parses and type-checks.

mod prompts;
mod example_descriptions;

use std::env;
use serde::{Deserialize, Serialize};

/// LLM API client for generating Covenant code
pub struct LlmClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
    provider: Provider,
}

#[derive(Clone, Copy)]
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

impl LlmClient {
    /// Create a new client, auto-detecting provider from environment
    pub fn new() -> Option<Self> {
        // Try OpenAI first
        if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            return Some(Self {
                client: reqwest::Client::new(),
                api_key,
                model: "gpt-5.2".to_string(),
                provider: Provider::OpenAI,
            });
        }

        // Try Anthropic
        if let Ok(api_key) = env::var("ANTHROPIC_API_KEY") {
            return Some(Self {
                client: reqwest::Client::new(),
                api_key,
                model: "claude-3-5-sonnet-20241022".to_string(),
                provider: Provider::Anthropic,
            });
        }

        None
    }

    /// Generate Covenant code from a description
    pub async fn generate(&self, description: &str) -> Result<String, String> {
        let system_prompt = prompts::SYSTEM_PROMPT;
        let user_prompt = format!(
            "Generate Covenant code for the following:\n\n{}\n\nOutput only the Covenant code, no explanations.",
            description
        );

        match self.provider {
            Provider::OpenAI => self.call_openai(&system_prompt, &user_prompt).await,
            Provider::Anthropic => self.call_anthropic(&system_prompt, &user_prompt).await,
        }
    }

    async fn call_openai(&self, system: &str, user: &str) -> Result<String, String> {
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
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;

        let response: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| format!("JSON parse error: {}", e))?;

        response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| "No response from OpenAI".to_string())
    }

    async fn call_anthropic(&self, system: &str, user: &str) -> Result<String, String> {
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
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;

        let response: AnthropicResponse = response
            .json()
            .await
            .map_err(|e| format!("JSON parse error: {}", e))?;

        response
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| "No response from Anthropic".to_string())
    }
}

/// Result of testing a single example
#[derive(Debug, Serialize)]
pub struct TestResult {
    pub file: String,
    pub description: String,
    pub parsed: bool,
    pub checked: bool,
    pub error: Option<String>,
}

/// Extract code blocks from markdown-formatted response
pub fn extract_code(response: &str) -> String {
    // Try to find code block
    if let Some(start) = response.find("```") {
        let after_backticks = &response[start + 3..];
        // Skip optional language identifier
        let code_start = after_backticks
            .find('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let code = &after_backticks[code_start..];
        if let Some(end) = code.find("```") {
            return code[..end].to_string();
        }
    }
    // Return as-is if no code block found
    response.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use example_descriptions::EXAMPLES;

    // This test is ignored by default since it requires API keys
    #[tokio::test]
    #[ignore]
    async fn test_generate_all_examples() {
        let client = match LlmClient::new() {
            Some(c) => c,
            None => {
                eprintln!("Skipping test: No API key found (set OPENAI_API_KEY or ANTHROPIC_API_KEY)");
                return;
            }
        };

        let mut results = Vec::new();

        for example in EXAMPLES {
            println!("Testing: {}", example.file);

            let generated = match client.generate(example.description).await {
                Ok(g) => g,
                Err(e) => {
                    results.push(TestResult {
                        file: example.file.to_string(),
                        description: example.description.to_string(),
                        parsed: false,
                        checked: false,
                        error: Some(format!("Generation failed: {}", e)),
                    });
                    continue;
                }
            };

            let code = extract_code(&generated);

            // Try to parse
            let parsed = match covenant_parser::parse(&code) {
                Ok(program) => {
                    // Try to type check
                    match covenant_checker::check(&program) {
                        Ok(_) => TestResult {
                            file: example.file.to_string(),
                            description: example.description.to_string(),
                            parsed: true,
                            checked: true,
                            error: None,
                        },
                        Err(errors) => TestResult {
                            file: example.file.to_string(),
                            description: example.description.to_string(),
                            parsed: true,
                            checked: false,
                            error: Some(format!("Type check failed: {:?}", errors)),
                        },
                    }
                }
                Err(e) => TestResult {
                    file: example.file.to_string(),
                    description: example.description.to_string(),
                    parsed: false,
                    checked: false,
                    error: Some(format!("Parse failed: {}", e)),
                },
            };

            results.push(parsed);
        }

        // Print summary
        let total = results.len();
        let parsed = results.iter().filter(|r| r.parsed).count();
        let checked = results.iter().filter(|r| r.checked).count();

        println!("\n=== Results ===");
        println!("Total: {}", total);
        println!("Parsed: {} ({:.1}%)", parsed, (parsed as f64 / total as f64) * 100.0);
        println!("Type checked: {} ({:.1}%)", checked, (checked as f64 / total as f64) * 100.0);

        // Print failures
        for result in &results {
            if !result.checked {
                println!("\nFailed: {}", result.file);
                if let Some(ref error) = result.error {
                    println!("  Error: {}", error);
                }
            }
        }

        // Save results to JSON
        let json = serde_json::to_string_pretty(&results).unwrap();
        std::fs::write("ai_generation_results.json", json).unwrap();
        println!("\nResults saved to ai_generation_results.json");
    }

    #[test]
    fn test_extract_code() {
        let response = "Here is the code:\n```covenant\nsnippet id=\"test\" kind=\"fn\"\nend\n```\nDone!";
        let code = extract_code(response);
        assert!(code.contains("snippet"));
        assert!(!code.contains("```"));
    }
}
