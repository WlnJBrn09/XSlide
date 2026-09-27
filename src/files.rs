//! User Documents folder listing + open/import for pdf, png, pptx (and presentation json).

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zip::ZipArchive;

use crate::documents::{Slide, SlideElement};

/// Extensions shown in the Documents sidebar.
const LIST_EXT: &[&str] = &[
    "pdf", "png", "pptx", "json", "cog", "jpg", "jpeg", "webp", "gif", "svg",
];
/// Also openable via Open/Import.
const OPEN_EXT: &[&str] = &[
    "pdf", "png", "pptx", "json", "cog", "jpg", "jpeg", "webp", "gif", "svg",
];

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub ext: String,
    pub size: u64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenedFile {
    pub name: String,
    pub path: String,
    pub ext: String,
    pub title: String,
    pub format: String,
    /// Presentation slides (pptx import or empty for pure media).
    #[serde(default)]
    pub slides: Vec<Slide>,
    #[serde(default)]
    pub active_slide: usize,
    /// When true, frontend should open `view_url` / binary instead of editable body.
    #[serde(default)]
    pub binary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenPathBody {
    pub path: String,
}

#[derive(Debug)]
pub enum FileError {
    NotFound,
    InvalidPath,
    Unsupported,
    Io(std::io::Error),
    Other(String),
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "file not found"),
            Self::InvalidPath => write!(f, "invalid path"),
            Self::Unsupported => write!(f, "unsupported file type"),
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

pub fn resolve_documents_dir() -> PathBuf {
    if let Ok(p) = std::env::var("XSLIDE_DOCS_DIR") {
        return PathBuf::from(p);
    }
    dirs::document_dir().unwrap_or_else(|| {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join("Documents")
    })
}

pub fn list_documents_folder(root: &Path) -> Result<Vec<FileEntry>, FileError> {
    if !root.exists() {
        fs::create_dir_all(root).map_err(FileError::Io)?;
    }
    let mut out = Vec::new();
    collect_files(root, root, 0, &mut out)?;
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

fn collect_files(
    root: &Path,
    dir: &Path,
    depth: u8,
    out: &mut Vec<FileEntry>,
) -> Result<(), FileError> {
    if depth > 3 {
        return Ok(());
    }
    let entries = fs::read_dir(dir).map_err(FileError::Io)?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, depth + 1, out)?;
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !LIST_EXT.contains(&ext.as_str()) {
            continue;
        }
        let meta = entry.metadata().map_err(FileError::Io)?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push(FileEntry {
            name,
            path: rel,
            ext: ext.clone(),
            size: meta.len(),
            kind: kind_for_ext(&ext).into(),
        });
    }
    Ok(())
}

fn kind_for_ext(ext: &str) -> &'static str {
    match ext {
        "pdf" => "pdf",
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" => "image",
        "pptx" => "presentation",
        "json" | "cog" => "xslide",
        _ => "file",
    }
}

/// Resolve a relative path under Documents; reject path traversal.
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, FileError> {
    let rel = rel.trim().trim_start_matches(['/', '\\']);
    if rel.is_empty() {
        return Err(FileError::InvalidPath);
    }
    let candidate = root.join(rel);
    let canon_root = fs::canonicalize(root).map_err(FileError::Io)?;
    let full = if candidate.exists() {
        fs::canonicalize(&candidate).map_err(FileError::Io)?
    } else {
        let mut norm = PathBuf::new();
        for c in Path::new(rel).components() {
            match c {
                Component::Normal(s) => norm.push(s),
                Component::CurDir => {}
                _ => return Err(FileError::InvalidPath),
            }
        }
        if norm.as_os_str().is_empty() {
            return Err(FileError::InvalidPath);
        }
        return Ok(root.join(norm));
    };
    if !full.starts_with(&canon_root) {
        return Err(FileError::InvalidPath);
    }
    Ok(full)
}

pub fn open_file(root: &Path, rel: &str) -> Result<OpenedFile, FileError> {
    let path = safe_join(root, rel)?;
    if !path.is_file() {
        return Err(FileError::NotFound);
    }
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled presentation".into());

    let bytes = fs::read(&path).map_err(FileError::Io)?;
    open_bytes_with_options(&name, rel, &ext, &title, &bytes, true)
}

