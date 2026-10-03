import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { EngineContext, tauriEngine } from "./engine";
import { createMockEngine } from "./mock";
import "./styles.css";

// Inside the app window, the real engine. In a plain browser, or with
// `npm run dev:mock`, a made-up one, for working on the interface (UI-1).
const inApp = "__TAURI_INTERNALS__" in window && import.meta.env.MODE !== "mock";
// `?first-launch` shows the made-up engine as a new install.
const firstLaunch = new URLSearchParams(window.location.search).has("first-launch");
const engine = inApp ? tauriEngine : createMockEngine({ firstLaunch });

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <EngineContext value={engine}>
        <App />
      </EngineContext>
    </StrictMode>,
  );
}
