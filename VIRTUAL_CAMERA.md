# Virtual Camera Setup

## Options for Virtual Camera on Windows

### Option 1: OBS Virtual Camera (Recommended)
The easiest approach - use OBS's built-in virtual camera:

1. Download and install OBS Studio: https://obsproject.com
2. In OBS, go to Tools > Virtual Camera
3. Start your GoPro Webcam Studio and select it as a source in OBS
4. In your video app (Zoom/Teams), select "OBS Virtual Camera" as the camera

### Option 2: DirectShow Filter Build (Alternative)
To build the C++ DirectShow filter manually:

```bash
# Requires MSYS2 + MinGW or Visual Studio
cd spikes/T0-C
mkdir -p build && cd build
cmake -G "MinGW Makefiles" ..
mingw32-make
```

The filter will be built as `goprovc.ax`.

### Option 3: Use existing OBS-level virtual camera
The Rust wrapper in `src-tauri/src/output/virtual_cam.rs` provides the API - the actual DLL is complex to build without proper build tools.

---

## Current Status

- ✅ Rust API for virtual camera exists (`VirtualCamManager`)
- ✅ UI enhanced with source switching and preview
- ⚠️ DirectShow filter DLL requires build tools

## Recommendation

Use OBS Virtual Camera for immediate testing. The Rust code is ready to integrate with any virtual camera solution via the frame push API.