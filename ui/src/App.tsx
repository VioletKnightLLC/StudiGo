import { useRef, useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

// Types matching backend
type SourceType = "screen" | "gopro" | "wifi" | "bluetooth";
type SceneLayout = "single" | "pip" | "side_by_side" | "grid";
type TransitionType = "cut" | "fade" | "slide" | "push" | "zoom";

interface SourceState {
  id: SourceType;
  name: string;
  active: boolean;
  connected: boolean;
}

interface Settings {
  outputResolution: string;
  transition: TransitionType;
}

interface Toast {
  id: number;
  message: string;
  type: "error" | "success" | "info";
}

function App() {
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const previewCanvasRef = useRef<HTMLCanvasElement>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const frameIntervalRef = useRef<number | null>(null);

  // Source states
  const [sources, setSources] = useState<SourceState[]>([
    { id: "screen", name: "Screen", active: false, connected: false },
    { id: "gopro", name: "GoPro", active: false, connected: false },
    { id: "wifi", name: "WiFi", active: false, connected: false },
    { id: "bluetooth", name: "Bluetooth", active: false, connected: false },
  ]);
  const [activeSource, setActiveSource] = useState<SourceType | null>(null);

  // Scene layout
  const [sceneLayout, setSceneLayout] = useState<SceneLayout>("single");

  // Status
  const [status, setStatus] = useState<string>("Ready");
  const [fps, setFps] = useState<number>(0);
  const [resolution, setResolution] = useState<string>("1920x1080");
  const [isCapturing, setIsCapturing] = useState<boolean>(false);
  const [connectionState, setConnectionState] = useState<string>("Disconnected");
  const [sourceHealth, setSourceHealth] = useState<string>("Uninitialized");
  const [autoReconnect, setAutoReconnect] = useState<boolean>(true);

  // Settings
  const [settings, setSettings] = useState<Settings>({
    outputResolution: "1920x1080",
    transition: "cut",
  });
  const [showSettings, setShowSettings] = useState<boolean>(false);

  // Toast notifications
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastIdRef = useRef(0);

  const frameCountRef = useRef(0);
  const lastTimeRef = useRef(performance.now());

  // Toast helpers
  const showToast = useCallback((message: string, type: Toast["type"] = "info") => {
    const id = ++toastIdRef.current;
    setToasts((prev) => [...prev, { id, message, type }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4000);
  }, []);

  const updateFps = useCallback(() => {
    const now = performance.now();
    const delta = now - lastTimeRef.current;
    frameCountRef.current++;

    if (delta >= 1000) {
      setFps(Math.round((frameCountRef.current * 1000) / delta));
      frameCountRef.current = 0;
      lastTimeRef.current = now;
    }
  }, []);

  // Update preview canvas with composited scene
  const updatePreview = useCallback(() => {
    const canvas = previewCanvasRef.current;
    const sourceCanvas = canvasRef.current;
    if (!canvas || !sourceCanvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const [width, height] = resolution.split("x").map(Number);
    canvas.width = 480; // Preview scaled down
    canvas.height = Math.round((480 * height) / width);

    // Clear
    ctx.fillStyle = "#1a1a2e";
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Draw based on layout
    const scale = canvas.width / width;

    if (sceneLayout === "single") {
      ctx.drawImage(
        sourceCanvas,
        0,
        0,
        sourceCanvas.width,
        sourceCanvas.height,
        0,
        0,
        canvas.width,
        canvas.height
      );
    } else if (sceneLayout === "pip") {
      // Main (full)
      ctx.drawImage(
        sourceCanvas,
        0,
        0,
        sourceCanvas.width,
        sourceCanvas.height,
        0,
        0,
        canvas.width,
        canvas.height
      );
      // PiP (small inset, bottom-right)
      const pipW = canvas.width * 0.25;
      const pipH = (pipW * height) / width;
      const pipX = canvas.width - pipW - 10;
      const pipY = canvas.height - pipH - 10;
      ctx.fillStyle = "#333";
      ctx.fillRect(pipX, pipY, pipW, pipH);
      ctx.strokeStyle = "#00ff88";
      ctx.lineWidth = 2;
      ctx.strokeRect(pipX, pipY, pipW, pipH);
    } else if (sceneLayout === "side_by_side") {
      const halfW = canvas.width / 2;
      ctx.drawImage(
        sourceCanvas,
        0,
        0,
        sourceCanvas.width / 2,
        sourceCanvas.height,
        0,
        0,
        halfW,
        canvas.height
      );
      ctx.drawImage(
        sourceCanvas,
        sourceCanvas.width / 2,
        0,
        sourceCanvas.width / 2,
        sourceCanvas.height,
        halfW,
        0,
        halfW,
        canvas.height
      );
      ctx.strokeStyle = "#00ff88";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(halfW, 0);
      ctx.lineTo(halfW, canvas.height);
      ctx.stroke();
    } else if (sceneLayout === "grid") {
      const quartW = canvas.width / 2;
      const quartH = canvas.height / 2;
      ctx.fillStyle = "#222";
      for (let i = 0; i < 4; i++) {
        const x = (i % 2) * quartW;
        const y = Math.floor(i / 2) * quartH;
        ctx.fillRect(x, y, quartW - 2, quartH - 2);
        ctx.strokeStyle = "#00ff88";
        ctx.strokeRect(x, y, quartW - 2, quartH - 2);
      }
    }
  }, [resolution, sceneLayout]);

  const captureFrame = useCallback(() => {
    const video = videoRef.current;
    const canvas = canvasRef.current;

    if (!video || !canvas || video.readyState < 2) {
      return;
    }

    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return;

    // Match canvas size to video
    if (canvas.width !== video.videoWidth || canvas.height !== video.videoHeight) {
      canvas.width = video.videoWidth || 1920;
      canvas.height = video.videoHeight || 1080;
    }

    // Draw video frame to canvas
    ctx.drawImage(video, 0, 0);

    // Get image data and convert to RGB
    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const rgbData: number[] = [];

    for (let i = 0; i < imageData.data.length; i += 4) {
      rgbData.push(imageData.data[i]);
      rgbData.push(imageData.data[i + 1]);
      rgbData.push(imageData.data[i + 2]);
    }

    // Send frame to Tauri backend
    invoke("receive_screen_frame", { frame: { data: rgbData, width: canvas.width, height: canvas.height } }).catch((err) => {
      console.error("Failed to send frame:", err);
    });

    updateFps();
    updatePreview();
  }, [updateFps, updatePreview]);

  const selectSource = async (sourceId: SourceType) => {
    try {
      setError(null);

      // Stop current capture if active
      if (isCapturing) {
        stopCapture();
      }

      setStatus(`Connecting to ${sourceId}...`);
      showToast(`Connecting to ${sourceId}...`, "info");

      if (sourceId === "screen") {
        await invoke("init_screen_source", { width: 1920, height: 1080, fps: 30 });

        const stream = await navigator.mediaDevices.getDisplayMedia({
          video: { width: { ideal: 1920 }, height: { ideal: 1080 }, frameRate: { ideal: 30 } },
          audio: false,
        });

        streamRef.current = stream;
        setStatus("Screen capture active");
        setIsCapturing(true);
        setConnectionState("Connected");

        await invoke("connect_screen_source");

        if (videoRef.current) {
          videoRef.current.srcObject = stream;
          videoRef.current.onloadedmetadata = () => {
            videoRef.current?.play();
            frameIntervalRef.current = window.setInterval(captureFrame, 33);
          };
        }

        stream.getVideoTracks()[0].onended = () => {
          setStatus("Capture ended by user");
          setConnectionState("Disconnected");
          stopCapture();
        };
      } else {
        // Other sources would connect via their respective backends
        setStatus(`${sourceId} source selected`);
        setConnectionState("Ready");
        showToast(`${sourceId} source activated`, "success");
      }

      setSources((prev) =>
        prev.map((s) => ({
          ...s,
          active: s.id === sourceId,
          connected: s.id === sourceId ? true : s.connected,
        }))
      );
      setActiveSource(sourceId);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      setStatus("Failed to connect");
      setConnectionState("Error");
      showToast(errorMessage, "error");
    }
  };

  const stopCapture = useCallback(() => {
    if (frameIntervalRef.current) {
      clearInterval(frameIntervalRef.current);
      frameIntervalRef.current = null;
    }

    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
    }

    invoke("disconnect_screen_source").catch(console.error);

    setStatus("Capture stopped");
    setFps(0);
    setIsCapturing(false);
    setConnectionState("Disconnected");
    setSources((prev) => prev.map((s) => ({ ...s, active: false, connected: false })));
    setActiveSource(null);
  }, []);

  const setError = (err: string | null) => {
    if (err) {
      showToast(err, "error");
    }
  };

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (frameIntervalRef.current) {
        clearInterval(frameIntervalRef.current);
      }
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((track) => track.stop());
      }
    };
  }, []);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't trigger shortcuts when typing in inputs
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;

      // 1-4: Switch sources
      if (e.key >= "1" && e.key <= "4") {
        const idx = parseInt(e.key) - 1;
        if (sources[idx]) {
          selectSource(sources[idx].id);
        }
      }
      // Space: Toggle capture
      if (e.key === " " && !e.repeat) {
        e.preventDefault();
        if (isCapturing) {
          stopCapture();
        } else if (activeSource) {
          selectSource(activeSource);
        }
      }
      // R: Reconnect
      if (e.key === "r" || e.key === "R") {
        handleReconnect();
      }
      // S: Toggle settings
      if (e.key === "s" && !e.ctrlKey) {
        setShowSettings(prev => !prev);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [sources, isCapturing, activeSource]);

  // Source health monitoring
  const checkSourceHealth = useCallback(async () => {
    try {
      const health = await invoke<string>("get_screen_source_health");
      setSourceHealth(health);

      // Auto-reconnect on disconnect
      if (health === "Disconnected" && autoReconnect && isCapturing) {
        showToast("Source disconnected - attempting reconnect...", "info");
        await handleReconnect();
      }
    } catch (err) {
      setSourceHealth("Uninitialized");
    }
  }, [autoReconnect, isCapturing, showToast]);

  // Health check interval
  useEffect(() => {
    const interval = setInterval(checkSourceHealth, 3000);
    return () => clearInterval(interval);
  }, [checkSourceHealth]);

  // Manual reconnect
  const handleReconnect = async () => {
    try {
      await invoke("reconnect_screen_source");
      showToast("Source reconnected", "success");
      setConnectionState("Connected");
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      showToast(`Reconnect failed: ${msg}`, "error");
    }
  };

  return (
    <div className="app">
      {/* Toast notifications */}
      <div className="toast-container">
        {toasts.map((toast) => (
          <div key={toast.id} className={`toast toast-${toast.type}`}>
            {toast.message}
          </div>
        ))}
      </div>

      <header className="app-header">
        <div className="header-content">
          <h1>GoPro Webcam Studio</h1>
          <button className="settings-toggle" onClick={() => setShowSettings(!showSettings)}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="3" />
              <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
            </svg>
          </button>
        </div>
      </header>

      <main className="app-content">
        {/* Source selector */}
        <section className="panel source-panel">
          <h2>Source</h2>
          <div className="source-buttons">
            {sources.map((source) => (
              <button
                key={source.id}
                className={`source-btn ${source.active ? "active" : ""} ${source.connected ? "connected" : ""}`}
                onClick={() => selectSource(source.id)}
              >
                <span className="source-icon">{getSourceIcon(source.id)}</span>
                <span className="source-name">{source.name}</span>
                {source.connected && <span className="connection-indicator" />}
              </button>
            ))}
          </div>
        </section>

        {/* Scene layout selector */}
        <section className="panel layout-panel">
          <h2>Layout</h2>
          <div className="layout-buttons">
            {(["single", "pip", "side_by_side", "grid"] as SceneLayout[]).map((layout) => (
              <button
                key={layout}
                className={`layout-btn ${sceneLayout === layout ? "active" : ""}`}
                onClick={() => setSceneLayout(layout)}
              >
                {getLayoutIcon(layout)}
                <span>{getLayoutName(layout)}</span>
              </button>
            ))}
          </div>
        </section>

        {/* Live preview */}
        <section className="panel preview-panel">
          <h2>Preview</h2>
          <div className="preview-container">
            <canvas ref={previewCanvasRef} className="preview-canvas" />
            {!isCapturing && !activeSource && (
              <div className="preview-placeholder">
                <p>Select a source to start</p>
              </div>
            )}
            {isCapturing && (
              <div className="preview-overlay">
                <span className="recording-dot" />
                LIVE
              </div>
            )}
          </div>
        </section>

        {/* Status panel */}
        <section className="panel status-panel">
          <h2>Status</h2>
          <div className="status-grid">
            <div className="status-item">
              <span className="status-label">State</span>
              <span className={`status-value status-${connectionState.toLowerCase()}`}>{connectionState}</span>
            </div>
            <div className="status-item">
              <span className="status-label">FPS</span>
              <span className="status-value">{fps}</span>
            </div>
            <div className="status-item">
              <span className="status-label">Resolution</span>
              <span className="status-value">{resolution}</span>
            </div>
            <div className="status-item">
              <span className="status-label">Layout</span>
              <span className="status-value">{getLayoutName(sceneLayout)}</span>
            </div>
            <div className="status-item">
              <span className="status-label">Health</span>
              <span className={`status-value health-${sourceHealth.toLowerCase()}`}>
                <span className="health-dot" data-health={sourceHealth}></span>
                {sourceHealth}
              </span>
            </div>
          </div>
          {isCapturing && (
            <div className="status-actions">
              <button className="reconnect-btn" onClick={handleReconnect} title="Press R to reconnect">
                🔄 Reconnect
              </button>
            </div>
          )}
        </section>

        {/* Settings panel */}
        {showSettings && (
          <section className="panel settings-panel">
            <h2>Settings</h2>
            <div className="settings-grid">
              <div className="setting-item">
                <label>Output Resolution</label>
                <select
                  value={settings.outputResolution}
                  onChange={(e) => {
                    setSettings({ ...settings, outputResolution: e.target.value });
                    setResolution(e.target.value);
                  }}
                >
                  <option value="1920x1080">1920x1080 (Full HD)</option>
                  <option value="1280x720">1280x720 (HD)</option>
                  <option value="640x480">640x480 (VGA)</option>
                  <option value="3840x2160">3840x2160 (4K)</option>
                </select>
              </div>
              <div className="setting-item">
                <label>Transition</label>
                <select
                  value={settings.transition}
                  onChange={(e) => setSettings({ ...settings, transition: e.target.value as TransitionType })}
                >
                  <option value="cut">Cut (Instant)</option>
                  <option value="fade">Fade</option>
                  <option value="slide">Slide</option>
                  <option value="push">Push</option>
                  <option value="zoom">Zoom</option>
                </select>
              </div>
              <div className="setting-item">
                <label>
                  <input
                    type="checkbox"
                    checked={autoReconnect}
                    onChange={(e) => setAutoReconnect(e.target.checked)}
                  />
                  Auto-reconnect on disconnect
                </label>
              </div>
            </div>
            <div className="settings-actions">
              <button className="action-btn stop" onClick={stopCapture} disabled={!isCapturing}>
                Stop Capture
              </button>
            </div>
          </section>
        )}
      </main>

      {/* Hidden capture elements */}
      <video ref={videoRef} autoPlay playsInline style={{ display: "none" }} />
      <canvas ref={canvasRef} style={{ display: "none" }} />
    </div>
  );
}

// Helper functions
function getSourceIcon(sourceId: SourceType): string {
  switch (sourceId) {
    case "screen":
      return "🖥️";
    case "gopro":
      return "📷";
    case "wifi":
      return "📶";
    case "bluetooth":
      return "🔵";
    default:
      return "📹";
  }
}

function getLayoutIcon(layout: SceneLayout): string {
  switch (layout) {
    case "single":
      return "▢";
    case "pip":
      return "▢⃝";
    case "side_by_side":
      return "▤";
    case "grid":
      return "▦";
    default:
      return "▢";
  }
}

function getLayoutName(layout: SceneLayout): string {
  switch (layout) {
    case "single":
      return "Single";
    case "pip":
      return "Picture-in-Picture";
    case "side_by_side":
      return "Side by Side";
    case "grid":
      return "2x2 Grid";
    default:
      return layout;
  }
}

export default App;