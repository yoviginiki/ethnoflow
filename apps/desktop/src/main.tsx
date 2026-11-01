import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ScreenBoundary } from "./ScreenBoundary";
import "./styles.css";

// A root boundary too: an error in App-level hooks (layers, transport, bridge) used to
// unmount everything and leave a blank window (2026-11-01, a mixer pref without its new
// strip); now it shows the error with "Try again" instead.
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ScreenBoundary screen="home">
      <App />
    </ScreenBoundary>
  </React.StrictMode>,
);