pub fn open_bytes(
    name: &str,
    rel: &str,
    ext: &str,
    title: &str,
    bytes: &[u8],
) -> Result<OpenedFile, FileError> {
    open_bytes_with_options(name, rel, ext, title, bytes, false)
}

pub fn open_bytes_with_options(
    name: &str,
    rel: &str,
    ext: &str,
    title: &str,
    bytes: &[u8],
    from_disk: bool,
) -> Result<OpenedFile, FileError> {
    if !OPEN_EXT.contains(&ext) {
        return Err(FileError::Unsupported);
    }
    match ext {
        "pdf" => binary_open(
            name,
            rel,
            ext,
            title,
            bytes,
            from_disk,
            "application/pdf",
            "pdf",
        ),
        "png" => binary_open(
            name,
            rel,
            ext,
            title,
            bytes,
            from_disk,
            "image/png",
            "image",
        ),
        "jpg" | "jpeg" => binary_open(
            name,
            rel,
            ext,
            title,
            bytes,
            from_disk,
            "image/jpeg",
            "image",
        ),
        "webp" => binary_open(
            name,
            rel,
            ext,
            title,
            bytes,
            from_disk,
            "image/webp",
            "image",
        ),
        "gif" => binary_open(
            name,
            rel,
            ext,
            title,
            bytes,
            from_disk,
            "image/gif",
            "image",
        ),
        "svg" => binary_open(
            name,
            rel,
            ext,
            title,
            bytes,
            from_disk,
            "image/svg+xml",
            "image",
        ),
        "pptx" => open_pptx(name, rel, title, bytes, from_disk),
        "json" | "cog" => open_presentation_json(name, rel, ext, title, bytes),
        _ => Err(FileError::Unsupported),
    }
}

fn binary_open(
    name: &str,
    rel: &str,
    ext: &str,
    title: &str,
    bytes: &[u8],
    from_disk: bool,
    mime: &str,
    format: &str,
) -> Result<OpenedFile, FileError> {
    let valid = match ext {
        "pdf" => bytes.starts_with(b"%PDF-"),
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        "svg" => std::str::from_utf8(bytes)
            .map(|s| s.contains("<svg"))
            .unwrap_or(false),
        _ => true,
    };
    if !valid {
        return Err(FileError::Other(format!("invalid {ext} file")));
    }
    let view_url = if from_disk && !rel.is_empty() {
        Some(format!("/api/files/raw?path={}", urlencoding_encode(rel)))
    } else {
        None
    };
    let binary_base64 = if view_url.is_none() {
        use base64::Engine;
        Some(base64::engine::general_purpose::STANDARD.encode(bytes))
    } else {
        None
    };
    // Single slide placeholder so the app has a canvas; media shown in viewer overlay.
    let slides = vec![media_cover_slide(title, format)];
    Ok(OpenedFile {
        name: name.into(),
        path: rel.into(),
        ext: ext.into(),
        title: title.into(),
        format: format.into(),
        slides,
        active_slide: 0,
        binary: true,
        view_url,
        binary_base64,
        mime: Some(mime.into()),
    })
}

fn el(
    kind: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    text: String,
    font_size: f64,
    align: &str,
    bold: Option<bool>,
) -> SlideElement {
    SlideElement {
        id: Uuid::new_v4().to_string(),
        kind: kind.into(),
        x,
        y,
        w,
        h,
        text,
        table: None,
        src: None,
        mime: None,
        font_size: Some(font_size),
        font_family: Some("Inter".into()),
        align: Some(align.into()),
        color: None,
        bold,
        italic: None,
        underline: None,
        strikethrough: None,
        highlight: None,
        vertical_align: None,
    }
}

fn media_cover_slide(title: &str, kind: &str) -> Slide {
    Slide {
        id: Uuid::new_v4().to_string(),
        background: Some("#ffffff".into()),
        elements: vec![
            el(
                "title",
                8.0,
                36.0,
                84.0,
                18.0,
                title.into(),
                32.0,
                "center",
                Some(true),
            ),
            el(
                "subtitle",
                12.0,
                56.0,
                76.0,
                10.0,
                format!("{kind} file"),
                16.0,
                "center",
                None,
            ),
        ],
    }
}

