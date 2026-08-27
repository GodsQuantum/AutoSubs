use crate::domain::{Settings, SubtitleLine};
use crate::subtitle::normalize::{normalize_subtitles, NormalizeOptions};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize)]
struct OpenAiResponse { choices: Vec<Choice> }
#[derive(Debug, Deserialize)]
struct Choice { message: Message }
#[derive(Debug, Deserialize)]
struct Message { content: String }

fn parse_corrected_array(content: &str) -> Option<Vec<String>> {
    if let Ok(value) = serde_json::from_str::<Vec<String>>(content.trim()) { return Some(value); }
    let start = content.find('[')?;
    let end = content.rfind(']')?;
    serde_json::from_str::<Vec<String>>(&content[start..=end]).ok()
}

pub async fn correct_lines(
    lines: Vec<SubtitleLine>,
    settings: &Settings,
    client: &reqwest::Client,
    token: &CancellationToken,
) -> Vec<SubtitleLine> {
    if !settings.llm_enabled || settings.llm_endpoint.trim().is_empty() || settings.llm_model.trim().is_empty() {
        return lines;
    }
    let payload: Vec<String> = lines.iter().map(|line| line.text.clone()).collect();
    let user = match serde_json::to_string(&payload) { Ok(value) => value, Err(_) => return lines };
    let system = format!(
        "{}\nInput is a JSON array of subtitle blocks. Return ONLY a JSON array of strings with exactly the same number of elements. Do not add markdown.",
        settings.llm_prompt
    );
    let body = serde_json::json!({
        "model": settings.llm_model,
        "temperature": 0.1,
        "messages": [
            {"role":"system","content":system},
            {"role":"user","content":user}
        ]
    });
    let mut request = client.post(&settings.llm_endpoint).json(&body);
    if !settings.llm_api_key.trim().is_empty() { request = request.bearer_auth(&settings.llm_api_key); }
    let response = tokio::select! {
        response = request.send() => match response { Ok(value) => value, Err(error) => { tracing::warn!(%error, "LLM request failed"); return lines; } },
        _ = token.cancelled() => return lines,
    };
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "LLM request returned non-success");
        return lines;
    }
    let parsed = tokio::select! {
        parsed = response.json::<OpenAiResponse>() => match parsed { Ok(value) => value, Err(error) => { tracing::warn!(%error, "LLM response parse failed"); return lines; } },
        _ = token.cancelled() => return lines,
    };
    let Some(content) = parsed.choices.first().map(|choice| choice.message.content.as_str()) else { return lines; };
    let Some(corrected) = parse_corrected_array(content) else { return lines; };
    if corrected.len() != lines.len() {
        tracing::warn!(got = corrected.len(), expected = lines.len(), "LLM changed subtitle block count; ignoring correction");
        return lines;
    }
    let updated = lines.into_iter().zip(corrected).map(|(mut line, text)| { line.text = text; line }).collect::<Vec<_>>();
    normalize_subtitles(&updated, NormalizeOptions::default()).lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_or_wrapped_json_array() {
        assert_eq!(parse_corrected_array("[\"a\",\"b\"]").unwrap(), vec!["a","b"]);
        assert_eq!(parse_corrected_array("```json\n[\"a\",\"b\"]\n```").unwrap(), vec!["a","b"]);
    }
}
