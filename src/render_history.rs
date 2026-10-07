use crate::{
    domain::{EncoderKind, RenderProfile},
    media::render::EncoderCapabilities,
};
use serde::{Deserialize, Serialize};

pub const MAX_RENDER_HISTORY: usize = 64;
const BENCHMARK_MEDIA_SECONDS: f64 = 12.0;
const BENCHMARK_PIXELS: f64 = 2160.0 * 3840.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderHistorySample {
    pub id: String,
    pub profile: RenderProfile,
    pub encoder: EncoderKind,
    pub media_duration_seconds: f64,
    pub target_pixels: u64,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub successful: bool,
    #[serde(default)]
    pub fallback_count: u32,
    #[serde(default)]
    pub fallback_overhead_ms: u64,
    #[serde(default)]
    pub created_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenderHistory {
    #[serde(default)]
    pub samples: Vec<RenderHistorySample>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EstimateBasis {
    Initial,
    History,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderEstimate {
    pub min_seconds: u64,
    pub max_seconds: u64,
    pub basis: EstimateBasis,
    pub sample_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderProfileOption {
    pub profile: RenderProfile,
    pub encoder: EncoderKind,
    pub estimate: RenderEstimate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
    pub options: Vec<RenderProfileOption>,
    pub actual_encoder: Option<EncoderKind>,
    pub last_elapsed_ms: Option<u64>,
}

pub fn push_sample(history: &mut RenderHistory, sample: RenderHistorySample) {
    history.samples.push(sample);
    if history.samples.len() > MAX_RENDER_HISTORY {
        let drop_count = history.samples.len() - MAX_RENDER_HISTORY;
        history.samples.drain(0..drop_count);
    }
}

fn benchmark_key(encoder: &EncoderKind) -> Option<&'static str> {
    match encoder {
        EncoderKind::NvencH264 => Some("nvenc_h264"),
        EncoderKind::QsvH264 => Some("qsv_h264"),
        EncoderKind::VaapiH264 => Some("vaapi_h264"),
        EncoderKind::VulkanH264 => Some("vulkan_h264"),
        EncoderKind::AmfH264 => Some("amf_h264"),
        _ => None,
    }
}

fn initial_seconds(
    profile: RenderProfile,
    encoder: &EncoderKind,
    media_duration_seconds: f64,
    target_pixels: u64,
    caps: &EncoderCapabilities,
) -> f64 {
    if let Some(ms) = benchmark_key(encoder)
        .and_then(|key| caps.h264_benchmarks_ms.get(key))
        .copied()
    {
        let pixel_factor = (target_pixels as f64 / BENCHMARK_PIXELS).max(0.05);
        return (ms as f64 / 1000.0)
            * (media_duration_seconds.max(0.1) / BENCHMARK_MEDIA_SECONDS)
            * pixel_factor;
    }

    let realtime_factor = match profile {
        RenderProfile::Auto => 0.45,
        RenderProfile::Fast => 0.35,
        RenderProfile::Quality => 0.9,
        RenderProfile::Compact => 0.7,
    };
    media_duration_seconds.max(0.1) * realtime_factor
}

pub fn estimate_render(
    profile: RenderProfile,
    encoder: &EncoderKind,
    media_duration_seconds: f64,
    target_pixels: u64,
    caps: &EncoderCapabilities,
    history: &RenderHistory,
) -> RenderEstimate {
    let successful = history
        .samples
        .iter()
        .filter(|sample| {
            sample.successful
                && sample.profile == profile
                && &sample.encoder == encoder
                && sample.media_duration_seconds.is_finite()
                && sample.media_duration_seconds > 0.0
                && sample.target_pixels > 0
                && sample.elapsed_ms > 0
        })
        .collect::<Vec<_>>();

    if successful.len() >= 3 {
        let predictions = successful
            .iter()
            .map(|sample| {
                (sample.elapsed_ms as f64 / 1000.0)
                    * (media_duration_seconds.max(0.1) / sample.media_duration_seconds)
                    * (target_pixels.max(1) as f64 / sample.target_pixels as f64)
            })
            .collect::<Vec<_>>();

        let weight_sum = (1..=predictions.len()).sum::<usize>() as f64;
        let center = predictions
            .iter()
            .enumerate()
            .map(|(index, value)| *value * (index + 1) as f64)
            .sum::<f64>()
            / weight_sum;
        let variance = predictions
            .iter()
            .map(|value| (value - center).powi(2))
            .sum::<f64>()
            / predictions.len() as f64;
        let spread = variance.sqrt() / center.max(0.001);
        let fallback_rate = successful
            .iter()
            .map(|sample| f64::from(sample.fallback_count))
            .sum::<f64>()
            / successful.len() as f64;
        let fallback_overhead_ratio = successful
            .iter()
            .map(|sample| sample.fallback_overhead_ms as f64 / sample.elapsed_ms.max(1) as f64)
            .sum::<f64>()
            / successful.len() as f64;
        let margin = (0.15
            + spread.min(0.35)
            + (fallback_rate * 0.05).min(0.20)
            + (fallback_overhead_ratio * 0.10).min(0.10))
        .clamp(0.15, 0.55);
        let min_seconds = (center * (1.0 - margin)).max(1.0).floor() as u64;
        let max_seconds = (center * (1.0 + margin))
            .max(min_seconds as f64 + 1.0)
            .ceil() as u64;
        return RenderEstimate {
            min_seconds,
            max_seconds,
            basis: EstimateBasis::History,
            sample_count: successful.len(),
        };
    }

    let center = initial_seconds(
        profile,
        encoder,
        media_duration_seconds,
        target_pixels,
        caps,
    )
    .max(1.0);
    RenderEstimate {
        min_seconds: (center * 0.65).max(1.0).floor() as u64,
        max_seconds: (center * 1.8).max(2.0).ceil() as u64,
        basis: EstimateBasis::Initial,
        sample_count: successful.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(successful: bool, elapsed_ms: u64, fallback_count: u32) -> RenderHistorySample {
        RenderHistorySample {
            id: format!("{elapsed_ms}-{fallback_count}"),
            profile: RenderProfile::Fast,
            encoder: EncoderKind::VulkanH264,
            media_duration_seconds: 60.0,
            target_pixels: 1920 * 1080,
            elapsed_ms,
            successful,
            fallback_count,
            fallback_overhead_ms: 0,
            created_at_ms: u128::from(elapsed_ms),
        }
    }

    #[test]
    fn history_is_bounded_to_recent_samples() {
        let mut history = RenderHistory::default();
        for index in 0..(MAX_RENDER_HISTORY + 5) {
            push_sample(&mut history, sample(true, index as u64 + 1, 0));
        }
        assert_eq!(history.samples.len(), MAX_RENDER_HISTORY);
        assert_eq!(history.samples.first().unwrap().elapsed_ms, 6);
    }

    #[test]
    fn sparse_history_uses_initial_estimate() {
        let caps = EncoderCapabilities {
            h264_benchmarks_ms: [("vulkan_h264".to_string(), 2_000)].into_iter().collect(),
            ..Default::default()
        };
        let history = RenderHistory {
            samples: vec![sample(true, 12_000, 0), sample(true, 11_000, 0)],
        };
        let estimate = estimate_render(
            RenderProfile::Fast,
            &EncoderKind::VulkanH264,
            60.0,
            1920 * 1080,
            &caps,
            &history,
        );
        assert_eq!(estimate.basis, EstimateBasis::Initial);
        assert_eq!(estimate.sample_count, 2);
        assert!(estimate.max_seconds > estimate.min_seconds);
    }

    #[test]
    fn successful_history_ignores_failures_and_fallbacks_widen_range() {
        let caps = EncoderCapabilities::default();
        let history = RenderHistory {
            samples: vec![
                sample(true, 10_000, 0),
                sample(false, 90_000, 0),
                sample(true, 11_000, 1),
                sample(true, 12_000, 2),
            ],
        };
        let estimate = estimate_render(
            RenderProfile::Fast,
            &EncoderKind::VulkanH264,
            60.0,
            1920 * 1080,
            &caps,
            &history,
        );
        assert_eq!(estimate.basis, EstimateBasis::History);
        assert_eq!(estimate.sample_count, 3);
        assert!(estimate.min_seconds >= 7);
        assert!(estimate.max_seconds <= 20);
        assert!(estimate.max_seconds - estimate.min_seconds >= 4);
    }
}