fn open_pptx(
    name: &str,
    rel: &str,
    title: &str,
    bytes: &[u8],
    from_disk: bool,
) -> Result<OpenedFile, FileError> {
    let slides = parse_pptx_slides(bytes)?;
    let view_url = if from_disk && !rel.is_empty() {
        Some(format!("/api/files/raw?path={}", urlencoding_encode(rel)))
    } else {
        None
    };
    let binary_base64 = if view_url.is_none() {
        use base64::Engine;
        Some(base64::engine::general_purpose::STANDARD.encode(bytes))
    } else {
        None
    };
    Ok(OpenedFile {
        name: name.into(),
        path: rel.into(),
        ext: "pptx".into(),
        title: title.into(),
        format: "presentation".into(),
        slides,
        active_slide: 0,
        binary: true,
        view_url,
        binary_base64,
        mime: Some(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation".into(),
        ),
    })
}

/// Import editable text and images at their original slide coordinates.
fn parse_pptx_slides(bytes: &[u8]) -> Result<Vec<Slide>, FileError> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| FileError::Other(e.to_string()))?;

    let mut slide_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| FileError::Other(e.to_string()))?;
        let name = file.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") && !name.contains("_rels")
        {
            slide_names.push(name);
        }
    }
    slide_names.sort_by_key(|a| extract_slide_num(a));
    // The numeric file names do not determine presentation order. A reordered
    // deck keeps the old names and changes only the relationships in the list.
    if let (Ok(pres), Ok(rels)) = (
        read_zip_text(&mut archive, "ppt/presentation.xml"),
        read_zip_text(&mut archive, "ppt/_rels/presentation.xml.rels"),
    ) {
        let targets: std::collections::HashMap<String, String> = tag_blocks(&rels, "Relationship")
            .into_iter()
            .filter_map(|tag| Some((tag_attr(tag, "Id")?, tag_attr(tag, "Target")?)))
            .collect();
        let ordered: Vec<String> = tag_blocks(&pres, "p:sldId")
            .into_iter()
            .filter_map(|tag| targets.get(&tag_attr(tag, "r:id")?).cloned())
            .map(|target| {
                if target.starts_with('/') {
                    target.trim_start_matches('/').to_string()
                } else {
                    format!("ppt/{target}")
                }
            })
            .filter(|path| slide_names.contains(path))
            .collect();
        if ordered.len() == slide_names.len() {
            slide_names = ordered;
        }
    }

    if slide_names.is_empty() {
        return Ok(vec![crate::documents::default_title_slide()]);
    }

    let mut slides = Vec::new();
    for path in slide_names {
        let xml = read_zip_text(&mut archive, &path)?;
        let rel_path = path.replacen("ppt/slides/", "ppt/slides/_rels/", 1) + ".rels";
        let image_targets: std::collections::HashMap<String, String> =
            read_zip_text(&mut archive, &rel_path)
                .ok()
                .map(|rels| {
                    tag_blocks(&rels, "Relationship")
                        .into_iter()
                        .filter_map(|tag| Some((tag_attr(tag, "Id")?, tag_attr(tag, "Target")?)))
                        .collect()
                })
                .unwrap_or_default();
        let mut elements = Vec::new();
        let mut nodes = tag_blocks(&xml, "p:sp")
            .into_iter()
            .map(|block| {
                (
                    block.as_ptr() as usize - xml.as_ptr() as usize,
                    false,
                    block,
                )
            })
            .chain(
                tag_blocks(&xml, "p:pic")
                    .into_iter()
                    .map(|block| (block.as_ptr() as usize - xml.as_ptr() as usize, true, block)),
            )
            .collect::<Vec<_>>();
        nodes.sort_by_key(|(offset, _, _)| *offset);
        for (_, is_picture, shape) in nodes {
            if is_picture {
                let pic = shape;
                let Some(rid) = tag_blocks(pic, "a:blip")
                    .first()
                    .and_then(|b| tag_attr(b, "r:embed"))
                else {
                    continue;
                };
                let Some(target) = image_targets.get(&rid) else {
                    continue;
                };
                let Some(filename) = target.rsplit('/').next() else {
                    continue;
                };
                let media_path = format!("ppt/media/{filename}");
                let Ok(mut media) = archive.by_name(&media_path) else {
                    continue;
                };
                let ext = filename
                    .rsplit('.')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let mime = match ext.as_str() {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "gif" => "image/gif",
                    _ => continue,
                };
                let mut data = Vec::new();
                media
                    .read_to_end(&mut data)
                    .map_err(|e| FileError::Other(e.to_string()))?;
                use base64::Engine;
                let (x, y, w, h) = shape_geometry(pic, elements.len());
                let mut item = el("image", x, y, w, h, String::new(), 14.0, "left", None);
                item.src = Some(format!(
                    "data:{mime};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(data)
                ));
                item.mime = Some(mime.into());
                elements.push(item);
                continue;
            }
            let text = tag_blocks(shape, "a:p")
                .into_iter()
                .map(|p| extract_a_t_texts(p).join(""))
                .collect::<Vec<_>>()
                .join("\n");
            if text.trim().is_empty() {
                continue;
            }
            let (x, y, w, h) = shape_geometry(shape, elements.len());
            let font_size = tag_blocks(shape, "a:rPr")
                .first()
                .and_then(|r| tag_attr(r, "sz"))
                .and_then(|v| v.parse::<f64>().ok())
                .map(|v| v / 100.0)
                .unwrap_or(18.0);
            let align = tag_blocks(shape, "a:pPr")
                .first()
                .and_then(|p| tag_attr(p, "algn"))
                .map(|s| match s.as_str() {
                    "ctr" => "center",
                    "r" => "right",
                    "just" => "justify",
                    _ => "left",
                })
                .unwrap_or("left");
            let kind = if elements.is_empty() { "title" } else { "text" };
            let mut item = el(kind, x, y, w, h, text, font_size, align, None);
            if let Some(run) = tag_blocks(shape, "a:rPr").first() {
                item.bold = tag_attr(run, "b").map(|s| s == "1");
                item.italic = tag_attr(run, "i").map(|s| s == "1");
                item.underline = tag_attr(run, "u").map(|s| s == "sng");
                item.strikethrough = tag_attr(run, "strike").map(|s| s == "sngStrike");
                item.vertical_align = tag_attr(run, "baseline")
                    .and_then(|s| s.parse::<i32>().ok())
                    .and_then(|n| {
                        if n > 0 {
                            Some("super".into())
                        } else if n < 0 {
                            Some("sub".into())
                        } else {
                            None
                        }
                    });
                item.color = tag_blocks(run, "a:solidFill")
                    .first()
                    .and_then(|fill| {
                        tag_blocks(fill, "a:srgbClr")
                            .first()
                            .and_then(|c| tag_attr(c, "val"))
                    })
                    .map(|s| format!("#{s}"));
                item.highlight = tag_blocks(run, "a:highlight")
                    .first()
                    .and_then(|h| {
                        tag_blocks(h, "a:srgbClr")
                            .first()
                            .and_then(|c| tag_attr(c, "val"))
                    })
                    .map(|s| format!("#{s}"));
                if let Some(font) = tag_blocks(run, "a:latin")
                    .first()
                    .and_then(|f| tag_attr(f, "typeface"))
                {
                    item.font_family = Some(font);
                }
            }
            elements.push(item);
        }
        if elements.is_empty() {
            elements.push(el(
                "title",
                8.0,
                40.0,
                84.0,
                16.0,
                String::new(),
                28.0,
                "center",
                None,
            ));
        }
        let background = tag_blocks(&xml, "p:bg")
            .first()
            .and_then(|bg| {
                tag_blocks(bg, "a:srgbClr")
                    .first()
                    .and_then(|c| tag_attr(c, "val"))
            })
            .map(|s| format!("#{s}"))
            .or_else(|| Some("#ffffff".into()));
        slides.push(Slide {
            id: Uuid::new_v4().to_string(),
            elements,
            background,
        });
    }
    Ok(slides)
}

