import Bubble from "./windows/Bubble";
import History from "./windows/History";
import Settings from "./windows/Settings";

// Each Tauri window loads the same index.html but with a different hash.
// We route here so each window gets the right component.
export default function App() {
  const hash = window.location.hash;

  if (hash === "#bubble") return <Bubble />;
  if (hash === "#history") return <History />;
  if (hash === "#settings") return <Settings />;

  // Invisible root window — render nothing
  return null;
}
