import { useRef, useState, useCallback } from "react";
import "./App.css";

function App() {
  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const recordedChunksRef = useRef<Blob[]>([]);
  const [status, setStatus] = useState<string>("Ready to capture");
  const [fps, setFps] = useState<number>(0);
  const [error, setError] = useState<string | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
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
    requestAnimationFrame(updateFps);
  }, []);

  const startCapture = async () => {
    try {
      setError(null);
      setStatus("Requesting screen capture...");
      
      const stream = await navigator.mediaDevices.getDisplayMedia({
        video: {
          width: { ideal: 1920 },
          height: { ideal: 1080 },
          frameRate: { ideal: 30 }
        },
        audio: false
      });
      
      streamRef.current = stream;
      setStatus("Capture started! Rendering to canvas...");
      
      if (videoRef.current) {
        videoRef.current.srcObject = stream;
        videoRef.current.onloadedmetadata = () => {
          videoRef.current?.play();
          requestAnimationFrame(updateFps);
          renderToCanvas();
        };
      }

      // Handle stream ending
      stream.getVideoTracks()[0].onended = () => {
        setStatus("Capture ended by user");
        stopRecording();
      };
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      setStatus("Failed to capture");
    }
  };

  const renderToCanvas = () => {
    const video = videoRef.current;
    const canvas = canvasRef.current;
    
    if (video && canvas && video.readyState >= 2) {
      const ctx = canvas.getContext('2d');
      if (ctx) {
        // Match canvas size to video
        canvas.width = video.videoWidth || 640;
        canvas.height = video.videoHeight || 480;
        
        // Draw video frame to canvas
        ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
      }
    }
    
    if (streamRef.current?.getVideoTracks()[0].readyState === 'live') {
      requestAnimationFrame(renderToCanvas);
    }
  };

  const stopCapture = () => {
    if (streamRef.current) {
      streamRef.current.getTracks().forEach(track => track.stop());
      streamRef.current = null;
      setStatus("Capture stopped");
      setFps(0);
    }
    stopRecording();
  };

  const startRecording = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    // Get canvas stream at 30fps
    const canvasStream = canvas.captureStream(30);
    
    const mediaRecorder = new MediaRecorder(canvasStream, {
      mimeType: 'video/webm;codecs=vp9',
      videoBitsPerSecond: 5000000
    });

    recordedChunksRef.current = [];

    mediaRecorder.ondataavailable = (event) => {
      if (event.data.size > 0) {
        recordedChunksRef.current.push(event.data);
      }
    };

    mediaRecorder.onstop = () => {
      const blob = new Blob(recordedChunksRef.current, { type: 'video/webm' });
      const url = URL.createObjectURL(blob);
      
      // Download the recording
      const a = document.createElement('a');
      a.href = url;
      a.download = 'recording.mp4';
      a.click();
      
      URL.revokeObjectURL(url);
      setStatus("Recording saved!");
    };

    mediaRecorderRef.current = mediaRecorder;
    mediaRecorder.start(1000); // Collect data every second
    setStatus("Recording started...");
  };

  const stopRecording = () => {
    if (mediaRecorderRef.current && mediaRecorderRef.current.state === 'recording') {
      mediaRecorderRef.current.stop();
      setStatus("Recording stopped");
    }
  };

  return (
    <main className="container">
      <h1>T0-A: getDisplayMedia Demo</h1>
      <p className="status">Status: <span>{status}</span></p>
      
      {fps > 0 && (
        <p className="fps">FPS: <span>{fps}</span></p>
      )}

      {error && (
        <p className="error">Error: {error}</p>
      )}

      <div className="controls">
        <button onClick={startCapture} disabled={!!streamRef.current}>
          Start Screen Capture
        </button>
        <button onClick={stopCapture} disabled={!streamRef.current}>
          Stop Capture
        </button>
        <button onClick={startRecording} disabled={!streamRef.current}>
          Start Recording
        </button>
        <button onClick={stopRecording} disabled={!mediaRecorderRef.current || mediaRecorderRef.current?.state !== 'recording'}>
          Stop Recording
        </button>
      </div>

      {/* Hidden video element for capturing */}
      <video 
        ref={videoRef} 
        autoPlay 
        playsInline 
        style={{ display: 'none' }}
      />

      {/* Canvas for rendering */}
      <div className="canvas-container">
        <canvas ref={canvasRef} />
      </div>

      <p className="info">
        Canvas renders the screen stream using requestAnimationFrame for smooth playback.
        Click "Start Recording" to capture 10 seconds of the canvas display.
      </p>
    </main>
  );
}

export default App;