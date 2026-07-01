//! Desktop webcam capture via `nokhwa`. Android capture is not yet implemented
//! (see TODO.md); on Android `CameraSession::open` simply reports unavailable.

use anyhow::Result;

#[cfg(not(target_os = "android"))]
pub use desktop::CameraSession;

#[cfg(target_os = "android")]
pub use android::CameraSession;

#[cfg(not(target_os = "android"))]
mod desktop {
    use anyhow::{Context, Result};
    use nokhwa::Camera;
    use nokhwa::pixel_format::RgbFormat;
    use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};

    pub struct CameraSession {
        camera: Camera,
    }

    impl CameraSession {
        /// Open the default camera and start streaming.
        pub fn open() -> Result<Self> {
            let format =
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
            let mut camera =
                Camera::new(CameraIndex::Index(0), format).context("Failed to open camera")?;
            camera.open_stream().context("Failed to start camera")?;
            Ok(Self { camera })
        }

        /// Grab one frame as `(rgba, width, height)`.
        pub fn grab(&mut self) -> Result<(Vec<u8>, u32, u32)> {
            let frame = self.camera.frame().context("Failed to read camera frame")?;
            let img = frame
                .decode_image::<RgbFormat>()
                .context("Failed to decode camera frame")?;
            let (w, h) = (img.width(), img.height());
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for chunk in img.into_raw().chunks_exact(3) {
                rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            Ok((rgba, w, h))
        }
    }
}

#[cfg(target_os = "android")]
mod android {
    use anyhow::{Result, bail};

    pub struct CameraSession;

    impl CameraSession {
        pub fn open() -> Result<Self> {
            bail!("Camera capture is not yet available on Android")
        }
        pub fn grab(&mut self) -> Result<(Vec<u8>, u32, u32)> {
            bail!("Camera capture is not yet available on Android")
        }
    }
}

/// Save an RGBA frame to a temporary square-able PNG and return its path. The
/// avatar pipeline (`Profile::set_avatar`) will centre-crop and resize it.
pub fn save_frame_png(rgba: &[u8], width: u32, height: u32) -> Result<std::path::PathBuf> {
    use anyhow::Context;
    let path = std::env::temp_dir().join(format!("rondelek_cam_{}.png", crate::util::new_uid()));
    let img =
        image::RgbaImage::from_raw(width, height, rgba.to_vec()).context("Invalid camera frame")?;
    img.save_with_format(&path, image::ImageFormat::Png)
        .context("Failed to write captured frame")?;
    Ok(path)
}
