import Bubble from "./windows/Bubble";
import History from "./windows/History";
import Onboarding from "./windows/Onboarding";
import Settings from "./windows/Settings";

// Each Tauri window loads the same index.html but with a different hash.
export default function App() {
  const hash = window.location.hash;

  if (hash === "#bubble") return <Bubble />;
  if (hash === "#history") return <History />;
  if (hash === "#onboard") return <Onboarding />;
  if (hash === "#settings") return <Settings />;

  return null;
}
