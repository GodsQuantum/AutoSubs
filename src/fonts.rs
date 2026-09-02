use anyhow::{Result, bail};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFace {
    pub id: String,
    pub family: String,
    pub full_name: String,
    pub style: String,
    pub weight: u16,
    pub italic: bool,
    pub file_name: String,
}

const FONTS_ROOT: &str = "/fonts";

pub fn scan_fonts() -> Result<Vec<FontFace>> {
    scan_fonts_from(Path::new(FONTS_ROOT))
}

fn scan_fonts_from(root: &Path) -> Result<Vec<FontFace>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    scan_files(root, |path| {
        metadata(path).or_else(|| fallback_metadata(path))
    })
}

pub fn resolve_font_content(id: &str) -> Result<PathBuf> {
    resolve_font_content_from(Path::new(FONTS_ROOT), id)
}

fn resolve_font_content_from(root: &Path, id: &str) -> Result<PathBuf> {
    if id.is_empty() || !id.len().is_multiple_of(2) || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("invalid font id")
    }

    let (root, paths) = font_paths(root)?;
    for path in paths {
        let relative = path.strip_prefix(&root)?;
        if font_id(relative) == id {
            return Ok(path);
        }
    }

    bail!("font not found")
}

pub fn css(fonts: &[FontFace]) -> String {
    let mut stylesheet = String::new();
    for font in fonts {
        let source_format = match font
            .file_name
            .rsplit(char::from(46))
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "otf" | "otc" => "opentype",
            _ => "truetype",
        };
        for (index, family) in [&font.family, &font.full_name].into_iter().enumerate() {
            if index == 1 && font.full_name.eq_ignore_ascii_case(&font.family) {
                continue;
            }
            stylesheet.push_str(&format!(
                r#"@font-face{{font-family:"{}";font-style:{};font-weight:{};src:url("/api/v1/fonts/{}/content") format("{}")}}
"#,
                css_escape(family),
                if font.italic { "italic" } else { "normal" },
                font.weight, font.id, source_format
            ));
        }
    }
    stylesheet
}

fn scan_files<F>(root: &Path, metadata: F) -> Result<Vec<FontFace>>
where
    F: Fn(&Path) -> Option<(String, String, String, u16, bool)>,
{
    let (root, paths) = font_paths(root)?;
    let mut faces = Vec::new();

    for canonical in paths {
        let Some((family, full_name, style, weight, italic)) = metadata(&canonical) else {
            continue;
        };
        let family = title_name(&family);
        let full_name = title_name(&full_name);
        let style = title_name(&style);
        let relative = canonical.strip_prefix(&root)?;

        faces.push(FontFace {
            id: font_id(relative),
            family,
            full_name,
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

    Ok(faces)
}

fn font_paths(root: &Path) -> Result<(PathBuf, Vec<PathBuf>)> {
    let root = fs::canonicalize(root)?;
    let mut pending = vec![root.clone()];
    let mut candidates = Vec::new();

    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let path = entry.path();

            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file() && is_font_file(&path) {
                candidates.push(path);
            }
        }
    }

    let mut paths = Vec::new();
    for path in candidates {
        let canonical = fs::canonicalize(path)?;
        if canonical.starts_with(&root) && canonical.is_file() && is_font_file(&canonical) {
            paths.push(canonical);
        }
    }

    paths.sort();
    paths.dedup();

    Ok((root, paths))
}

fn metadata(path: &Path) -> Option<(String, String, String, u16, bool)> {
    let output = Command::new("fc-scan")
        .args([
            r#"--format=%{family}
%{fullname}
%{style}
%{weight}
"#,
            path.to_str()?,
        ])
        .output()
        .ok()?;
    parse_fc_scan_output(&String::from_utf8_lossy(&output.stdout))
}

fn fallback_metadata(path: &Path) -> Option<(String, String, String, u16, bool)> {
    let name = path
        .file_stem()?
        .to_string_lossy()
        .replace([char::from(95), char::from(45)], " ");
    Some((name.clone(), name, "Regular".into(), 400, false))
}

pub fn parse_fc_scan_output(value: &str) -> Option<(String, String, String, u16, bool)> {
    let mut fields = value.lines().map(str::trim);
    let family = fields
        .next()?
        .split(char::from(44))
        .next()?
        .trim()
        .to_owned();
    let full_name = fields
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(&family)
        .split(char::from(44))
        .next()?
        .trim()
        .to_owned();
    let style = fields
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("Regular")
        .split(char::from(44))
        .next()?
        .trim()
        .to_owned();
    let raw_weight = fields
        .next()
        .and_then(|value| value.parse::<u16>().ok())
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
        full_name,
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

fn css_escape(value: &str) -> String {
    value
        .replace(char::from(92), &format!("{0}{0}", char::from(92)))
        .replace(
            char::from(34),
            &format!("{}{}", char::from(92), char::from(34)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scans_every_supported_font_file_even_with_duplicate_metadata() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(root.path().join("nested/one.ttf"), b"font").unwrap();
        fs::write(root.path().join("two.OTF"), b"font").unwrap();
        fs::write(root.path().join("ignored.txt"), b"nope").unwrap();

        let faces = scan_files(root.path(), |path| match path.file_name()?.to_str()? {
            "one.ttf" => Some((
                "Demo".into(),
                "Demo Regular".into(),
                "Regular".into(),
                400,
                false,
            )),
            "two.OTF" => Some((
                " demo ".into(),
                "Demo Regular".into(),
                "regular".into(),
                400,
                false,
            )),
            _ => None,
        })
        .unwrap();

        assert_eq!(faces.len(), 2);
        assert_eq!(faces[0].family, "Demo");
        assert_eq!(faces[0].style, "Regular");
    }

    #[test]
    fn font_content_resolution_rejects_traversal() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("safe.ttf"), b"font").unwrap();
        let id = font_id(std::path::Path::new("safe.ttf"));
        assert_eq!(
            resolve_font_content_from(root.path(), &id).unwrap(),
            root.path().join("safe.ttf")
        );
        assert!(resolve_font_content_from(root.path(), "2e2e2f6576696c2e747466").is_err());
    }

    #[test]
    fn parses_fc_scan_metadata_without_running_fc_scan() {
        let metadata = parse_fc_scan_output("Demo Sans\nDemo Sans Book\nBook\n80\n").unwrap();
        assert_eq!(
            metadata,
            (
                "Demo Sans".into(),
                "Demo Sans Book".into(),
                "Book".into(),
                400,
                false
            )
        );
    }

    #[test]
    fn keeps_exact_family_names_for_catalog_and_css() {
        let metadata =
            parse_fc_scan_output("League Spartan,Alias\nLeague Spartan Regular\nRegular\n80\n")
                .unwrap();
        assert_eq!(metadata.0, "League Spartan");
        let face = FontFace {
            id: "league".into(),
            family: metadata.0,
            full_name: metadata.1,
            style: metadata.2,
            weight: metadata.3,
            italic: metadata.4,
            file_name: "LeagueSpartan.ttf".into(),
        };
        let stylesheet = css(&[face]);
        assert!(stylesheet.contains("font-family:\"League Spartan\""));
        assert!(stylesheet.contains("font-family:\"League Spartan Regular\""));
    }

    #[test]
    fn missing_fonts_directory_is_an_empty_catalog() {
        let root = tempfile::tempdir().unwrap().path().join("missing");
        assert!(scan_fonts_from(&root).unwrap().is_empty());
    }
}
