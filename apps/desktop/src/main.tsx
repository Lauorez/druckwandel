import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App.js";
import { SettingsView } from "./SettingsView.js";
import { isSettingsRoute } from "./settingsWindow.js";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {isSettingsRoute() ? <SettingsView /> : <App />}
  </React.StrictMode>,
);
