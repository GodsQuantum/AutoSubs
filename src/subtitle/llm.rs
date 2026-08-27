use crate::domain::{Settings, SubtitleLine};
use reqwest::Client;
use serde_json::json;
use tokio_util::sync::CancellationToken;

pub async fn correct_lines(lines: Vec<SubtitleLine>, settings: &Settings, client: &Client, token: &CancellationToken) -> Vec<SubtitleLine> {
    if !settings.llm_enabled || settings.llm_endpoint.trim().is_empty() || settings.llm_model.trim().is_empty() || lines.is_empty() { return lines; }
    let marker = "<AUTOSUBS_LINE>";
    let input = lines.iter().enumerate().map(|(i, line)| format!("{marker}{i}:{}", line.text.replace('\n', " "))).collect::<Vec<_>>().join("\n");
    let body = json!({
        "model": settings.llm_model,
        "temperature": 0,
        "messages": [
            {"role":"system","content": settings.llm_prompt},
            {"role":"user","content": format!("Keep every {marker}N: prefix unchanged and return exactly {} lines.\n{}", lines.len(), input)}
        ]
    });
    let mut request = client.post(&settings.llm_endpoint).json(&body);
    if !settings.llm_api_key.trim().is_empty() { request = request.bearer_auth(&settings.llm_api_key); }
    let response = tokio::select! { r = request.send() => r, _ = token.cancelled() => return lines };
    let Ok(response) = response else { return lines; };
    if !response.status().is_success() { return lines; }
    let Ok(value) = response.json::<serde_json::Value>().await else { return lines; };
    let Some(content) = value.pointer("/choices/0/message/content").and_then(|v| v.as_str()) else { return lines; };
    let mut corrected = vec![None::<String>; lines.len()];
    for row in content.lines() {
        let Some(rest) = row.trim().strip_prefix(marker) else { continue; };
        let Some((index, text)) = rest.split_once(':') else { continue; };
        if let Ok(index) = index.parse::<usize>() { if index < corrected.len() { corrected[index] = Some(text.trim().to_owned()); } }
    }
    if corrected.iter().any(Option::is_none) { return lines; }
    lines.into_iter().zip(corrected).map(|(mut line, text)| { line.text = text.unwrap_or_default(); line }).collect()
}
