// GoPro Webcam Studio - Tauri application entry point

pub mod compositor;
pub mod editor;
pub mod frame;
pub mod output;
pub mod scene;
pub mod source_bus;
pub mod sources;

use std::sync::{Arc, Mutex};

use output::virtual_cam::{VirtualCamManager, VIRTUAL_CAM_NAME};

use crate::source_bus::SourceBus;
use sources::screen::{ScreenCaptureSource, ScreenFrameInput};
use sources::uvc::{GoProDetection, UvcConfig, UvcSource};

/// Connection health status for sources
#[derive(Debug, Clone, serde::Serialize, Default)]
pub enum SourceHealth {
    /// Source is connected and healthy
    Healthy,
    /// Source is connected but showing issues (frame drops, latency)
    Degraded,
    /// Source is disconnected
    Disconnected,
    /// Source is not initialized
    #[default]
    Uninitialized,
}

/// Global screen capture source instance
static SCREEN_SOURCE: once_cell::sync::Lazy<Arc<Mutex<Option<ScreenCaptureSource>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(None)));

/// Receive a screen frame from the frontend (via getDisplayMedia)
#[tauri::command]
fn receive_screen_frame(frame: ScreenFrameInput) -> Result<String, String> {
    let mut guard = SCREEN_SOURCE.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut source) = *guard {
        let timestamp_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        let frame_buffer = source.frame_buffer();
        if let Ok(mut buf) = frame_buffer.lock() {
            buf.data = Some(frame.data);
            buf.width = frame.width;
            buf.height = frame.height;
            buf.timestamp_us = timestamp_us;
        }
        Ok("Frame received".to_string())
    } else {
        Err("Screen source not initialized".to_string())
    }
}

/// Initialize the screen capture source
#[tauri::command]
fn init_screen_source(width: u32, height: u32, fps: f32) -> Result<String, String> {
    let config = sources::screen::ScreenCaptureConfig {
        source_id: "screen".to_string(),
        width,
        height,
        fps,
    };

    let mut guard = SCREEN_SOURCE.lock().map_err(|e| e.to_string())?;
    *guard = Some(ScreenCaptureSource::with_config(config));

    Ok("Screen source initialized".to_string())
}

/// Connect to the screen source
#[tauri::command]
fn connect_screen_source() -> Result<String, String> {
    let mut guard = SCREEN_SOURCE.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut source) = *guard {
        source.connect().map_err(|e| e.to_string())?;
        Ok("Screen source connected".to_string())
    } else {
        Err("Screen source not initialized".to_string())
    }
}

/// Disconnect from the screen source
#[tauri::command]
fn disconnect_screen_source() -> Result<String, String> {
    let mut guard = SCREEN_SOURCE.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut source) = *guard {
        source.disconnect().map_err(|e| e.to_string())?;
        Ok("Screen source disconnected".to_string())
    } else {
        Err("Screen source not initialized".to_string())
    }
}

/// Get the next frame from the screen source
#[tauri::command]
fn next_screen_frame() -> Result<Vec<u8>, String> {
    let mut guard = SCREEN_SOURCE.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut source) = *guard {
        source.next_frame().map_err(|e| e.to_string())
    } else {
        Err("Screen source not initialized".to_string())
    }
}

/// Get health status of the screen source
#[tauri::command]
fn get_screen_source_health() -> SourceHealth {
    let guard = SCREEN_SOURCE.lock().ok();

    match guard {
        Some(inner) => match &*inner {
            Some(source) if source.is_connected() => SourceHealth::Healthy,
            Some(_) => SourceHealth::Disconnected,
            None => SourceHealth::Uninitialized,
        },
        None => SourceHealth::Uninitialized,
    }
}

/// Attempt to reconnect to the screen source
#[tauri::command]
fn reconnect_screen_source() -> Result<String, String> {
    let mut guard = SCREEN_SOURCE.lock().map_err(|e| e.to_string())?;

    if let Some(ref mut source) = *guard {
        // First disconnect if connected
        if source.is_connected() {
            let _ = source.disconnect();
        }
        // Reconnect
        source
            .connect()
            .map_err(|e| format!("Reconnect failed: {}", e))?;
        Ok("Screen source reconnected".to_string())
    } else {
        Err("Screen source not initialized - please initialize first".to_string())
    }
}

/// Check if screen source is currently connected
#[tauri::command]
fn is_screen_source_connected() -> bool {
    SCREEN_SOURCE
        .lock()
        .ok()
        .map(|guard| guard.as_ref().map(|s| s.is_connected()).unwrap_or(false))
        .unwrap_or(false)
}

/// Get last error message from screen source
#[tauri::command]
fn get_screen_source_error() -> Option<String> {
    // In a more complete implementation, we'd track last errors
    // For now, check connection state
    let guard = SCREEN_SOURCE.lock().ok();
    match guard {
        Some(inner) => {
            if let Some(source) = &*inner {
                if !source.is_connected() {
                    return Some("Source disconnected".to_string());
                }
                None
            } else {
                Some("Source not initialized".to_string())
            }
        }
        None => Some("Failed to lock source".to_string()),
    }
}