fn read_zip_text(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String, FileError> {
    let mut file = archive
        .by_name(name)
        .map_err(|e| FileError::Other(e.to_string()))?;
    let mut xml = String::new();
    file.read_to_string(&mut xml)
        .map_err(|e| FileError::Other(e.to_string()))?;
    Ok(xml)
}

fn tag_blocks<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let needle = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut rest = xml;
    while let Some(start) = rest.find(&needle) {
        let tag_start = &rest[start..];
        let next = tag_start.as_bytes().get(needle.len()).copied();
        if !matches!(
            next,
            Some(b'>') | Some(b'/') | Some(b' ') | Some(b'\t') | Some(b'\n')
        ) {
            rest = &tag_start[needle.len()..];
            continue;
        }
        let Some(open_end) = tag_start.find('>') else {
            break;
        };
        if tag_start[..=open_end].ends_with("/>") {
            out.push(&tag_start[..=open_end]);
            rest = &tag_start[open_end + 1..];
        } else if let Some(end) = tag_start[open_end + 1..].find(&close) {
            let full_end = open_end + 1 + end + close.len();
            out.push(&tag_start[..full_end]);
            rest = &tag_start[full_end..];
        } else {
            break;
        }
    }
    out
}

fn tag_attr(tag: &str, key: &str) -> Option<String> {
    let head = tag.split_once('>')?.0;
    let needle = format!("{key}=\"");
    // Require whitespace before the name, so `id` never reads `r:id`.
    let pos = head.find(&format!(" {needle}"))? + 1 + needle.len();
    let end = head[pos..].find('"')?;
    Some(decode_xml_entities(&head[pos..pos + end]))
}

