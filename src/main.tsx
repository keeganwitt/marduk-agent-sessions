import React from "react";
import ReactDOM from "react-dom/client";
import { openUrl } from "@tauri-apps/plugin-opener";
import App from "./App";
import { installExternalLinkHandler } from "./lib/externalLinks";
import { applyTheme, cachedTheme } from "./lib/theme";

installExternalLinkHandler(document, openUrl);
applyTheme(cachedTheme());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