/// Global UVC (GoPro USB) source instance
static UVC_SOURCE: once_cell::sync::Lazy<Arc<Mutex<Option<UvcSource>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(None)));

/// Detect a GoPro camera (combined MSMF name match + USB VID match).
/// Returns a structured diagnostic for the frontend, including all visible
/// cameras so the UI can offer a manual device picker if auto-detection misses.
#[tauri::command]
fn detect_gopro() -> GoProDetection {
    UvcSource::detect_gopro()
}

/// List all cameras visible to the system (MSMF backend).
#[tauri::command]
fn list_cameras() -> Result<Vec<String>, String> {
    UvcSource::list_cameras().map_err(|e| e.to_string())
}

/// Find the GoPro camera by name in the MSMF camera list.
/// Returns (index, name) if found.
#[tauri::command]
fn find_gopro_camera() -> Result<Option<(u32, String)>, String> {
    UvcSource::find_gopro_camera().map_err(|e| e.to_string())
}

/// Initialize the UVC source with a given camera index and resolution.
#[tauri::command]
fn init_uvc_source(camera_index: u32, width: u32, height: u32, fps: u32) -> Result<String, String> {
    let config = UvcConfig {
        camera_index,
        width,
        height,
        fps,
        source_id: "gopro-uvc".to_string(),
    };
    let mut guard = UVC_SOURCE.lock().map_err(|e| e.to_string())?;
    *guard = Some(UvcSource::with_config(config));
    Ok(format!(
        "UVC source initialized: camera {} at {}x{} {}fps",
        camera_index, width, height, fps
    ))
}

/// Connect to the UVC source (opens the camera stream).
#[tauri::command]
fn connect_uvc_source() -> Result<String, String> {
    let mut guard = UVC_SOURCE.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut source) = *guard {
        source.connect().map_err(|e| e.to_string())?;
        Ok("UVC source connected".to_string())
    } else {
        Err("UVC source not initialized - call init_uvc_source first".to_string())
    }
}

/// Disconnect from the UVC source.
#[tauri::command]
fn disconnect_uvc_source() -> Result<String, String> {
    let mut guard = UVC_SOURCE.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut source) = *guard {
        source.disconnect().map_err(|e| e.to_string())?;
        Ok("UVC source disconnected".to_string())
    } else {
        Ok("UVC source not initialized (already clean)".to_string())
    }
}

/// Get the next frame from the UVC source as raw RGB bytes.
#[tauri::command]
fn next_uvc_frame() -> Result<Vec<u8>, String> {
    let mut guard = UVC_SOURCE.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut source) = *guard {
        source.next_frame().map_err(|e| e.to_string())
    } else {
        Err("UVC source not initialized".to_string())
    }
}

/// Get frame metadata (width, height, fps) from the UVC source.
#[tauri::command]
fn get_uvc_frame_metadata() -> Result<Option<(u32, u32, f32)>, String> {
    let guard = UVC_SOURCE.lock().map_err(|e| e.to_string())?;
    if let Some(ref source) = *guard {
        Ok(source.frame_metadata().map(|m| (m.width, m.height, m.fps)))
    } else {
        Ok(None)
    }
}

/// Check if the UVC source is currently connected.
#[tauri::command]
fn is_uvc_source_connected() -> bool {
    UVC_SOURCE
        .lock()
        .ok()
        .map(|guard| guard.as_ref().map(|s| s.is_connected()).unwrap_or(false))
        .unwrap_or(false)
}

/// Get health status of the UVC source.
#[tauri::command]
fn get_uvc_source_health() -> SourceHealth {
    let guard = UVC_SOURCE.lock().ok();
    match guard {
        Some(inner) => match &*inner {
            Some(source) if source.is_connected() => SourceHealth::Healthy,
            Some(_) => SourceHealth::Disconnected,
            None => SourceHealth::Uninitialized,
        },
        None => SourceHealth::Uninitialized,
    }
}

/// Attempt to reconnect to the UVC source.
#[tauri::command]
fn reconnect_uvc_source() -> Result<String, String> {
    let mut guard = UVC_SOURCE.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut source) = *guard {
        if source.is_connected() {
            let _ = source.disconnect();
        }
        source
            .connect()
            .map_err(|e| format!("Reconnect failed: {}", e))?;
        Ok("UVC source reconnected".to_string())
    } else {
        Err("UVC source not initialized - call init_uvc_source first".to_string())
    }
}

