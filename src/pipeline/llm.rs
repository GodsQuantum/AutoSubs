use reqwest::Client;
use crate::subtitle::types::{Settings, SubtitleLine};

fn normalize_and_fix_overlaps(lines: Vec<SubtitleLine>) -> Vec<SubtitleLine> {
    // Placeholder implementation for overlap fixing
    lines
}

pub async fn llm_correct_lines(
    lines: Vec<SubtitleLine>,
    settings: &Settings,
    http_client: &Client,
) -> Vec<SubtitleLine> {
    if !settings.llm_enabled || settings.llm_endpoint.is_empty() || settings.llm_api_key.is_empty() {
        return normalize_and_fix_overlaps(lines);
    }

    let input_text = lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n");
    
    let body = serde_json::json!({
        "model": settings.llm_model,
        "temperature": 0.1,
        "messages": [
            { "role": "system", "content": settings.llm_prompt },
            { "role": "user", "content": input_text }
        ]
    });

    let res = http_client.post(&settings.llm_endpoint)
        .bearer_auth(&settings.llm_api_key)
        .json(&body)
        .send()
        .await;
        
    let mut corrected_lines = lines.clone();
        
    match res {
        Ok(response) if response.status().is_success() => {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                    let new_texts: Vec<&str> = content.split('\n').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                    if new_texts.len() == lines.len() {
                        for (i, text) in new_texts.into_iter().enumerate() {
                            corrected_lines[i].text = text.to_string();
                        }
                    } else {
                        log::warn!("LLM returned different number of lines: {} vs {}", new_texts.len(), lines.len());
                    }
                }
            }
        }
        Ok(resp) => {
            log::warn!("LLM request failed with status: {}", resp.status());
        }
        Err(e) => {
            log::warn!("LLM request error: {}", e);
        }
    }

    normalize_and_fix_overlaps(corrected_lines)
}
