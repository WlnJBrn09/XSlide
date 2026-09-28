//! Export presentation to editable JSON, text, or PowerPoint.

use serde::Deserialize;
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::documents::{Slide, SlideElement};
use base64::Engine;

#[derive(Debug, Deserialize)]
pub struct ExportBody {
    pub format: String,
    pub title: Option<String>,
    pub slides: Vec<Slide>,
    pub active_slide: Option<usize>,
}

#[derive(Debug)]
pub struct ExportFile {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum ExportError {
    Unsupported,
    Other(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "unsupported export format"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

pub fn export_document(body: &ExportBody) -> Result<ExportFile, ExportError> {
    let title = sanitize_filename(body.title.as_deref().unwrap_or("presentation"));
    match body.format.to_lowercase().as_str() {
        "json" | "cog" => {
            let json = serde_json::to_vec_pretty(&serde_json::json!({
                "title": body.title,
                "slides": body.slides,
                "active_slide": body.active_slide.unwrap_or(0),
            }))
            .map_err(|e| ExportError::Other(e.to_string()))?;
            Ok(ExportFile {
                filename: format!("{title}.json"),
                content_type: "application/json".into(),
                bytes: json,
            })
        }
        "txt" => {
            let mut plain = String::new();
            for (i, slide) in body.slides.iter().enumerate() {
                plain.push_str(&format!("--- Slide {} ---\n", i + 1));
                for el in &slide.elements {
                    if !el.text.trim().is_empty() {
                        plain.push_str(&el.text);
                        plain.push('\n');
                    }
                }
                plain.push('\n');
            }
            Ok(ExportFile {
                filename: format!("{title}.txt"),
                content_type: "text/plain; charset=utf-8".into(),
                bytes: plain.into_bytes(),
            })
        }
        "pptx" => Ok(ExportFile {
            filename: format!("{title}.pptx"),
            content_type:
                "application/vnd.openxmlformats-officedocument.presentationml.presentation".into(),
            bytes: build_pptx(&body.slides).map_err(ExportError::Other)?,
        }),
        _ => Err(ExportError::Unsupported),
    }
}

const SLIDE_W: f64 = 12_192_000.0;
const SLIDE_H: f64 = 6_858_000.0;

const SLIDE_MASTER_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sldMaster xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/><p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst></p:sldMaster>";

const SLIDE_LAYOUT_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sldLayout xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" type=\"blank\" preserve=\"1\"><p:cSld name=\"Blank\"><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>";

const THEME_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"XSlide\"><a:themeElements><a:clrScheme name=\"XSlide\"><a:dk1><a:srgbClr val=\"000000\"/></a:dk1><a:lt1><a:srgbClr val=\"FFFFFF\"/></a:lt1><a:dk2><a:srgbClr val=\"1F2937\"/></a:dk2><a:lt2><a:srgbClr val=\"F3F4F6\"/></a:lt2><a:accent1><a:srgbClr val=\"4472C4\"/></a:accent1><a:accent2><a:srgbClr val=\"ED7D31\"/></a:accent2><a:accent3><a:srgbClr val=\"A5A5A5\"/></a:accent3><a:accent4><a:srgbClr val=\"FFC000\"/></a:accent4><a:accent5><a:srgbClr val=\"5B9BD5\"/></a:accent5><a:accent6><a:srgbClr val=\"70AD47\"/></a:accent6><a:hlink><a:srgbClr val=\"0563C1\"/></a:hlink><a:folHlink><a:srgbClr val=\"954F72\"/></a:folHlink></a:clrScheme><a:fontScheme name=\"XSlide\"><a:majorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont><a:minorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont></a:fontScheme><a:fmtScheme name=\"XSlide\"><a:fillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w=\"6350\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln><a:ln w=\"12700\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln><a:ln w=\"19050\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>";

fn build_pptx(slides: &[Slide]) -> Result<Vec<u8>, String> {
    if slides.is_empty() {
        return Err("presentation has no slides".into());
    }
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let mut types = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/><Default Extension=\"jpg\" ContentType=\"image/jpeg\"/><Default Extension=\"gif\" ContentType=\"image/gif\"/><Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/><Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml\"/><Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/><Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>");
        for i in 0..slides.len() {
            types.push_str(&format!("<Override PartName=\"/ppt/slides/slide{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>", i + 1));
        }
        types.push_str("</Types>");
        zip_file(&mut zip, "[Content_Types].xml", &types)?;
        zip_file(&mut zip, "_rels/.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"ppt/presentation.xml\"/></Relationships>")?;

        let mut presentation = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:presentation xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\">");
        // PowerPoint rejects a deck unless every slide links to a layout owned by a master.
        let master_rid = slides.len() + 1;
        presentation.push_str(&format!(
            "<p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId{master_rid}\"/></p:sldMasterIdLst><p:sldIdLst>"
        ));
        let mut rels = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
        for i in 0..slides.len() {
            presentation.push_str(&format!(
                "<p:sldId id=\"{}\" r:id=\"rId{}\"/>",
                256 + i,
                i + 1
            ));
            rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{}.xml\"/>", i + 1, i + 1));
        }
        rels.push_str(&format!("<Relationship Id=\"rId{master_rid}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"slideMasters/slideMaster1.xml\"/><Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"theme/theme1.xml\"/>", master_rid + 1));
        presentation.push_str("</p:sldIdLst><p:sldSz cx=\"12192000\" cy=\"6858000\" type=\"screen16x9\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/></p:presentation>");
        rels.push_str("</Relationships>");
        zip_file(&mut zip, "ppt/presentation.xml", &presentation)?;
        zip_file(&mut zip, "ppt/_rels/presentation.xml.rels", &rels)?;
        zip_file(&mut zip, "ppt/slideMasters/slideMaster1.xml", SLIDE_MASTER_XML)?;
        zip_file(&mut zip, "ppt/slideMasters/_rels/slideMaster1.xml.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"../theme/theme1.xml\"/></Relationships>")?;
        zip_file(&mut zip, "ppt/slideLayouts/slideLayout1.xml", SLIDE_LAYOUT_XML)?;
        zip_file(&mut zip, "ppt/slideLayouts/_rels/slideLayout1.xml.rels", "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"../slideMasters/slideMaster1.xml\"/></Relationships>")?;
        zip_file(&mut zip, "ppt/theme/theme1.xml", THEME_XML)?;

        for (i, slide) in slides.iter().enumerate() {
            let (xml, images) = slide_xml(slide, i + 1)?;
            zip_file(&mut zip, &format!("ppt/slides/slide{}.xml", i + 1), &xml)?;
            let mut slide_rels = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
            // Images use rId1..rIdN inside the slide XML, so the layout takes the next id.
            slide_rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/>", images.len() + 1));
            for (j, (ext, bytes)) in images.iter().enumerate() {
                let file = format!("image{}_{}.{}", i + 1, j + 1, ext);
                slide_rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{}\"/>", j + 1, file));
                zip.start_file(format!("ppt/media/{file}"), SimpleFileOptions::default())
                    .map_err(|e| e.to_string())?;
                zip.write_all(bytes).map_err(|e| e.to_string())?;
            }
            slide_rels.push_str("</Relationships>");
            zip_file(
                &mut zip,
                &format!("ppt/slides/_rels/slide{}.xml.rels", i + 1),
                &slide_rels,
            )?;
        }
        zip.finish().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

