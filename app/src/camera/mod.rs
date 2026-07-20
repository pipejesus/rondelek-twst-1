//! Desktop webcam capture via `nokhwa` (macOS / Windows / Linux).

use anyhow::{Context, Result};
use image::RgbaImage;
use nokhwa::Camera;
use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};

/// Longest edge (px) we keep for the live preview / avatar source. Webcams
/// often stream 1080p; decoding, converting and uploading that every frame is
/// what made the preview crawl. The stored avatar is only 256px, so a 640px
/// working frame loses nothing while keeping per-frame work cheap.
const PREVIEW_MAX: u32 = 640;

pub struct CameraSession {
    camera: Camera,
}

impl CameraSession {
    /// Open the default camera and start streaming.
    pub fn open() -> Result<Self> {
        // `AbsoluteHighestFrameRate` is the only robust request here: the
        // Closest/Exact/Highest* variants require the camera to advertise an
        // exact format/resolution and fail to open otherwise.
        let format =
            RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
        let mut camera =
            Camera::new(CameraIndex::Index(0), format).context("Failed to open camera")?;
        camera.open_stream().context("Failed to start camera")?;
        Ok(Self { camera })
    }

    /// Grab one frame as `(rgba, width, height)`, downscaled to `PREVIEW_MAX`.
    pub fn grab(&mut self) -> Result<(Vec<u8>, u32, u32)> {
        let frame = self.camera.frame().context("Failed to read camera frame")?;
        // Decode straight to RGBA — the library does the colour conversion in
        // optimised code, replacing the old per-pixel Rust loop.
        let img = frame
            .decode_image::<RgbAFormat>()
            .context("Failed to decode camera frame")?;
        let img = downscale_max(img, PREVIEW_MAX);
        let (w, h) = (img.width(), img.height());
        Ok((img.into_raw(), w, h))
    }
}

/// Downscale `img` so its longest edge is at most `max`, preserving aspect.
/// Returns the original untouched when it already fits.
fn downscale_max(img: RgbaImage, max: u32) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    let longest = w.max(h);
    if longest <= max {
        return img;
    }
    let scale = max as f32 / longest as f32;
    let nw = ((w as f32 * scale).round() as u32).max(1);
    let nh = ((h as f32 * scale).round() as u32).max(1);
    image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
}

/// Save an RGBA frame to a temporary square-able PNG and return its path. The
/// avatar pipeline (`Profile::set_avatar`) will centre-crop and resize it.
pub fn save_frame_png(rgba: &[u8], width: u32, height: u32) -> Result<std::path::PathBuf> {
    let path = std::env::temp_dir().join(format!("rondelek_cam_{}.png", rondelek_core::util::new_uid()));
    let img =
        image::RgbaImage::from_raw(width, height, rgba.to_vec()).context("Invalid camera frame")?;
    img.save_with_format(&path, image::ImageFormat::Png)
        .context("Failed to write captured frame")?;
    Ok(path)
}
