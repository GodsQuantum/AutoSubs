use crate::domain::{FitMode, FormatKey, FormatProfile};

const MIN_CUSTOM_DIMENSION: u32 = 16;
const MAX_CUSTOM_DIMENSION: u32 = 16_384;

pub fn normalize_format_profile(profile: &mut FormatProfile) -> Result<(), String> {
    match profile.key {
        FormatKey::Source => {
            profile.fit = FitMode::Preserve;
            profile.width = None;
            profile.height = None;
        }
        FormatKey::Custom => {
            let (Some(width), Some(height)) = (profile.width, profile.height) else {
                return Err("custom format requires width and height".into());
            };
            if !(MIN_CUSTOM_DIMENSION..=MAX_CUSTOM_DIMENSION).contains(&width)
                || !(MIN_CUSTOM_DIMENSION..=MAX_CUSTOM_DIMENSION).contains(&height)
            {
                return Err(format!(
                    "custom width and height must be between {MIN_CUSTOM_DIMENSION} and {MAX_CUSTOM_DIMENSION}"
                ));
            }
            if width % 2 != 0 || height % 2 != 0 {
                return Err(
                    "custom width and height must be even for yuv420p/H.264 compatibility".into(),
                );
            }
            if profile.fit == FitMode::Preserve {
                profile.fit = FitMode::Cover;
            }
        }
        _ => {
            profile.width = None;
            profile.height = None;
            if profile.fit == FitMode::Preserve {
                profile.fit = FitMode::Cover;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(
        key: FormatKey,
        fit: FitMode,
        width: Option<u32>,
        height: Option<u32>,
    ) -> FormatProfile {
        FormatProfile {
            key,
            fit,
            width,
            height,
        }
    }

    #[test]
    fn source_always_normalizes_to_preserve_and_drops_custom_dimensions() {
        let mut value = profile(FormatKey::Source, FitMode::Cover, Some(1080), Some(1920));
        normalize_format_profile(&mut value).unwrap();
        assert_eq!(value.fit, FitMode::Preserve);
        assert_eq!((value.width, value.height), (None, None));
    }

    #[test]
    fn explicit_canvas_never_keeps_preserve_fit() {
        let mut value = profile(FormatKey::Portrait916, FitMode::Preserve, None, None);
        normalize_format_profile(&mut value).unwrap();
        assert_eq!(value.fit, FitMode::Cover);
    }

    #[test]
    fn canonical_canvas_drops_stale_custom_dimensions() {
        let mut value = profile(FormatKey::Square11, FitMode::Contain, Some(900), Some(1600));
        normalize_format_profile(&mut value).unwrap();
        assert_eq!((value.width, value.height), (None, None));
    }

    #[test]
    fn custom_canvas_requires_sane_even_dimensions() {
        for (width, height) in [
            (0, 1080),
            (15, 1080),
            (1081, 1920),
            (1080, 1921),
            (20_000, 1080),
        ] {
            let mut value = profile(FormatKey::Custom, FitMode::Cover, Some(width), Some(height));
            assert!(
                normalize_format_profile(&mut value).is_err(),
                "{width}x{height} should be rejected"
            );
        }

        let mut valid = profile(FormatKey::Custom, FitMode::Preserve, Some(1080), Some(1920));
        normalize_format_profile(&mut valid).unwrap();
        assert_eq!(valid.fit, FitMode::Cover);
    }
}
