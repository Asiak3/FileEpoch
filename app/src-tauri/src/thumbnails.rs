//! Image thumbnails for the icon views.
//!
//! The image is read through the same whitelisted, read-only object endpoint used for restores,
//! decoded and shrunk in memory, and returned as a small JPEG/PNG data URL. Nothing is written to
//! disk; the UI keeps a bounded in-memory cache.

use std::io::Cursor;
use std::sync::Arc;

use base64::Engine;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};
use kopia_client::{KopiaClient, Oid};
use tokio::sync::Semaphore;

use crate::commands::{AppState, CmdError};

/// Larger files are not downloaded for a thumbnail.
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
/// Longest side of a thumbnail, in pixels (twice the largest size shown, for high-DPI screens).
const THUMB_SIZE: u32 = 192;
/// Images bigger than this are refused before decoding (protects memory).
const MAX_DIMENSION: u32 = 16_384;
const MAX_DECODE_BYTES: u64 = 512 * 1024 * 1024;

/// Limits how many thumbnails are downloaded and decoded at the same time.
pub struct Thumbnails {
    permits: Semaphore,
}

impl Default for Thumbnails {
    fn default() -> Self {
        Thumbnails { permits: Semaphore::new(3) }
    }
}

/// Returns a data URL, or `None` when the file is too big or not a decodable image
/// (the UI then keeps the generic icon).
#[tauri::command]
pub async fn thumbnail(
    state: tauri::State<'_, AppState>,
    thumbs: tauri::State<'_, Thumbnails>,
    oid: String,
) -> Result<Option<String>, CmdError> {
    let oid: Oid = oid.parse().map_err(|_| CmdError::other("invalid object id"))?;
    let client = state.client()?;
    let _permit = thumbs.permits.acquire().await.map_err(|_| CmdError::other("shutting down"))?;
    let Some(bytes) = download(&client, &oid).await? else { return Ok(None) };
    let encoded = tokio::task::spawn_blocking(move || make_thumbnail(&bytes))
        .await
        .map_err(|e| CmdError::other(format!("thumbnail task failed: {e}")))?;
    Ok(encoded)
}

async fn download(client: &Arc<KopiaClient>, oid: &Oid) -> Result<Option<Vec<u8>>, CmdError> {
    let mut stream = client.open_object(oid, None).await?;
    if stream.content_length().is_some_and(|n| n > MAX_SOURCE_BYTES) {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.chunk().await? {
        if bytes.len() as u64 + chunk.len() as u64 > MAX_SOURCE_BYTES {
            return Ok(None);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Some(bytes))
}

/// Decodes, applies the EXIF orientation, shrinks and encodes. `None` if it is not a supported image.
fn make_thumbnail(bytes: &[u8]) -> Option<String> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);

    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?;
    reader.limits(limits);
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).ok()?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    let thumb =
        if img.width() > THUMB_SIZE || img.height() > THUMB_SIZE { img.thumbnail(THUMB_SIZE, THUMB_SIZE) } else { img };

    let mut out = Cursor::new(Vec::new());
    let mime = if thumb.color().has_alpha() {
        thumb.to_rgba8().write_to(&mut out, image::ImageFormat::Png).ok()?;
        "image/png"
    } else {
        let rgb = DynamicImage::ImageRgb8(thumb.to_rgb8());
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82).encode_image(&rgb).ok()?;
        "image/jpeg"
    };
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(out.into_inner())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, alpha: bool) -> Vec<u8> {
        let img = if alpha {
            DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 128])))
        } else {
            DynamicImage::ImageRgb8(image::RgbImage::from_pixel(w, h, image::Rgb([10, 20, 30])))
        };
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn decode(url: &str) -> DynamicImage {
        let b64 = url.split_once(";base64,").unwrap().1;
        image::load_from_memory(&base64::engine::general_purpose::STANDARD.decode(b64).unwrap()).unwrap()
    }

    #[test]
    fn shrinks_and_keeps_the_aspect_ratio() {
        let url = make_thumbnail(&png(1000, 500, false)).unwrap();
        assert!(url.starts_with("data:image/jpeg;base64,"));
        let t = decode(&url);
        assert_eq!((t.width(), t.height()), (192, 96));
    }

    #[test]
    fn small_images_are_not_enlarged_and_transparency_is_kept() {
        let url = make_thumbnail(&png(40, 30, true)).unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        let t = decode(&url);
        assert_eq!((t.width(), t.height()), (40, 30));
    }

    #[test]
    fn non_images_give_none() {
        assert!(make_thumbnail(b"%PDF-1.7 not an image").is_none());
        assert!(make_thumbnail(&[]).is_none());
        let mut broken = png(100, 100, false);
        broken.truncate(60);
        assert!(make_thumbnail(&broken).is_none());
    }
}
