import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, describeError, type RecentProject } from "./api";
import { messages } from "./i18n";
import { useStore } from "./store";

/**
 * "Project" menu in the top bar, present on every screen: new, open, recent, close.
 * Projects save themselves, so there is no Save item; the menu says so instead.
 */
export function ProjectMenu() {
  const t = messages[useStore((s) => s.prefs.lang)];
  const project = useStore((s) => s.project);
  const mode = useStore((s) => s.mode);
  const [show, setShow] = useState(false);
  const [recent, setRecent] = useState<RecentProject[]>([]);
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const st = () => useStore.getState();

  useEffect(() => {
    if (!show) return;
    api.recentProjects().then(setRecent).catch(() => setRecent([]));
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setShow(false);
        setNaming(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setShow(false);
        setNaming(false);
      }
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [show]);

  /** The backend stops everything on close; the store follows (layers.ts resets the switches). */
  const closeCurrent = async () => {
    if (!st().project) return;
    await api.closeProject();
    st().setLoaded(null, null);
    st().setProject(null, null);
    st().setMessage(null);
  };
  const run = async (label: string, fn: () => Promise<void>) => {
    setBusy(label);
    st().setError(null);
    try {
      await fn();
      setShow(false);
      setNaming(false);
    } catch (e) {
      st().setError(describeError(e));
    } finally {
      setBusy(null);
    }
  };
  const openAt = (path: string) =>
    run(t.opening, async () => {
      await closeCurrent();
      const r = await api.openProject(path);
      st().setProject(r.project, r.report);
    });
  const create = () =>
    run(t.creating, async () => {
      const n = name.trim();
      if (!n) return;
      await closeCurrent();
      st().setProject(await api.createProject(n, null), null);
      setName("");
    });
  const browse = async () => {
    const dir = await api.defaultProjectsDir().catch(() => "");
    const path = await open({ title: t.openProject, directory: true, multiple: false, defaultPath: dir || undefined });
    if (path && !Array.isArray(path)) await openAt(path);
  };
  const locked = mode !== "idle" || busy !== null;
  const others = recent.filter((r) => r.path !== project?.root).slice(0, 8);

  return (
    <div className="project-menu" ref={ref}>
      <button
        className="ghost project-menu-btn"
        aria-haspopup="menu"
        aria-expanded={show}
        title={project ? project.root : t.noProjectOpen}
        disabled={mode !== "idle"}
        onClick={() => setShow((v) => !v)}
      >
        📁 <strong>{project ? project.name : t.noProjectOpen}</strong> ▾
      </button>
      {show && (
        <div className="project-menu-pop panel" role="menu" aria-label={t.projectMenu}>
          {busy && (
            <span className="pending" role="status">
              {busy}
            </span>
          )}
          {naming ? (
            <form
              className="project-menu-new"
              onSubmit={(e) => {
                e.preventDefault();
                void create();
              }}
            >
              <input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder={t.projectName} maxLength={120} aria-label={t.projectName} />
              <button type="submit" disabled={!name.trim() || locked}>
                {t.create}
              </button>
            </form>
          ) : (
            <button className="ghost" role="menuitem" disabled={locked} onClick={() => setNaming(true)}>
              {t.newProject}…
            </button>
          )}
          <button className="ghost" role="menuitem" disabled={locked} onClick={() => void browse()}>
            {t.openProject}
          </button>
          <div className="project-menu-sep" />
          <div className="muted small">{t.recentProjects}</div>
          {others.length === 0 && <div className="muted small">{t.noRecent}</div>}
          {others.map((r) => (
            <button key={r.path} className="ghost" role="menuitem" disabled={locked || !r.exists} title={r.path} onClick={() => void openAt(r.path)}>
              {r.name}
              {!r.exists && <span className="badge warn"> {t.projectMissing}</span>}
            </button>
          ))}
          {project && (
            <>
              <div className="project-menu-sep" />
              <div className="muted small">✓ {t.projectSaved}</div>
              <div className="muted small path">{project.root}</div>
              <button className="ghost" role="menuitem" disabled={locked} onClick={() => void run(t.saving, closeCurrent)}>
                {t.closeProject}
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