fn shape_geometry(shape: &str, index: usize) -> (f64, f64, f64, f64) {
    let fallback = (8.0, 15.0 + (index as f64 * 12.0), 84.0, 12.0);
    let Some(xfrm) = tag_blocks(shape, "a:xfrm").first().copied() else {
        return fallback;
    };
    let Some(off) = tag_blocks(xfrm, "a:off").first().copied() else {
        return fallback;
    };
    let Some(ext) = tag_blocks(xfrm, "a:ext").first().copied() else {
        return fallback;
    };
    let value = |tag: &str, key: &str, scale: f64| {
        tag_attr(tag, key)
            .and_then(|s| s.parse::<f64>().ok())
            .map(|v| (v / scale * 100.0).clamp(0.0, 100.0))
    };
    (
        value(off, "x", 12_192_000.0).unwrap_or(fallback.0),
        value(off, "y", 6_858_000.0).unwrap_or(fallback.1),
        value(ext, "cx", 12_192_000.0).unwrap_or(fallback.2),
        value(ext, "cy", 6_858_000.0).unwrap_or(fallback.3),
    )
}

fn extract_slide_num(name: &str) -> u32 {
    name.trim_start_matches("ppt/slides/slide")
        .trim_end_matches(".xml")
        .parse()
        .unwrap_or(0)
}

fn extract_a_t_texts(xml: &str) -> Vec<String> {
    tag_blocks(xml, "a:t")
        .into_iter()
        .filter_map(|block| {
            block
                .split_once('>')
                .and_then(|(_, tail)| tail.split_once("</a:t>"))
                .map(|(text, _)| decode_xml_entities(text))
        })
        .collect()
}

fn decode_xml_entities(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('&') {
            if let Some(end) = after.find(';').filter(|n| *n <= 12) {
                let entity = &after[..end];
                let decoded = match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16)
                        .ok()
                        .and_then(char::from_u32),
                    e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
                    _ => None,
                };
                if let Some(c) = decoded {
                    out.push(c);
                    rest = &after[end + 1..];
                    continue;
                }
            }
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn open_presentation_json(
    name: &str,
    rel: &str,
    ext: &str,
    title: &str,
    bytes: &[u8],
) -> Result<OpenedFile, FileError> {
    #[derive(Deserialize)]
    struct Doc {
        title: Option<String>,
        slides: Option<Vec<Slide>>,
        active_slide: Option<usize>,
    }
    let doc: Doc = serde_json::from_slice(bytes)
        .map_err(|e| FileError::Other(format!("invalid presentation json: {e}")))?;
    let slides = doc
        .slides
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| vec![crate::documents::default_title_slide()]);
    let active_slide = doc
        .active_slide
        .unwrap_or(0)
        .min(slides.len().saturating_sub(1));
    Ok(OpenedFile {
        name: name.into(),
        path: rel.into(),
        ext: ext.into(),
        title: doc.title.unwrap_or_else(|| title.into()),
        format: "xslide".into(),
        slides,
        active_slide,
        binary: false,
        view_url: None,
        binary_base64: None,
        mime: Some("application/json".into()),
    })
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn mime_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "json" | "cog" => "application/json",
        _ => "application/octet-stream",
    }
}

pub fn read_raw_file(root: &Path, rel: &str) -> Result<(Vec<u8>, &'static str), FileError> {
    let path = safe_join(root, rel)?;
    if !path.is_file() {
        return Err(FileError::NotFound);
    }
    let data = fs::read(&path).map_err(FileError::Io)?;
    Ok((data, mime_for_path(&path)))
}
