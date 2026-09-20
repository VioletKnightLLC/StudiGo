import { useRef, useState, useCallback, useEffect } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

// Types
type SourceType = "screen" | "gopro" | "generic" | "wifi" | "bluetooth" | "screen-fallback";
type MediaKind = "image" | "video";
interface MediaItem {
  id: string;
  name: string;
  kind: MediaKind;
  enabled: boolean;
  active: boolean;
}
 type SceneLayout = "single" | "pip" | "side_by_side" | "grid";
 type TransitionType = "cut" | "fade" | "slide" | "push" | "zoom";
 type MediaRecorderState = "idle" | "countdown" | "recording" | "paused" | "stopped";

 interface SourceState {
 id: SourceType;
 name: string;
 enabled: boolean;
 active: boolean;
 connected: boolean;
 position: "primary" | "secondary" | "inactive";
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

 interface GoProDetection {
 detected: boolean;
 camera_named: boolean;
 usb_vid_match: boolean;
 camera_index: number | null;
 camera_name: string | null;
 all_cameras: string[];
 message: string;
 }

 interface RecordingState {
 state: MediaRecorderState;
 duration: number;
 filePath: string | null;
 error: string | null;
 }

 function App() {
 const previewCanvasRef = useRef<HTMLCanvasElement>(null);
 const [sources, setSources] = useState<SourceState[]>([
 { id: "screen", name: "Screen", enabled: false, active: false, connected: false, position: "inactive" },
 { id: "gopro", name: "GoPro", enabled: false, active: false, connected: false, position: "inactive" },
 { id: "generic", name: "Generic Camera", enabled: false, active: false, connected: false, position: "inactive" },
 { id: "wifi", name: "WiFi", enabled: false, active: false, connected: false, position: "inactive" },
 { id: "bluetooth", name: "Bluetooth", enabled: false, active: false, connected: false, position: "inactive" },
 ]);

 const enabledSourcesRef = useRef<Set<SourceType>>(new Set());
 const sourceVideosRef = useRef<Map<SourceType, HTMLVideoElement>>(new Map());
 const sourceStreamsRef = useRef<Map<SourceType, MediaStream>>(new Map());
 const mediaItemsRef = useRef<MediaItem[]>([]);
 const mediaElsRef = useRef<Map<string, HTMLImageElement | HTMLVideoElement>>(new Map());
 const [mediaItems, setMediaItems] = useState<MediaItem[]>([]);

 const [sceneLayout, setSceneLayout] = useState<SceneLayout>("single");
 const [status, setStatus] = useState<string>("Ready");
void status; // Mark as intentionally unused (used via setStatus for debugging)
 const [fps, setFps] = useState<number>(0);
 const [resolution, setResolution] = useState<string>("1920x1080");
 const [isCapturing, setIsCapturing] = useState<boolean>(false);
 const [connectionState, setConnectionState] = useState<string>("Disconnected");
 const [sourceHealth, setSourceHealth] = useState<string>("Uninitialized");
 const [autoReconnect, setAutoReconnect] = useState<boolean>(true);
 const [recordingState, setRecordingState] = useState<RecordingState>({ state: "idle", duration: 0, filePath: null, error: null });
 const [countdownValue, setCountdownValue] = useState<number | null>(null);
 const [isPreviewMaximized, setIsPreviewMaximized] = useState<boolean>(false);
 const [settings, setSettings] = useState<Settings>({ outputResolution: "1920x1080", transition: "cut" });
 const [showSettings, setShowSettings] = useState<boolean>(false);
 const [toasts, setToasts] = useState<Toast[]>([]);
 const toastIdRef = useRef(0);
 const frameCountRef = useRef(0);
 const lastTimeRef = useRef(performance.now());
 const rafRef = useRef<number | null>(null);
 const recordingTimerRef = useRef<number | null>(null);
 const mediaRecorderRef = useRef<MediaRecorder | null>(null);
 const recordedChunksRef = useRef<Blob[]>([]);

 // Toast helpers
 const showToast = useCallback((message: string, type: Toast["type"] = "info") => {
 const id = ++toastIdRef.current;
 setToasts((prev) => [...prev, { id, message, type }]);
 setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 4000);
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

 // Get primary and secondary source IDs (fixed sources only)
 const getPrimarySource = useCallback((): SourceType | null => {
 const s = sources.find((s) => s.enabled && s.position === "primary");
 return s ? s.id : sources.find((s) => s.enabled)?.id || null;
 }, [sources]);

 const getSecondarySource = useCallback((): SourceType | null => {
 return sources.find((s) => s.enabled && s.position === "secondary")?.id || null;
 }, [sources]);

 // Ordered list of every active drawable: enabled fixed sources first, then enabled media.
 const getActiveDrawables = useCallback((): string[] => {
 const fixed = sources
   .filter((s) => s.enabled)
   .sort((a, b) => {
     const rank = { primary: 0, secondary: 1, inactive: 2 };
     return (rank[a.position] ?? 2) - (rank[b.position] ?? 2);
   })
   .map((s) => (s.position === "primary" ? s.id : s.id));
 const media = mediaItems.filter((m) => m.enabled).map((m) => `media:${m.id}`);
 return [...fixed, ...media];
 }, [sources, mediaItems]);

 // Resolve + draw any source (fixed SourceType or "media:<id>") into a canvas region.
 const drawSource = useCallback((ctx: CanvasRenderingContext2D, sourceId: string, x: number, y: number, w: number, h: number) => {
 let vid: HTMLVideoElement | null = null;
 let img: HTMLImageElement | null = null;
 if (sourceId.startsWith("media:")) {
   const el = mediaElsRef.current.get(sourceId.slice(6));
   if (el instanceof HTMLVideoElement) vid = el;
   else if (el instanceof HTMLImageElement) img = el;
 } else {
   vid = sourceVideosRef.current.get(sourceId as SourceType) ?? null;
 }

 // Readiness + dimensions (video width/height vs image natural/natural)
 let ready = false;
 let elW = 0;
 let elH = 0;
 if (vid) {
   ready = vid.readyState >= 2 && vid.videoWidth > 0;
   elW = vid.videoWidth;
   elH = vid.videoHeight;
 } else if (img) {
   ready = img.complete && img.naturalWidth > 0;
   elW = img.naturalWidth;
   elH = img.naturalHeight;
 }

 if (!ready || (elW === 0 && elH === 0)) {
   ctx.fillStyle = "#1a1a2e";
   ctx.fillRect(x, y, w, h);
   ctx.fillStyle = "#555";
   ctx.font = "14px sans-serif";
   ctx.textAlign = "center";
   ctx.fillText(`${sourceId}: not ready`, x + w / 2, y + h / 2);
   return;
 }
 const srcAspect = elW / Math.max(1, elH);
 const targetAspect = w / Math.max(1, h);
 let drawW = w, drawH = h, drawX = x, drawY = y;
 if (srcAspect > targetAspect) {
   drawH = w / srcAspect;
   drawY = y + (h - drawH) / 2;
 } else {
   drawW = h * srcAspect;
   drawX = x + (w - drawW) / 2;
 }
 const el = vid ?? img!;
 ctx.drawImage(el, Math.round(drawX), Math.round(drawY), Math.round(drawW), Math.round(drawH));
 }, []);

 // Render loop
 const renderFrame = useCallback(() => {
 const canvas = previewCanvasRef.current;
 if (!canvas) return;
 const ctx = canvas.getContext("2d");
 if (!ctx) return;

 const [width, height] = resolution.split("x").map(Number);
 const drawables = getActiveDrawables();
 const primary = drawables[0] ?? null;
 const secondary = drawables[1] ?? null;

 canvas.width = isPreviewMaximized ? window.innerWidth : 480;
 canvas.height = isPreviewMaximized ? window.innerHeight : Math.round((480 * height) / width);

 ctx.fillStyle = "#000";
 ctx.fillRect(0, 0, canvas.width, canvas.height);

 if (sceneLayout === "single") {
 if (primary) drawSource(ctx, primary, 0, 0, canvas.width, canvas.height);
 } else if (sceneLayout === "pip") {
 if (primary) drawSource(ctx, primary, 0, 0, canvas.width, canvas.height);
 if (secondary) {
 const pipW = canvas.width * 0.3;
 const pipH = (pipW * height) / width;
 const pipX = canvas.width - pipW - 12;
 const pipY = canvas.height - pipH - 12;
 ctx.fillStyle = "#000";
 ctx.fillRect(pipX - 2, pipY - 2, pipW + 4, pipH + 4);
 drawSource(ctx, secondary, pipX, pipY, pipW, pipH);
 ctx.strokeStyle = "#00d4aa";
 ctx.lineWidth = 2;
 ctx.strokeRect(pipX, pipY, pipW, pipH);
 }
 } else if (sceneLayout === "side_by_side") {
 const halfW = canvas.width / 2;
 if (drawables[0]) drawSource(ctx, drawables[0], 0, 0, halfW, canvas.height);
 if (drawables[1]) drawSource(ctx, drawables[1], halfW, 0, halfW, canvas.height);
 ctx.strokeStyle = "#00d4aa";
 ctx.lineWidth = 2;
 ctx.beginPath();
 ctx.moveTo(halfW, 0);
 ctx.lineTo(halfW, canvas.height);
 ctx.stroke();
 } else if (sceneLayout === "grid") {
 const halfW = canvas.width / 2;
 const halfH = canvas.height / 2;
 const slots = [
 [0, 0, halfW, halfH],
 [halfW, 0, halfW, halfH],
 [0, halfH, halfW, halfH],
 [halfW, halfH, halfW, halfH],
 ];
 slots.forEach(([sx, sy, sw, sh], idx) => {
 if (drawables[idx]) drawSource(ctx, drawables[idx], sx, sy, sw, sh);
 else {
 ctx.fillStyle = "#151525";
 ctx.fillRect(sx + 1, sy + 1, sw - 2, sh - 2);
 }
 });
 }

 updateFps();
 rafRef.current = requestAnimationFrame(renderFrame);
 }, [sources, sceneLayout, resolution, isPreviewMaximized, getPrimarySource, getSecondarySource, drawSource, getActiveDrawables, updateFps]);

 // Start render loop when capturing
 useEffect(() => {
 if (isCapturing) {
 rafRef.current = requestAnimationFrame(renderFrame);
 } else if (rafRef.current) {
 cancelAnimationFrame(rafRef.current);
 rafRef.current = null;
 }
 return () => {
 if (rafRef.current) {
 cancelAnimationFrame(rafRef.current);
 rafRef.current = null;
 }
 };
 }, [isCapturing, renderFrame]);

 // Enable a source
 const enableSource = async (sourceId: SourceType) => {
 try {
 setStatus(`Enabling ${sourceId}...`);

 if (sourceId === "screen") {
 const stream = await navigator.mediaDevices.getDisplayMedia({
 video: { width: { ideal: 1920 }, height: { ideal: 1080 }, frameRate: { ideal: 30 } },
 audio: false,
 });
 sourceStreamsRef.current.set("screen", stream);

 const vid = document.createElement("video");
 vid.srcObject = stream;
 vid.autoplay = true;
 vid.playsInline = true;
 vid.style.display = "none";
 document.body.appendChild(vid);
 sourceVideosRef.current.set("screen", vid);

 setSources((prev) =>
 prev.map((s) =>
 s.id === sourceId
 ? { ...s, enabled: true, active: true, connected: true, position: "primary" }
 : s.position === "primary" && s.enabled ? { ...s, position: "secondary" } : s
 )
 );
 enabledSourcesRef.current.add("screen");
 setStatus("Screen capture enabled");

 } else if (sourceId === "generic") {
 const stream = await navigator.mediaDevices.getUserMedia({
 video: { width: { ideal: 1920 }, height: { ideal: 1080 }, frameRate: { ideal: 30 } },
 audio: false,
 });
 sourceStreamsRef.current.set("generic", stream);

 const vid = document.createElement("video");
 vid.srcObject = stream;
 vid.autoplay = true;
 vid.playsInline = true;
 vid.style.display = "none";
 document.body.appendChild(vid);
 sourceVideosRef.current.set("generic", vid);

 setSources((prev) => {
 const hasPrimary = prev.some((s) => s.enabled && s.position === "primary");
 return prev.map((s) =>
 s.id === sourceId
 ? { ...s, enabled: true, active: true, connected: true, position: hasPrimary ? "secondary" : "primary" }
 : s
 );
 });
 enabledSourcesRef.current.add("generic");
 setStatus("Generic webcam enabled");

 } else if (sourceId === "gopro") {
 const detection = await invoke<GoProDetection>("detect_gopro");

 // Check if GoPro is detected as native UVC (Hero 9+)
 if (detection.camera_named && detection.camera_index !== null) {
 const [w, h] = resolution.split("x").map(Number);
 await invoke("init_uvc_source", { cameraIndex: detection.camera_index, width: w, height: h, fps: 30 });
 await invoke("connect_uvc_source");

 setSources((prev) => {
 const hasPrimary = prev.some((s) => s.enabled && s.position === "primary");
 return prev.map((s) =>
 s.id === sourceId
 ? { ...s, enabled: true, active: true, connected: true, position: hasPrimary ? "secondary" : "primary" }
 : s
 );
 });
 enabledSourcesRef.current.add("gopro");
 setStatus("GoPro enabled (native UVC)");
 showToast("GoPro connected via native UVC", "success");
 } else if (detection.usb_vid_match) {
 // Hero 8 - falls back to screen capture via GoPro Webcam Utility
 showToast("Hero 8 detected (RNDIS mode). Opening screen capture for GoPro Utility window.", "info");
 setStatus("GoPro enabled via screen fallback");

 // Auto-enable screen capture instead
 const stream = await navigator.mediaDevices.getDisplayMedia({
 video: { width: { ideal: 1920 }, height: { ideal: 1080 }, frameRate: { ideal: 30 } },
 audio: false,
 });
 sourceStreamsRef.current.set("screen-fallback", stream);

 const vid = document.createElement("video");
 vid.srcObject = stream;
 vid.autoplay = true;
 vid.playsInline = true;
 vid.style.display = "none";
 document.body.appendChild(vid);
 sourceVideosRef.current.set("screen-fallback", vid);

 setSources((prev) => {
 const hasPrimary = prev.some((s) => s.enabled && s.position === "primary");
 return prev.map((s) =>
 s.id === "screen"
 ? { ...s, enabled: true, active: true, connected: true, position: hasPrimary ? "secondary" : "primary" }
 : s.id === sourceId
 ? { ...s, enabled: true, active: true, connected: true, position: "inactive" } // mark gopro as inactive since we use screen fallback
 : s
 );
 });
 enabledSourcesRef.current.add("screen-fallback");
 setStatus("Screen capture (GoPro Utility) enabled");
 showToast("Screen capture active - position GoPro Utility window in view", "success");
 } else {
 showToast(detection.message, "error");
 return;
 }
 } else {
 showToast(`${sourceId} not implemented yet`, "info");
 return;
 }

 setIsCapturing(true);
 setConnectionState("Connected");
 setSourceHealth("Healthy");
 showToast(`${sourceId} connected`, "success");
 } catch (err) {
 const msg = err instanceof Error ? err.message : String(err);
 showToast(`Failed to enable ${sourceId}: ${msg}`, "error");
 setStatus(`Failed to enable ${sourceId}`);
 }
 };

 // Disable a source
 const disableSource = (sourceId: SourceType) => {
 const stream = sourceStreamsRef.current.get(sourceId);
 if (stream) {
 stream.getTracks().forEach((t) => t.stop());
 sourceStreamsRef.current.delete(sourceId);
 }
 const vid = sourceVideosRef.current.get(sourceId);
 if (vid) {
 vid.remove();
 sourceVideosRef.current.delete(sourceId);
 }

 setSources((prev) =>
 prev.map((s) =>
 s.id === sourceId ? { ...s, enabled: false, active: false, connected: false, position: "inactive" } : s
 )
 );
 enabledSourcesRef.current.delete(sourceId);

 if (sourceId === "gopro") {
 invoke("disconnect_uvc_source").catch(console.error);
 }
 if (sourceId === "screen-fallback") {
 invoke("disconnect_uvc_source").catch(console.error);
 }

 const remaining = enabledSourcesRef.current.size;
 setIsCapturing(remaining > 0);
 setConnectionState(remaining > 0 ? "Connected" : "Disconnected");
 setSourceHealth(remaining > 0 ? "Healthy" : "Disconnected");
 setStatus("Source disabled");
 };

 // Toggle source
 const toggleSource = async (sourceId: SourceType) => {
 const source = sources.find((s) => s.id === sourceId);
 if (!source) return;
 if (source.enabled) disableSource(sourceId);
 else await enableSource(sourceId);
 };

 // Normalize a file path to a webview-loadable asset URL (convertFileSrc is required to bypass webview file restrictions).
 const pathToUrl = (p: string) => convertFileSrc(p);

 // Import one or more local media files via the native OS file picker.
 const importMedia = async () => {
 try {
   const selected = await open({
     multiple: true,
     filters: [
       { name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp"] },
       { name: "Videos", extensions: ["mp4", "mov", "webm", "mkv", "avi", "m4v"] },
       { name: "All media", extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp", "mp4", "mov", "webm", "mkv", "avi", "m4v"] },
     ],
   });
   if (!selected) return; // cancelled
   const paths = (Array.isArray(selected) ? selected : [selected]).filter(Boolean);
   if (paths.length === 0) return;

   const newItems: MediaItem[] = [];
   for (const p of paths) {
     const id = `m${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
     const isImage = /\.(png|jpe?g|gif|webp|bmp)$/i.test(p);
     const name = p.split(/[\\\\/]/).pop() || id;
     const kind: MediaKind = isImage ? "image" : "video";
     // Only accept the platform URL form (browser can't read raw Windows paths).
     const url = pathToUrl(p);
     const item: MediaItem = { id, name, kind, enabled: true, active: true };
     newItems.push(item);
     mediaElsRef.current.set(id, loadMediaElement(url, kind));
   }

   mediaItemsRef.current = [...mediaItemsRef.current, ...newItems];
   setMediaItems(mediaItemsRef.current);
   setIsCapturing(true);
   setConnectionState("Connected");
   setSourceHealth("Healthy");
   setStatus(`Imported ${newItems.length} media file(s)`);
   showToast(`Imported ${newItems.length} file(s). Pick a layout to composite.`, "success");
 } catch (err) {
   showToast(`Import failed: ${err instanceof Error ? err.message : String(err)}`, "error");
 }
 };

 // Build the DOM element to draw (image loads immediately; video starts muted & looping).
 const loadMediaElement = (url: string, kind: MediaKind): HTMLImageElement | HTMLVideoElement => {
 if (kind === "image") {
   const img = new Image();
   img.src = url;
   return img;
 }
 const vid = document.createElement("video");
 vid.src = url;
 vid.muted = true;
 vid.loop = true;
 vid.autoplay = true;
 vid.playsInline = true;
 vid.crossOrigin = "anonymous";
 vid.style.display = "none";
 document.body.appendChild(vid);
 vid.play().catch(() => {});
 return vid;
 };

 // Toggle a media item's compositing.
 const toggleMedia = (id: string) => {
 mediaItemsRef.current = mediaItemsRef.current.map((m) =>
   m.id === id ? { ...m, enabled: !m.enabled, active: !m.enabled } : m
 );
 setMediaItems(mediaItemsRef.current);
 const remaining = mediaItemsRef.current.filter((m) => m.enabled).length;
 setIsCapturing(remaining > 0 || enabledSourcesRef.current.size > 0);
 };

 // Remove a media item.
 const removeMedia = (id: string) => {
 const el = mediaElsRef.current.get(id);
 if (el instanceof HTMLVideoElement) el.remove();
 mediaElsRef.current.delete(id);
 mediaItemsRef.current = mediaItemsRef.current.filter((m) => m.id !== id);
 setMediaItems(mediaItemsRef.current);
 const remaining = mediaItemsRef.current.filter((m) => m.enabled).length;
 setIsCapturing(remaining > 0 || enabledSourcesRef.current.size > 0);
 };

 // Set position
 const setSourcePosition = (sourceId: SourceType, position: "primary" | "secondary" | "inactive") => {
 setSources((prev) =>
 prev.map((s) => {
 if (s.id === sourceId) return { ...s, position };
 // If assigning primary, demote any other primary to secondary
 if (position === "primary" && s.id !== sourceId && s.position === "primary") {
 return { ...s, position: "secondary" };
 }
 return s;
 })
 );
 };

 // Stop all capture
 const stopCapture = useCallback(() => {
 if (recordingState.state === "recording" || recordingState.state === "paused") {
 if (mediaRecorderRef.current && mediaRecorderRef.current.state !== "inactive") {
 mediaRecorderRef.current.stop();
 }
 if (recordingTimerRef.current) {
 clearInterval(recordingTimerRef.current);
 recordingTimerRef.current = null;
 }
 }

 enabledSourcesRef.current.forEach((id) => disableSource(id));
 enabledSourcesRef.current.clear();

 invoke("disconnect_screen_source").catch(console.error);
 invoke("disconnect_uvc_source").catch(console.error);

 setStatus("Capture stopped");
 setFps(0);
 setIsCapturing(false);
 setConnectionState("Disconnected");
 }, [recordingState]);

 // Countdown logic
 const startCountdown = useCallback(
 (onComplete: () => void) => {
 let count = 3;
 setCountdownValue(count);
 setRecordingState((prev) => ({ ...prev, state: "countdown" }));
 const timer = window.setInterval(() => {
 count -= 1;
 if (count > 0) {
 setCountdownValue(count);
 } else {
 clearInterval(timer);
 setCountdownValue(null);
 setRecordingState({ state: "recording", duration: 0, filePath: null, error: null });
 onComplete();
 }
 }, 1000);
 },
 []
 );

 // Recording functions
 const startRecording = useCallback(() => {
 const primary = getPrimarySource();
 if (!primary) {
 showToast("No video source active. Please enable a camera source first.", "error");
 return;
 }

 startCountdown(() => {
 recordedChunksRef.current = [];
 const canvas = previewCanvasRef.current;
 if (!canvas) return;

 const stream = canvas.captureStream(30);
 const mimeType = "video/webm; codecs=vp9";

 try {
 const mediaRecorder = new MediaRecorder(stream, {
 mimeType: MediaRecorder.isTypeSupported(mimeType) ? mimeType : "video/webm",
 });

 mediaRecorder.ondataavailable = (event) => {
 if (event.data.size > 0) {
 recordedChunksRef.current.push(event.data);
 }
 };

 mediaRecorder.onstop = () => {
 const blob = new Blob(recordedChunksRef.current, { type: "video/webm" });
 const url = URL.createObjectURL(blob);
 const a = document.createElement("a");
 a.href = url;
 a.download = `gopro-webcam-recording-${new Date().toISOString().replace(/[:.]/g, "-")}.webm`;
 document.body.appendChild(a);
 a.click();
 document.body.removeChild(a);
 URL.revokeObjectURL(url);
 showToast("Recording saved! Check your Downloads folder.", "success");
 setRecordingState((prev) => ({ ...prev, state: "idle", duration: 0 }));
 };

 mediaRecorder.onerror = (event) => {
 console.error("MediaRecorder error:", event);
 showToast("Recording error occurred. Check console.", "error");
 setRecordingState((prev) => ({ ...prev, state: "idle", error: "Recording failed" }));
 };

 mediaRecorderRef.current = mediaRecorder;
 mediaRecorder.start(1000);

 let duration = 0;
 recordingTimerRef.current = window.setInterval(() => {
 duration += 1;
 setRecordingState((prev) => ({ ...prev, duration }));
 }, 1000);

 showToast("Recording started!", "success");
 } catch (err) {
 console.error("Failed to start recording:", err);
 showToast("Failed to start recording: " + String(err), "error");
 setRecordingState({ state: "idle", duration: 0, filePath: null, error: String(err) });
 }
 });
 }, [showToast, startCountdown, getPrimarySource]);

 const stopRecording = useCallback(() => {
 if (mediaRecorderRef.current && mediaRecorderRef.current.state !== "inactive") {
 mediaRecorderRef.current.stop();
 }
 if (recordingTimerRef.current) {
 clearInterval(recordingTimerRef.current);
 recordingTimerRef.current = null;
 }
 setRecordingState((prev) => ({ ...prev, state: "stopped" }));
 showToast("Recording stopped! Saving file...", "info");
 }, [showToast]);

 // Keyboard shortcuts
 useEffect(() => {
 const handleKeyDown = (e: KeyboardEvent) => {
 if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;

 if (e.key === "F11") {
 e.preventDefault();
 setIsPreviewMaximized((prev) => !prev);
 }
 if (e.key === "Escape") setIsPreviewMaximized(false);
 if ((e.key === "R" && e.shiftKey) || (e.key === "r" && e.ctrlKey)) {
 if (isCapturing && recordingState.state === "idle") startRecording();
 }
 if (e.key === "s" && !e.ctrlKey) setShowSettings((prev) => !prev);
 };
 window.addEventListener("keydown", handleKeyDown);
 return () => window.removeEventListener("keydown", handleKeyDown);
 }, [isCapturing, recordingState, startRecording]);

 // Cleanup on unmount
 useEffect(() => {
 return () => {
 if (rafRef.current) cancelAnimationFrame(rafRef.current);
 if (recordingTimerRef.current) clearInterval(recordingTimerRef.current);
 enabledSourcesRef.current.forEach((id) => {
 const stream = sourceStreamsRef.current.get(id);
 if (stream) stream.getTracks().forEach((t) => t.stop());
 const vid = sourceVideosRef.current.get(id);
 if (vid) vid.remove();
 });
 };
 }, []);

 const formatDuration = (seconds: number) => {
 const mins = Math.floor(seconds / 60);
 const secs = seconds % 60;
 return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
 };

 return (
 <div className="app">
 {/* Countdown overlay */}
 {countdownValue !== null && (
 <div className="countdown-overlay">
 <div className="countdown-number">{countdownValue}</div>
 <div className="countdown-label">Recording starts in...</div>
 </div>
 )}

 {/* Recording indicator */}
 {recordingState.state === "recording" && (
 <div className="recording-overlay">
 <div className="recording-indicator">
 <span className="recording-dot" />
 <span className="recording-label">REC</span>
 <span className="recording-time">{formatDuration(recordingState.duration)}</span>
 </div>
 </div>
 )}

 {/* Toasts */}
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
 <h2>Sources</h2>
 <p className="panel-hint">Enable multiple sources for PiP / Side-by-Side</p>
 <div className="source-list">
 {sources.map((source) => (
 <div key={source.id} className={`source-item ${source.enabled ? "enabled" : ""} ${source.connected ? "connected" : ""}`}>
 <label className="source-checkbox">
 <input
 type="checkbox"
 checked={source.enabled}
 onChange={() => toggleSource(source.id)}
 disabled={source.id === "wifi" || source.id === "bluetooth"}
 />
 <span className="source-icon">{getSourceIcon(source.id)}</span>
 <span className="source-name">{source.name}</span>
 </label>
 {source.enabled && (
 <select
 className="source-position-select"
 value={source.position}
 onChange={(e) => setSourcePosition(source.id, e.target.value as "primary" | "secondary" | "inactive")}
 >
 <option value="primary">Primary</option>
 <option value="secondary">PiP / Secondary</option>
 <option value="inactive">Inactive</option>
 </select>
 )}
 {source.connected && <span className="connection-dot" title="Connected" />}
 </div>
 ))}
 </div>
 </section>

 {/* Media library */}
 <section className="panel media-panel">
 <h2>Media Library</h2>
 <p className="panel-hint">Import local images/videos and composite them with any source</p>
 <button className="import-btn" onClick={importMedia}>
 <span className="import-icon">＋</span> Import Media Files
 </button>
 {mediaItems.length > 0 && (
 <div className="media-list">
 {mediaItems.map((item) => (
 <div key={item.id} className={`media-item ${item.enabled ? "enabled" : ""}`}>
 <label className="media-checkbox">
 <input
 type="checkbox"
 checked={item.enabled}
 onChange={() => toggleMedia(item.id)}
 />
 <span className="media-kind">{item.kind === "image" ? "🖼️" : "🎞️"}</span>
 <span className="media-name" title={item.name}>{item.name}</span>
 </label>
 <button className="media-remove" onClick={() => removeMedia(item.id)} title="Remove">
 ✕
 </button>
 </div>
 ))}
 </div>
 )}
 </section>

 {/* Recording controls */}
 <section className="panel recording-panel">
 <h2>Recording</h2>
 <div className="recording-controls">
 {recordingState.state === "idle" && (
 <button className="record-btn" onClick={startRecording} disabled={!isCapturing}>
 <span className="record-icon">🔴</span> Record
 </button>
 )}
 {(recordingState.state === "recording" || recordingState.state === "paused") && (
 <button className="stop-btn" onClick={stopRecording}>
 <span className="stop-icon">⏹️</span> Stop ({formatDuration(recordingState.duration)})
 </button>
 )}
 {recordingState.state === "countdown" && (
 <button className="countdown-btn" disabled>
 <span className="countdown-icon">⏳</span> Starting in {countdownValue}s...
 </button>
 )}
 {recordingState.state === "stopped" && (
 <button className="record-btn" onClick={startRecording}>
 <span className="record-icon">🔴</span> Record Again
 </button>
 )}
 </div>
 <div className="recording-info">
 {recordingState.error && <span className="recording-error">{recordingState.error}</span>}
 {recordingState.state === "idle" && !isCapturing && <span className="recording-hint">Enable a camera source first</span>}
 </div>
 </section>

 {/* Scene layout */}
 <section className="panel layout-panel">
 <h2>Layout</h2>
 <div className="layout-buttons">
 {(["single", "pip", "side_by_side", "grid"] as SceneLayout[]).map((layout) => (
 <button key={layout} className={`layout-btn ${sceneLayout === layout ? "active" : ""}`} onClick={() => setSceneLayout(layout)}>
 {getLayoutIcon(layout)}
 <span>{getLayoutName(layout)}</span>
 </button>
 ))}
 </div>
 </section>

 {/* Live preview */}
 <section className={`panel preview-panel ${isPreviewMaximized ? "maximized" : ""}`}>
 <h2>
 Preview
 <button className="maximize-btn" onClick={() => setIsPreviewMaximized(!isPreviewMaximized)} title={isPreviewMaximized ? "Restore (Esc)" : "Maximize (F11)"}>
 {isPreviewMaximized ? "⛶" : "⛯"}
 </button>
 </h2>
 <div className="preview-container" onDoubleClick={() => setIsPreviewMaximized(!isPreviewMaximized)}>
 <canvas ref={previewCanvasRef} className="preview-canvas" />
 {!isCapturing && (
 <div className="preview-placeholder">
 <p>Enable a source to start</p>
 </div>
 )}
 {isCapturing && recordingState.state !== "recording" && (
 <div className="preview-overlay">
 <span className="live-dot" /> LIVE
 </div>
 )}
 {isPreviewMaximized && (
 <button className="restore-btn" onClick={(e) => { e.stopPropagation(); setIsPreviewMaximized(false); }}>
 ✕ Restore
 </button>
 )}
 </div>
 </section>

 {/* Status */}
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
 </section>

 {/* Settings */}
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
 <select value={settings.transition} onChange={(e) => setSettings({ ...settings, transition: e.target.value as TransitionType })}>
 <option value="cut">Cut (Instant)</option>
 <option value="fade">Fade</option>
 <option value="slide">Slide</option>
 <option value="push">Push</option>
 <option value="zoom">Zoom</option>
 </select>
 </div>
 <div className="setting-item">
 <label>
 <input type="checkbox" checked={autoReconnect} onChange={(e) => setAutoReconnect(e.target.checked)} />
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
 </div>
 );
 }

 // Helpers
 function getSourceIcon(sourceId: SourceType): string {
 switch (sourceId) {
 case "screen": return "🖥️";
 case "gopro": return "📷";
 case "generic": return "📹";
 case "wifi": return "📶";
 case "bluetooth": return "🔵";
 default: return "📹";
 }
 }

 function getLayoutIcon(layout: SceneLayout): string {
 switch (layout) {
 case "single": return "▢";
 case "pip": return "▢⃝";
 case "side_by_side": return "▤";
 case "grid": return "▦";
 default: return "▢";
 }
 }

 function getLayoutName(layout: SceneLayout): string {
 switch (layout) {
 case "single": return "Single";
 case "pip": return "Picture-in-Picture";
 case "side_by_side": return "Side by Side";
 case "grid": return "2x2 Grid";
 default: return layout;
 }
 }

 export default App;