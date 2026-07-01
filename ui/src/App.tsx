import { useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface ScreenFrameInput {
  data: number[];
  width: number;
  height: number;
}

function App() {
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const frameIntervalRef = useRef<number | null>(null);
  
  const [status, setStatus] = useState<string>("Ready");
  const [fps, setFps] = useState<number>(0);
  const [error, setError] = useState<string | null>(null);
  const [isCapturing, setIsCapturing] = useState<boolean>(false);

  const frameCountRef = useRef(0);
  const lastTimeRef = useRef(performance.now());

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

  const captureFrame = useCallback(() => {
    const video = videoRef.current;
    const canvas = canvasRef.current;
    
    if (!video || !canvas || video.readyState < 2) {
      return;
    }

    const ctx = canvas.getContext('2d', { willReadFrequently: true });
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
    
    // Convert RGBA to RGB (skip every 4th byte)
    for (let i = 0; i < imageData.data.length; i += 4) {
      rgbData.push(imageData.data[i]);     // R
      rgbData.push(imageData.data[i + 1]); // G
      rgbData.push(imageData.data[i + 2]); // B
    }

    // Send frame to Tauri backend
    const frameInput: ScreenFrameInput = {
      data: rgbData,
      width: canvas.width,
      height: canvas.height,
    };

    invoke("receive_screen_frame", { frame: frameInput }).catch((err) => {
      console.error("Failed to send frame:", err);
    });

    updateFps();
  }, [updateFps]);

  const startCapture = async () => {
    try {
      setError(null);
      setStatus("Requesting screen capture...");

      // Initialize screen source in backend first
      await invoke("init_screen_source", { 
        width: 1920, 
        height: 1080, 
        fps: 30 
      });

      const stream = await navigator.mediaDevices.getDisplayMedia({
        video: {
          width: { ideal: 1920 },
          height: { ideal: 1080 },
          frameRate: { ideal: 30 }
        },
        audio: false
      });

      streamRef.current = stream;
      setStatus("Capture started!");
      setIsCapturing(true);

      // Connect to screen source in backend
      await invoke("connect_screen_source");

      if (videoRef.current) {
        videoRef.current.srcObject = stream;
        videoRef.current.onloadedmetadata = () => {
          videoRef.current?.play();
          
          // Start capturing frames at ~30fps
          frameIntervalRef.current = window.setInterval(captureFrame, 33);
        };
      }

      // Handle stream ending
      stream.getVideoTracks()[0].onended = () => {
        setStatus("Capture ended by user");
        stopCapture();
      };
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      setStatus("Failed to capture");
    }
  };

  const stopCapture = () => {
    if (frameIntervalRef.current) {
      clearInterval(frameIntervalRef.current);
      frameIntervalRef.current = null;
    }

    if (streamRef.current) {
      streamRef.current.getTracks().forEach(track => track.stop());
      streamRef.current = null;
    }

    invoke("disconnect_screen_source").catch(console.error);
    
    setStatus("Capture stopped");
    setFps(0);
    setIsCapturing(false);
  };

  return (
    <div className="app">
      <header className="app-header">
        <h1>GoPro Webcam Studio</h1>
        <p className="status">Status: <span>{status}</span></p>
      </header>
      <main className="app-content">
        <div className="controls">
          <button 
            onClick={startCapture} 
            disabled={isCapturing}
            className="primary-button"
          >
            Select Screen/Window
          </button>
          <button 
            onClick={stopCapture} 
            disabled={!isCapturing}
            className="secondary-button"
          >
            Stop Capture
          </button>
        </div>

        {fps > 0 && (
          <p className="fps">FPS: <span>{fps}</span></p>
        )}

        {error && (
          <p className="error">Error: {error}</p>
        )}

        {/* Hidden video element for capturing */}
        <video 
          ref={videoRef} 
          autoPlay 
          playsInline 
          style={{ display: 'none' }}
        />

        {/* Hidden canvas for frame extraction */}
        <canvas ref={canvasRef} style={{ display: 'none' }} />

        <div className="preview">
          {isCapturing ? (
            <p>Screen capture active - frames being sent to backend</p>
          ) : (
            <p className="placeholder">Click "Select Screen/Window" to start capturing</p>
          )}
        </div>
      </main>
    </div>
  );
}

export default App;