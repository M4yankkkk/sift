import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
// Fonts are bundled so the app makes no network requests (no Google Fonts at runtime).
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "./index.css";

const rootElement = document.getElementById("root");
if (rootElement) {
  ReactDOM.createRoot(rootElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>
  );
}