fn zip_file(
    zip: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
    name: &str,
    data: &str,
) -> Result<(), String> {
    zip.start_file(name, SimpleFileOptions::default())
        .map_err(|e| e.to_string())?;
    zip.write_all(data.as_bytes()).map_err(|e| e.to_string())
}

fn slide_xml(slide: &Slide, slide_no: usize) -> Result<(String, Vec<(String, Vec<u8>)>), String> {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><p:sld xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld>");
    if let Some(color) = slide.background.as_deref().and_then(rgb) {
        xml.push_str(&format!("<p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"{color}\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"));
    }
    xml.push_str("<p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>");
    let mut images = Vec::new();
    for (index, el) in slide.elements.iter().enumerate() {
        let x = coord(el.x, SLIDE_W);
        let y = coord(el.y, SLIDE_H);
        let w = coord(el.w, SLIDE_W).max(1);
        let h = coord(el.h, SLIDE_H).max(1);
        let transform =
            format!("<a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm>");
        if el.kind == "image" {
            let src = el.src.as_deref().ok_or("image has no source")?;
            let (ext, data) = decode_image(src)?;
            images.push((ext, data));
            let rid = images.len();
            xml.push_str(&format!("<p:pic><p:nvPicPr><p:cNvPr id=\"{}\" name=\"Picture {}\"/><p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"rId{rid}\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{transform}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>", index + 2, index + 1));
        } else if matches!(el.kind.as_str(), "video" | "audio") {
            return Err(format!(
                "slide {slide_no} contains unsupported {} media",
                el.kind
            ));
        } else {
            xml.push_str(&text_shape(el, index + 2, &transform));
        }
    }
    xml.push_str("</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>");
    Ok((xml, images))
}

fn text_shape(el: &SlideElement, id: usize, transform: &str) -> String {
    let mut out = format!("<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"Text {id}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr><p:spPr>{transform}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap=\"square\"/><a:lstStyle/>");
    for line in el.text.split('\n') {
        let align = match el.align.as_deref() {
            Some("center") => "ctr",
            Some("right") => "r",
            Some("justify") => "just",
            _ => "l",
        };
        let size = (el.font_size.unwrap_or(18.0).clamp(1.0, 200.0) * 100.0).round() as u32;
        let strike = if el.strikethrough == Some(true) {
            "sngStrike"
        } else {
            "noStrike"
        };
        let baseline = match el.vertical_align.as_deref() {
            Some("super") => 30000,
            Some("sub") => -25000,
            _ => 0,
        };
        out.push_str(&format!("<a:p><a:pPr algn=\"{align}\"/><a:r><a:rPr lang=\"en-US\" sz=\"{size}\" b=\"{}\" i=\"{}\" u=\"{}\" strike=\"{strike}\" baseline=\"{baseline}\">", if el.bold == Some(true) { 1 } else { 0 }, if el.italic == Some(true) { 1 } else { 0 }, if el.underline == Some(true) { "sng" } else { "none" }));
        if let Some(color) = el.color.as_deref().and_then(rgb) {
            out.push_str(&format!(
                "<a:solidFill><a:srgbClr val=\"{color}\"/></a:solidFill>"
            ));
        }
        if let Some(color) = el.highlight.as_deref().and_then(rgb) {
            out.push_str(&format!(
                "<a:highlight><a:srgbClr val=\"{color}\"/></a:highlight>"
            ));
        }
        if let Some(font) = el.font_family.as_deref() {
            out.push_str(&format!("<a:latin typeface=\"{}\"/>", xml_escape(font)));
        }
        out.push_str("</a:rPr><a:t>");
        out.push_str(&xml_escape(line));
        out.push_str("</a:t></a:r><a:endParaRPr lang=\"en-US\"/></a:p>");
    }
    out.push_str("</p:txBody></p:sp>");
    out
}

fn decode_image(src: &str) -> Result<(String, Vec<u8>), String> {
    let (header, payload) = src
        .split_once(',')
        .ok_or("image must be embedded for export")?;
    let ext = match header.to_ascii_lowercase().as_str() {
        "data:image/png;base64" => "png",
        "data:image/jpeg;base64" | "data:image/jpg;base64" => "jpg",
        "data:image/gif;base64" => "gif",
        _ => return Err("unsupported image type for PowerPoint export".into()),
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| e.to_string())?;
    Ok((ext.into(), bytes))
}

fn coord(percent: f64, dimension: f64) -> i64 {
    if percent.is_finite() {
        (percent.clamp(0.0, 100.0) / 100.0 * dimension).round() as i64
    } else {
        0
    }
}

fn rgb(value: &str) -> Option<String> {
    let hex = value.strip_prefix('#')?;
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(hex.to_ascii_uppercase())
    } else {
        None
    }
}

fn xml_escape(s: &str) -> String {
    s.chars()
        .filter(|c| matches!(*c, '\t' | '\n' | '\r') || *c >= ' ')
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn sanitize_filename(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| {
            // Keep letters from every script; headers carry them via RFC 5987 `filename*`.
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let t = t.trim().trim_matches('.');
    if t.is_empty() {
        "presentation".into()
    } else {
        t.chars().take(80).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pptx_opens_back_with_all_slides() {
        let mut first = crate::documents::default_title_slide();
        first.elements[0].text = "First & <title>".into();
        first.elements[0].x = 21.0;
        first.elements[0].y = 31.0;
        first.elements[0].font_family = Some("Arial".into());
        first.elements[0].color = Some("#123abc".into());
        first.elements[0].highlight = Some("#ffee00".into());
        let mut image = first.elements[0].clone();
        image.kind = "image".into();
        image.text.clear();
        image.src = Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC".into());
        image.mime = Some("image/png".into());
        first.elements.push(image);
        let mut second = crate::documents::default_title_slide();
        second.elements[0].text = "Second slide".into();
        let bytes = build_pptx(&[first, second]).unwrap();
        let opened =
            crate::files::open_bytes("deck.pptx", "deck.pptx", "pptx", "deck", &bytes).unwrap();
        assert_eq!(opened.slides.len(), 2);
        assert!(opened.slides[0]
            .elements
            .iter()
            .any(|e| e.text == "First & <title>"));
        assert!((opened.slides[0].elements[0].x - 21.0).abs() < 0.01);
        assert!((opened.slides[0].elements[0].y - 31.0).abs() < 0.01);
        assert_eq!(
            opened.slides[0].elements[0].font_family.as_deref(),
            Some("Arial")
        );
        assert_eq!(
            opened.slides[0].elements[0].color.as_deref(),
            Some("#123ABC")
        );
        assert_eq!(
            opened.slides[0].elements[0].highlight.as_deref(),
            Some("#FFEE00")
        );
        assert!(opened.slides[0].elements.iter().any(|e| e.kind == "image"
            && e.src
                .as_deref()
                .unwrap_or("")
                .starts_with("data:image/png;base64,")));
        assert!(opened.slides[1]
            .elements
            .iter()
            .any(|e| e.text == "Second slide"));
        let mut layered = crate::documents::default_title_slide();
        let mut front = layered.elements[0].clone();
        front.kind = "image".into();
        front.text.clear();
        front.src = Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC".into());
        layered.elements.insert(0, front);
        let bytes = build_pptx(&[layered]).unwrap();
        let opened =
            crate::files::open_bytes("layered.pptx", "layered.pptx", "pptx", "layered", &bytes)
                .unwrap();
        assert_eq!(opened.slides[0].elements[0].kind, "image");
    }
}
