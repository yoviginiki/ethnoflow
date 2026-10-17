import { api, describeError, VOICES, type Voice } from "./api";
import { messages } from "./i18n";
import { useStore } from "./store";

interface Props {
  /** Reload the loaded take's waveform/analysis after a re-render. */
  onRerendered: () => Promise<void>;
}

/** Controls for a loaded MIDI take: tempo factor, muted tracks, synth voice. */
export function MidiPanel({ onRerendered }: Props) {
  const s = useStore();
  const t = messages[s.prefs.lang];
  const midi = s.midi;
  if (!midi) return null;
  const busy = s.mode !== "idle" || s.pending !== null || s.notesPlayer !== null;

  const apply = async (patch: Partial<typeof midi.render>) => {
    try {
      s.setError(null);
      const v = await api.setMidiRender({ ...midi.render, ...patch });
      s.setMidi(v);
      await onRerendered();
    } catch (e) {
      s.setError(describeError(e));
    }
  };

  return (
    <div className="panel midi-panel">
      <span className="badge">{t.midiFile}</span>
      <span className="small muted">
        {t.midiTempo}: <b className="mono-num">{midi.bpm.toFixed(1)} BPM</b>{" "}
        {midi.tempo_from_file ? `(${t.midiTempoFromFile})` : `(${t.midiTempoDefault})`}
        {midi.time_signature && (
          <>
            {" "}
            · {midi.time_signature[0]}/{midi.time_signature[1]}
          </>
        )}
      </span>
      <label className="inline">
        ×<span className="mono-num">{midi.render.tempo_factor.toFixed(2)}</span>
        <input
          type="range"
          min={0.25}
          max={2}
          step={0.05}
          value={midi.render.tempo_factor}
          disabled={busy}
          onChange={(e) => void apply({ tempo_factor: Number(e.target.value) })}
          aria-label={t.midiTempo}
        />
      </label>
      <label className="inline">
        {t.voice}
        <select value={midi.render.voice} disabled={busy} onChange={(e) => void apply({ voice: e.target.value as Voice })}>
          {VOICES.map((v) => (
            <option key={v} value={v}>
              {t.voices[v]}
            </option>
          ))}
        </select>
      </label>
      {midi.tracks.filter((tr) => tr.note_count > 0).length > 1 && (
        <span className="inline">
          <span className="small muted">{t.tracks}:</span>
          {midi.tracks
            .filter((tr) => tr.note_count > 0)
            .map((tr) => {
              const muted = midi.render.muted_tracks.includes(tr.index);
              return (
                <label key={tr.index} className="inline small">
                  <input
                    type="checkbox"
                    checked={!muted}
                    disabled={busy}
                    onChange={(e) =>
                      void apply({
                        muted_tracks: e.target.checked
                          ? midi.render.muted_tracks.filter((i) => i !== tr.index)
                          : [...midi.render.muted_tracks, tr.index],
                      })
                    }
                  />
                  {tr.name ?? `${t.track} ${tr.index + 1}`} ({tr.note_count})
                </label>
              );
            })}
        </span>
      )}
    </div>
  );
}
