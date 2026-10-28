import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { initTheme } from "./lib/theme";
import "./index.css";

initTheme();

// Behave like a desktop app: no browser context menu outside text fields,
// and no reload or print shortcuts that would reset or print the window.
if (import.meta.env.PROD) {
  window.addEventListener("contextmenu", (e) => {
    const t = e.target as HTMLElement | null;
    if (!t?.closest("input, textarea, .selectable")) e.preventDefault();
  });
  window.addEventListener("keydown", (e) => {
    const mod = e.ctrlKey || e.metaKey;
    if (e.key === "F5" || (mod && ["r", "p", "u"].includes(e.key.toLowerCase()))) e.preventDefault();
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