/// Initialize the UVC source from auto-detected GoPro (uses detect_gopro's index).
/// Convenience command combining detect + init in one call.
#[tauri::command]
fn init_uvc_from_detected() -> Result<GoProDetection, String> {
    let detection = UvcSource::detect_gopro();
    if let Some(idx) = detection.camera_index {
        let config = UvcConfig {
            camera_index: idx,
            width: 1920,
            height: 1080,
            fps: 30,
            source_id: "gopro-uvc".to_string(),
        };
        let mut guard = UVC_SOURCE.lock().map_err(|e| e.to_string())?;
        *guard = Some(UvcSource::with_config(config));
        log::info!(
            "UVC source initialized from detected GoPro at index {}",
            idx
        );
    }
    Ok(detection)
}

/// Global virtual camera manager instance
static VIRTUAL_CAM_MANAGER: once_cell::sync::Lazy<Arc<Mutex<VirtualCamManager>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(VirtualCamManager::new())));

/// Register the virtual camera with Windows (makes it available in OBS/Zoom/Teams)
#[tauri::command]
fn register_virtual_cam() -> Result<String, String> {
    let manager = VIRTUAL_CAM_MANAGER.lock().map_err(|e| e.to_string())?;
    manager.register().map_err(|e| e.to_string())?;
    Ok(format!("{} virtual camera registered", VIRTUAL_CAM_NAME))
}

/// Unregister the virtual camera from Windows
#[tauri::command]
fn unregister_virtual_cam() -> Result<String, String> {
    let manager = VIRTUAL_CAM_MANAGER.lock().map_err(|e| e.to_string())?;
    manager.unregister().map_err(|e| e.to_string())?;
    Ok(format!("{} virtual camera unregistered", VIRTUAL_CAM_NAME))
}

/// Check if the virtual camera is registered
#[tauri::command]
fn is_virtual_cam_registered() -> bool {
    VIRTUAL_CAM_MANAGER
        .lock()
        .map(|m| m.is_registered())
        .unwrap_or(false)
}

/// Start the virtual camera streaming
#[tauri::command]
fn start_virtual_cam(width: u32, height: u32, fps: u32) -> Result<String, String> {
    use output::virtual_cam::{MediaType, VideoFormat};

    let manager = VIRTUAL_CAM_MANAGER.lock().map_err(|e| e.to_string())?;
    let filter = manager.get_default_filter();

    let format = VideoFormat {
        width,
        height,
        fps,
        media_type: MediaType::RGB24,
    };

    // Set the format directly
    filter.set_format(format).map_err(|e| e.to_string())?;

    // Start the filter
    filter.start().map_err(|e| e.to_string())?;

    Ok(format!(
        "{} virtual camera started at {}x{} {}fps",
        VIRTUAL_CAM_NAME, width, height, fps
    ))
}

/// Stop the virtual camera streaming
#[tauri::command]
fn stop_virtual_cam() -> Result<String, String> {
    let manager = VIRTUAL_CAM_MANAGER.lock().map_err(|e| e.to_string())?;
    let filter = manager.get_default_filter();

    filter.stop().map_err(|e| e.to_string())?;

    Ok(format!("{} virtual camera stopped", VIRTUAL_CAM_NAME))
}

/// Push a frame to the virtual camera
#[tauri::command]
fn push_virtual_cam_frame(frame_data: Vec<u8>, width: u32, height: u32) -> Result<String, String> {
    use output::virtual_cam::VirtualCamFrame;

    let manager = VIRTUAL_CAM_MANAGER.lock().map_err(|e| e.to_string())?;
    let filter = manager.get_default_filter();

    let frame = VirtualCamFrame::from_data(frame_data, width, height);
    filter.push_frame(frame).map_err(|e| e.to_string())?;

    Ok("Frame pushed".to_string())
}

/// Stitch a set of media files into a single edited, publish-ready video using a
/// local LLM agent (vision "eyes" + reasoning "director") and FFmpeg to render.
/// Original files are never modified; a new combined file is produced at `out_path`.
#[tauri::command]
fn ai_stitch(files: Vec<String>, out_path: String) -> Result<editor::StitchResult, String> {
    editor::ai_stitch(&files, &out_path).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // Screen source commands
            receive_screen_frame,
            init_screen_source,
            connect_screen_source,
            disconnect_screen_source,
            next_screen_frame,
            get_screen_source_health,
            reconnect_screen_source,
            is_screen_source_connected,
            get_screen_source_error,
            // GoPro UVC source commands
            detect_gopro,
            list_cameras,
            find_gopro_camera,
            init_uvc_source,
            init_uvc_from_detected,
            connect_uvc_source,
            disconnect_uvc_source,
            next_uvc_frame,
            get_uvc_frame_metadata,
            is_uvc_source_connected,
            get_uvc_source_health,
            reconnect_uvc_source,
            // Virtual camera commands
            register_virtual_cam,
            unregister_virtual_cam,
            is_virtual_cam_registered,
            start_virtual_cam,
            stop_virtual_cam,
            push_virtual_cam_frame,
            // AI clip-stitching editor
            ai_stitch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
