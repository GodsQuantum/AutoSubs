//! Same-origin Cloud9-Speaches discovery: no browser CORS dependency.
//! Only explicitly supplied loopback/private LAN URLs are probed.
//! Protects against SSRF to cloud metadata/link-local and DNS rebinding.
use crate::error::{AppError, AppResult};
use axum::Json;
use reqwest::{Client, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

const MODEL_PREFS: &[&str] = &[
    "deepdml/faster-whisper-large-v3-turbo-ct2",
    "Systran/faster-whisper-large-v3",
];

#[derive(Debug, Deserialize)]
pub struct ProbeRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResponse {
    pub origin: String,
    pub transcription_url: String,
    pub alignment_url: String,
    pub model: String,
    pub alignment_model: &'static str,
    pub backend: String,
    pub backend_verified: bool,
}

fn safe_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            v4.is_loopback()
                || octets[0] == 10
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                || (octets[0] == 192 && octets[1] == 168)
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local(),
    }
}

fn valid_input(raw: &str) -> AppResult<(Url, String, u16)> {
    if raw.len() > 500 {
        return Err(AppError::BadRequest("Speaches URL is too long".into()));
    }
    let parsed =
        Url::parse(raw).map_err(|_| AppError::BadRequest("Invalid Speaches URL".into()))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(
            parsed.path(),
            "/" | "/v1" | "/v1/" | "/v1/audio/transcriptions"
        )
    {
        return Err(AppError::BadRequest(
            "Use a plain HTTP(S) Cloud9-Speaches base URL without credentials or parameters".into(),
        ));
    }
    let hostname = parsed
        .host_str()
        .ok_or_else(|| AppError::BadRequest("Missing hostname".into()))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    // Explicitly refuse public HTTP and arbitrary DNS/URLs. The UI is for
    // this machine or an explicitly named service on its private LAN.
    if hostname != "localhost"
        && !hostname.ends_with(".local")
        && !hostname.parse::<IpAddr>().is_ok_and(safe_ip)
    {
        return Err(AppError::BadRequest(
            "Only localhost, RFC1918/ULA private IPs, or .local hosts may be paired".into(),
        ));
    }
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| AppError::BadRequest("Missing port".into()))?;
    Ok((parsed, hostname, port))
}

async fn pinned_client(hostname: &str, port: u16) -> AppResult<Client> {
    let mut builder = Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(Policy::none())
        .no_proxy();
    if let Ok(ip) = hostname.parse::<IpAddr>() {
        if !safe_ip(ip) {
            return Err(AppError::BadRequest("Unsafe IP address".into()));
        }
    } else {
        let addresses: Vec<SocketAddr> = tokio::net::lookup_host((hostname, port))
            .await
            .map_err(|_| AppError::BadRequest("Speaches host cannot be resolved".into()))?
            .filter(|address| safe_ip(address.ip()))
            .collect();
        if addresses.is_empty() {
            return Err(AppError::BadRequest(
                "Speaches hostname has no permitted private LAN address".into(),
            ));
        }
        builder = builder.resolve_to_addrs(hostname, &addresses);
    }
    builder
        .build()
        .map_err(|e| AppError::BadRequest(format!("Cannot probe Speaches: {e}")))
}

async fn read_json(client: &Client, origin: &str, path: &str) -> AppResult<Value> {
    let response = client
        .get(format!("{origin}{path}"))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Cannot reach Cloud9-Speaches {path}: {e}")))?;
    if !response.status().is_success() {
        return Err(AppError::BadRequest(format!(
            "Cloud9-Speaches {path} returned HTTP {}",
            response.status()
        )));
    }
    if response
        .content_length()
        .is_some_and(|len| len > 256 * 1024)
    {
        return Err(AppError::BadRequest(
            "Speaches response is too large".into(),
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    if bytes.len() > 256 * 1024 {
        return Err(AppError::BadRequest(
            "Speaches response is too large".into(),
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| AppError::BadRequest(format!("Invalid JSON at {path}")))
}

pub async fn probe(Json(request): Json<ProbeRequest>) -> AppResult<Json<ProbeResponse>> {
    let (_, hostname, port) = valid_input(request.url.trim())?;
    let origin = {
        let url = Url::parse(request.url.trim())
            .map_err(|_| AppError::BadRequest("Invalid URL".into()))?;
        url.origin().ascii_serialization()
    };
    let client = pinned_client(&hostname, port).await?;
    let health = read_json(&client, &origin, "/health").await?;
    let models = read_json(&client, &origin, "/v1/models").await?;
    let speaches_healthy = health.get("status").and_then(Value::as_str) == Some("ok")
        || health.get("message").and_then(Value::as_str) == Some("OK");
    if !speaches_healthy {
        return Err(AppError::BadRequest(
            "Cloud9-Speaches is not healthy".into(),
        ));
    }
    let ids = models
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::BadRequest("Speaches returned no model catalog".into()))?;
    let model = MODEL_PREFS
        .iter()
        .find(|preference| {
            ids.iter()
                .any(|item| item.get("id").and_then(Value::as_str) == Some(**preference))
        })
        .ok_or_else(|| AppError::BadRequest("No supported Whisper Turbo model available".into()))?;
    Ok(Json(ProbeResponse {
        transcription_url: format!("{origin}/v1/audio/transcriptions"),
        alignment_url: String::new(),
        origin,
        model: (*model).to_owned(),
        alignment_model: "",
        backend: "whisper-native-words".to_owned(),
        backend_verified: false, // Model metadata alone does not prove word timing.
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_loopback_and_private_lan_only() {
        for address in [
            "http://127.0.0.1:8005",
            "http://192.0.2.42:8005",
            "http://10.20.30.40:8005",
            "http://172.31.255.254:8005",
            "http://[::1]:8005",
            "http://speaches.local:8005",
        ] {
            assert!(valid_input(address).is_ok(), "must allow {address}");
        }
    }

    #[test]
    fn reject_metadata_public_credentials_and_wrong_paths() {
        for address in [
            "http://169.254.169.254/latest/meta-data",
            "http://8.8.8.8:8005",
            "http://example.com:8005",
            "http://192.0.2.42:8005/admin",
            "http://user:pass@127.0.0.1:8005",
            "file:///etc/passwd",
            "http://127.0.0.1:8005?url=x",
            "http://127.0.0.1:8005#fragment",
        ] {
            assert!(valid_input(address).is_err(), "must reject {address}");
        }
    }
}
