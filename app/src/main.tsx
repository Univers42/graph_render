import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App.tsx";
import { studioTokens } from "./ui/studioTokens.ts";
import "./studio.css";
import "./controls.css";

/**
 * The engine's theme is read from live `--osio-*` custom properties
 * (`resolveSceneTheme`), so the studio publishes its own values on the document
 * root before the first panel resolves a theme. Importing this module for its
 * side effect is the whole point of it.
 */
studioTokens();

const host = document.getElementById("root");
if (host === null) throw new Error("#root is missing from index.html");

createRoot(host).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
