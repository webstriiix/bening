import "./polyfills";
import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/space-grotesk";
import "@fontsource/space-mono";
import "./theme.css";
import "./components.css";
import App from "./App";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
