use crate::subtitle::normalize::normalize_and_fix_overlaps;
use crate::subtitle::types::{Settings, SubtitleLine};
use serde::Deserialize;

#[derive(Deserialize)]
struct LlmResponse {
    choices: Option<Vec<LlmChoice>>,
}
#[derive(Deserialize)]
struct LlmChoice {
    message: Option<LlmMessage>,
}
#[derive(Deserialize)]
struct LlmMessage {
    content: Option<String>,
}

pub async fn llm_correct_lines(
    lines: Vec<SubtitleLine>,
    settings: &Settings,
    http_client: &reqwest::Client,
) -> Vec<SubtitleLine> {
    if !settings.llm_enabled
        || settings.llm_api_key.is_empty()
        || settings.llm_endpoint.is_empty()
    {
        return lines;
    }

    let text_to_correct = lines
        .iter()
        .map(|l| l.text.replace('\n', " "))
        .collect::<Vec<_>>()
        .join("\n");

    let body = serde_json::json!({
        "model": settings.llm_model,
        "temperature": 0.1,
        "messages": [
            { "role": "system", "content": &settings.llm_prompt },
            { "role": "user",   "content": text_to_correct }
        ]
    });

    let result = http_client
        .post(&settings.llm_endpoint)
        .bearer_auth(&settings.llm_api_key)
        .json(&body)
        .send()
        .await;

    match result {
        Err(e) => {
            tracing::warn!("LLM request failed: {}", e);
            lines
        }
        Ok(resp) => match resp.json::<LlmResponse>().await {
            Err(e) => {
                tracing::warn!("LLM response parse failed: {}", e);
                lines
            }
            Ok(data) => {
                let content = data
                    .choices
                    .as_deref()
                    .and_then(|c| c.first())
                    .and_then(|c| c.message.as_ref())
                    .and_then(|m| m.content.as_deref())
                    .unwrap_or("");

                let corrected: Vec<&str> = content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .collect();

                if corrected.len() == lines.len() {
                    let updated: Vec<SubtitleLine> = lines
                        .into_iter()
                        .zip(corrected)
                        .map(|(mut l, text)| { l.text = text.to_string(); l })
                        .collect();
                    normalize_and_fix_overlaps(&updated)
                } else {
                    tracing::warn!(
                        "LLM returned {} lines, expected {}. Keeping original.",
                        corrected.len(),
                        lines.len()
                    );
                    lines
                }
            }
        },
    }
}
