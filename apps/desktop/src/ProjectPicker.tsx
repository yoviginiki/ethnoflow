import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, describeError, type RecentProject } from "./api";
import { messages } from "./i18n";
import { useStore } from "./store";

export function ProjectPicker() {
  const t = messages[useStore((s) => s.prefs.lang)];
  const setProject = useStore((s) => s.setProject);
  const setError = useStore((s) => s.setError);
  const error = useStore((s) => s.error);
  const [name, setName] = useState("");
  const [recent, setRecent] = useState<RecentProject[]>([]);
  const [dir, setDir] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.recentProjects().then(setRecent).catch((e) => setError(describeError(e)));
    api.defaultProjectsDir().then(setDir).catch(() => setDir(""));
  }, [setError]);

  const openAt = async (path: string) => {
    setBusy(true);
    try {
      setError(null);
      const r = await api.openProject(path);
      setProject(r.project, r.report);
    } catch (e) {
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };

  const onCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setBusy(true);
    try {
      setError(null);
      setProject(await api.createProject(name.trim(), null), null);
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  };

  const onBrowse = async () => {
    const path = await open({ title: t.openProject, directory: true, multiple: false, defaultPath: dir || undefined });
    if (path && !Array.isArray(path)) await openAt(path);
  };

  return (
    <section className="picker">
      <form className="panel" onSubmit={onCreate}>
        <h2>{t.newProject}</h2>
        <label>
          {t.projectName}
          <input value={name} onChange={(e) => setName(e.target.value)} maxLength={120} autoFocus />
        </label>
        {dir && (
          <div className="muted small">
            {t.savedIn}: {dir}
          </div>
        )}
        <button type="submit" disabled={busy || !name.trim()}>
          {t.create}
        </button>
      </form>

      <div className="panel">
        <h2>{t.recentProjects}</h2>
        {recent.length === 0 && <div className="muted">{t.noRecent}</div>}
        <ul className="recent">
          {recent.map((r) => (
            <li key={r.path}>
              <button className="ghost" disabled={busy || !r.exists} onClick={() => openAt(r.path)} title={r.path}>
                {r.name}
                {!r.exists && <span className="badge warn"> {t.projectMissing}</span>}
              </button>
              <span className="muted small path">{r.path}</span>
            </li>
          ))}
        </ul>
        <button onClick={onBrowse} disabled={busy}>
          {t.openProject}
        </button>
      </div>

      {error && (
        <div className="banner error" role="alert">
          {t.errorPrefix}: {error}
        </div>
      )}
    </section>
  );
}
