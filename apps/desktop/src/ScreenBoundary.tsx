import { Component, type ErrorInfo, type ReactNode } from "react";
import { messages } from "./i18n";
import { useStore, type Screen } from "./store";

interface Props {
  screen: Screen;
  children: ReactNode;
}
interface State {
  error: string | null;
}

/**
 * A screen that throws while rendering must not take the whole app down (2026-10-31:
 * a one-frame inconsistency in the tuner unmounted everything, including the assistant
 * bridge, and the window went blank). The boundary shows the error and a way home; it
 * resets when the screen changes.
 */
export class ScreenBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(e: unknown): State {
    return { error: e instanceof Error ? `${e.name}: ${e.message}` : String(e) };
  }

  componentDidCatch(e: unknown, info: ErrorInfo) {
    console.error("screen crashed", this.props.screen, e, info.componentStack);
  }

  componentDidUpdate(prev: Props) {
    if (prev.screen !== this.props.screen && this.state.error !== null) this.setState({ error: null });
  }

  render() {
    if (this.state.error === null) return this.props.children;
    const st = useStore.getState();
    const t = messages[st.prefs.lang];
    return (
      <section className="panel screen-crash" role="alert">
        <h2>{t.screenCrashed}</h2>
        <p className="muted">{t.screenCrashedHint}</p>
        <pre className="small">{this.state.error}</pre>
        <div className="row">
          <button onClick={() => this.setState({ error: null })}>{t.screenCrashedRetry}</button>
          <button className="ghost" onClick={() => st.setScreen("home")}>
            {t.screenCrashedHome}
          </button>
        </div>
      </section>
    );
  }
}
