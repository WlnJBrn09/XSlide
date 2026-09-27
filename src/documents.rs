//! Local filesystem presentation store.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideElement {
    pub id: String,
    /// "title" | "subtitle" | "text" | "image" | "video" | "audio"
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<Vec<Vec<String>>>,
    /// data: URL or /api/files/raw URL for media
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<String>, // "super" | "sub" | null
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slide {
    pub id: String,
    #[serde(default)]
    pub elements: Vec<SlideElement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub starred: bool,
    #[serde(default)]
    pub slides: Vec<Slide>,
    #[serde(default)]
    pub active_slide: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: String,
    pub title: String,
    pub starred: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_format: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveDocument {
    pub title: Option<String>,
    pub slides: Option<Vec<Slide>>,
    pub active_slide: Option<usize>,
    pub starred: Option<bool>,
    pub source_path: Option<String>,
    pub source_format: Option<String>,
}

#[derive(Debug)]
pub enum StoreError {
    NotFound,
    InvalidId,
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "not found"),
            Self::InvalidId => write!(f, "invalid id"),
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Json(e) => write!(f, "json: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

pub struct DocumentStore {
    root: PathBuf,
    lock: Mutex<()>,
}

impl DocumentStore {
    pub fn open(root: PathBuf) -> Result<Self, StoreError> {
        fs::create_dir_all(&root).map_err(StoreError::Io)?;
        Ok(Self {
            root,
            lock: Mutex::new(()),
        })
    }

    fn path_for(&self, id: &str) -> Result<PathBuf, StoreError> {
        validate_id(id)?;
        Ok(self.root.join(format!("{id}.json")))
    }

    pub fn list(&self) -> Result<Vec<DocumentMeta>, StoreError> {
        let _g = self.lock.lock().ok();
        let mut out = Vec::new();
        let entries = fs::read_dir(&self.root).map_err(StoreError::Io)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match read_doc(&path) {
                Ok(doc) => out.push(DocumentMeta {
                    id: doc.id,
                    title: doc.title,
                    starred: doc.starred,
                    created_at: doc.created_at,
                    updated_at: doc.updated_at,
                    source_path: doc.source_path,
                    source_format: doc.source_format,
                }),
                Err(e) => tracing::warn!(path = %path.display(), error = %e, "skip bad document"),
            }
        }
        out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(out)
    }

    pub fn get(&self, id: &str) -> Result<Document, StoreError> {
        let _g = self.lock.lock().ok();
        let path = self.path_for(id)?;
        if !path.exists() {
            return Err(StoreError::NotFound);
        }
        read_doc(&path)
    }

    pub fn create(&self, body: SaveDocument) -> Result<Document, StoreError> {
        let _g = self.lock.lock().ok();
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        let slides = body
            .slides
            .map(normalize_slides)
            .unwrap_or_else(|| vec![default_title_slide()]);
        let active_slide = body
            .active_slide
            .unwrap_or(0)
            .min(slides.len().saturating_sub(1));
        let doc = Document {
            id: id.clone(),
            title: sanitize_title(body.title.unwrap_or_else(|| "Untitled presentation".into())),
            starred: body.starred.unwrap_or(false),
            slides,
            active_slide,
            created_at: now,
            updated_at: now,
            source_path: body.source_path,
            source_format: body.source_format,
        };
        write_doc(&self.path_for(&id)?, &doc)?;
        Ok(doc)
    }

    pub fn update(&self, id: &str, body: SaveDocument) -> Result<Document, StoreError> {
        let _g = self.lock.lock().ok();
        let path = self.path_for(id)?;
        if !path.exists() {
            return Err(StoreError::NotFound);
        }
        let mut doc = read_doc(&path)?;
        if let Some(title) = body.title {
            doc.title = sanitize_title(title);
        }
        if let Some(slides) = body.slides {
            doc.slides = normalize_slides(slides);
        }
        if let Some(active) = body.active_slide {
            doc.active_slide = active.min(doc.slides.len().saturating_sub(1));
        }
        if let Some(starred) = body.starred {
            doc.starred = starred;
        }
        if body.source_path.is_some() {
            doc.source_path = body.source_path;
        }
        if body.source_format.is_some() {
            doc.source_format = body.source_format;
        }
        doc.updated_at = Utc::now();
        write_doc(&path, &doc)?;
        Ok(doc)
    }

    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        let _g = self.lock.lock().ok();
        let path = self.path_for(id)?;
        if !path.exists() {
            return Err(StoreError::NotFound);
        }
        fs::remove_file(path).map_err(StoreError::Io)
    }
}

pub fn default_title_slide() -> Slide {
    Slide {
        id: Uuid::new_v4().to_string(),
        elements: vec![
            SlideElement {
                id: Uuid::new_v4().to_string(),
                kind: "title".into(),
                x: 8.0,
                y: 28.0,
                w: 84.0,
                h: 28.0,
                text: String::new(),
                table: None,
                src: None,
                mime: None,
                font_size: Some(44.0),
                font_family: Some("Inter".into()),
                align: Some("center".into()),
                color: None,
                bold: None,
                italic: None,
                underline: None,
                strikethrough: None,
                highlight: None,
                vertical_align: None,
            },
            SlideElement {
                id: Uuid::new_v4().to_string(),
                kind: "subtitle".into(),
                x: 12.0,
                y: 58.0,
                w: 76.0,
                h: 14.0,
                text: String::new(),
                table: None,
                src: None,
                mime: None,
                font_size: Some(22.0),
                font_family: Some("Inter".into()),
                align: Some("center".into()),
                color: None,
                bold: None,
                italic: None,
                underline: None,
                strikethrough: None,
                highlight: None,
                vertical_align: None,
            },
        ],
        background: Some("#ffffff".into()),
    }
}

fn normalize_slides(slides: Vec<Slide>) -> Vec<Slide> {
    if slides.is_empty() {
        return vec![default_title_slide()];
    }
    slides
        .into_iter()
        .map(|mut s| {
            if s.id.trim().is_empty() {
                s.id = Uuid::new_v4().to_string();
            }
            for el in &mut s.elements {
                if el.id.trim().is_empty() {
                    el.id = Uuid::new_v4().to_string();
                }
            }
            s
        })
        .collect()
}

fn validate_id(id: &str) -> Result<(), StoreError> {
    if id.is_empty() || id.len() > 64 {
        return Err(StoreError::InvalidId);
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(StoreError::InvalidId);
    }
    Ok(())
}

fn sanitize_title(title: String) -> String {
    let t = title.trim();
    if t.is_empty() {
        "Untitled presentation".into()
    } else {
        t.chars().take(200).collect()
    }
}

fn read_doc(path: &Path) -> Result<Document, StoreError> {
    let raw = fs::read_to_string(path).map_err(StoreError::Io)?;
    serde_json::from_str(&raw).map_err(StoreError::Json)
}

fn write_doc(path: &Path, doc: &Document) -> Result<(), StoreError> {
    let raw = serde_json::to_string_pretty(doc).map_err(StoreError::Json)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, raw.as_bytes()).map_err(StoreError::Io)?;
    fs::rename(&tmp, path).map_err(StoreError::Io)
}
