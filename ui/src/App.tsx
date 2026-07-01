import { useState } from "react";
import "./App.css";

function App() {
  const [status] = useState("Initializing...");

  return (
    <div className="app">
      <header className="app-header">
        <h1>GoPro Webcam Studio</h1>
        <p className="status">{status}</p>
      </header>
      <main className="app-content">
        <div className="placeholder">
          <p>Tauri skeleton initialized</p>
          <p>Full implementation coming in subsequent tasks</p>
        </div>
      </main>
    </div>
  );
}

export default App;