use anyhow::{Result, bail};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFace {
    pub id: String,
    pub family: String,
    pub style: String,
    pub weight: u16,
    pub italic: bool,
    pub file_name: String,
}

pub fn scan_fonts(root: &Path) -> Result<Vec<FontFace>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    scan_files(root, |path| {
        metadata(path).or_else(|| fallback_metadata(path))
    })
}

pub fn resolve_font_content(root: &Path, id: &str) -> Result<PathBuf> {
    let relative = decode_id(id)?;
    let root = fs::canonicalize(root)?;
    let path = root.join(relative);
    let canonical = fs::canonicalize(&path)?;
    if !canonical.starts_with(&root) || !canonical.is_file() || !is_font_file(&canonical) {
        bail!("font is outside the configured fonts directory")
    }
    Ok(canonical)
}

pub fn css(fonts: &[FontFace]) -> String {
    fonts
        .iter()
        .map(|font| {
            format!(
                "@font-face{{font-family:'{}';font-style:{};font-weight:{};src:url('/api/v1/fonts/{}/content') format('{}')}}\n",
                css_escape(&font.family),
                if font.italic { "italic" } else { "normal" },
                font.weight,
                font.id,
                match font.file_name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str() {
                    "otf" | "otc" => "opentype",
                    _ => "truetype",
                }
            )
        })
        .collect()
}

fn scan_files<F>(root: &Path, metadata: F) -> Result<Vec<FontFace>>
where
    F: Fn(&Path) -> Option<(String, String, u16, bool)>,
{
    let root = fs::canonicalize(root)?;
    let mut paths = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if is_font_file(&path) {
                paths.push(path);
            }
        }
    }
    paths.sort();
    let mut unique = BTreeMap::new();
    for path in paths {
        let canonical = fs::canonicalize(&path)?;
        if !canonical.starts_with(&root) {
            continue;
        }
        let Some((family, style, weight, italic)) = metadata(&canonical) else {
            continue;
        };
        let family = title_name(&family);
        let style = title_name(&style);
        let key = format!(
            "{}\0{}\0{}",
            family.to_ascii_lowercase(),
            style.to_ascii_lowercase(),
            weight
        );
        unique.entry(key).or_insert_with(|| FontFace {
            id: font_id(canonical.strip_prefix(&root).unwrap()),
            family,
            style,
            weight,
            italic,
            file_name: canonical
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        });
    }
    Ok(unique.into_values().collect())
}

fn metadata(path: &Path) -> Option<(String, String, u16, bool)> {
    let output = Command::new("fc-scan")
        .args(["--format=%{family}\n%{style}\n%{weight}\n", path.to_str()?])
        .output()
        .ok()?;
    parse_fc_scan_output(&String::from_utf8_lossy(&output.stdout))
}

fn fallback_metadata(path: &Path) -> Option<(String, String, u16, bool)> {
    Some((
        path.file_stem()?.to_string_lossy().replace(['_', '-'], " "),
        "Regular".into(),
        400,
        false,
    ))
}

pub fn parse_fc_scan_output(value: &str) -> Option<(String, String, u16, bool)> {
    let mut fields = value.lines().map(str::trim).filter(|v| !v.is_empty());
    let family = fields.next()?.to_owned();
    let style = fields.next().unwrap_or("Regular").to_owned();
    let raw_weight = fields
        .next()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(80);
    let weight = match raw_weight {
        0..=40 => 300,
        41..=80 => 400,
        81..=120 => 500,
        121..=180 => 600,
        181..=200 => 700,
        _ => raw_weight.clamp(100, 900),
    };
    Some((
        family,
        style.clone(),
        weight,
        style.to_ascii_lowercase().contains("italic"),
    ))
}

fn is_font_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|v| v.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("ttf" | "otf" | "ttc" | "otc")
    )
}

fn title_name(value: &str) -> String {
    let mut chars = value.trim().chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn font_id(relative: &Path) -> String {
    relative
        .to_string_lossy()
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn decode_id(id: &str) -> Result<PathBuf> {
    if id.is_empty() || !id.len().is_multiple_of(2) || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("invalid font id")
    }
    let bytes = (0..id.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&id[i..i + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let path = PathBuf::from(String::from_utf8(bytes)?);
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        bail!("invalid font id")
    }
    Ok(path)
}

fn css_escape(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric() || " -_.".contains(*character))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scans_supported_fonts_recursively_and_normalizes_duplicate_metadata() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(root.path().join("nested/one.ttf"), b"font").unwrap();
        fs::write(root.path().join("two.OTF"), b"font").unwrap();
        fs::write(root.path().join("ignored.txt"), b"nope").unwrap();

        let faces = scan_files(root.path(), |path| match path.file_name()?.to_str()? {
            "one.ttf" => Some(("Demo".into(), "Regular".into(), 400, false)),
            "two.OTF" => Some((" demo ".into(), "regular".into(), 400, false)),
            _ => None,
        })
        .unwrap();

        assert_eq!(faces.len(), 1);
        assert_eq!(faces[0].family, "Demo");
        assert_eq!(faces[0].style, "Regular");
    }

    #[test]
    fn font_content_resolution_rejects_traversal() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("safe.ttf"), b"font").unwrap();
        let id = font_id(std::path::Path::new("safe.ttf"));
        assert_eq!(
            resolve_font_content(root.path(), &id).unwrap(),
            root.path().join("safe.ttf")
        );
        assert!(resolve_font_content(root.path(), "2e2e2f6576696c2e747466").is_err());
    }

    #[test]
    fn parses_fc_scan_metadata_without_running_fc_scan() {
        let metadata = parse_fc_scan_output("Demo Sans\nBook\n80\n").unwrap();
        assert_eq!(metadata, ("Demo Sans".into(), "Book".into(), 400, false));
    }

    #[test]
    fn missing_fonts_directory_is_an_empty_catalog() {
        let root = tempfile::tempdir().unwrap().path().join("missing");
        assert!(scan_fonts(&root).unwrap().is_empty());
    }
}
