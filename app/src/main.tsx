import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import "ueblueprint/dist/css/ueb-style.min.css";
import { Blueprint } from "ueblueprint";

// Reference the import so tree-shaking can't drop the module side-effects
// that register the `<ueb-blueprint>` custom element.
if (!customElements.get("ueb-blueprint")) {
  console.warn("ueb-blueprint not registered; Blueprint class:", Blueprint);
}

ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
