import "antd/dist/reset.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router";
import { App } from "./App.tsx";
import { CogniGraphProvider } from "./components/CogniGraphProvider.tsx";
import "./styles/base.css";
import "./styles/layout.css";
import "./styles/components.css";
import "./styles/inspector.css";
import "./styles/dialogs.css";
import "./styles/screens.css";
import "./styles/auth.css";
import "./styles/consoles.css";
import "./styles/graph-explorer.css";
import "./styles/review.css";

const container = document.getElementById("root");

if (!container) {
  throw new Error("CogniGraph UI root was not found");
}

// Bun's dev server re-evaluates this module on hot reload; calling
// createRoot twice on the same container is a React error. Reuse the root
// across HMR updates instead.
const globals = globalThis as { __cognigraphRoot?: ReturnType<typeof createRoot> };
if (!globals.__cognigraphRoot) {
  globals.__cognigraphRoot = createRoot(container);
}
const root = globals.__cognigraphRoot;

root.render(
  <StrictMode>
    <BrowserRouter>
      <CogniGraphProvider>
        <App />
      </CogniGraphProvider>
    </BrowserRouter>
  </StrictMode>,
);
