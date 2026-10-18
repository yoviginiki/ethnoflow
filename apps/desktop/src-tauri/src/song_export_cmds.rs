//! Song export (P2 Phase F start; fixes the audit's "export omits the background"): one
//! offline graph with the arrangement's clips, the drone, the pad, the atmosphere rack
//! (with its automation and scenes) and the rhythm section, rendered through the same
//! buses, master space and limiter as live playback. Writes the mix, one aligned stem per
//! group (same start, same length), a `markers.csv` with the song map's sections and a
//! `manifest.json` for the DAW.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::{
    write_wav, AudioBuffer, ClipSource, ClipsParams, DroneParams, DroneSource, Graph, GraphParams,
    PadParams, PadSource, WavFormat,
};
use project::{effective_rack, LayerSource};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::ambient_cmds::{
    apply_layer_strip, apply_source, offline_sources, pitch_set, LiveParams, AMBIENT_SLOT0,
};
use crate::arrangement_cmds::{apply_tracks, build_clips, ARRANGE_SLOT};
use crate::bus_cmds::{apply_bus_strip, bus_gain_offline, BUS_DRONE, BUS_PAD, BUS_RHYTHM};
use crate::commands::{err, CmdResult};
use crate::graph_cmds::{DroneSettings, DRONE_SLOT, PAD_SLOT};
use crate::pad_cmds::PadSettings;
use crate::rhythm_cmds::{
    apply_params, extra_stem_name, offline_parts, RhythmSettings, SectionContext, ARP_SLOT,
    BASS_SLOT, DRUMS_SLOT, EXTRA_SLOT0, MAX_EXTRAS, OSTINATO_SLOT, PART_NAMES,
};
use crate::state::{AppState, Inner};

#[derive(Debug, Clone, Deserialize)]
pub struct SongExportInclude {
    #[serde(default = "yes")]
    pub clips: bool,
    pub drone: Option<DroneSettings>,
    pub pad: Option<PadSettings>,
    #[serde(default)]
    pub ambient: bool,
    pub rhythm: Option<RhythmSettings>,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct SongExportResult {
    pub path: String,
    pub stems: Vec<String>,
    pub markers: Option<String>,
    pub manifest: String,
    pub seconds: f64,
    pub sample_rate: u32,
    pub peak: f32,
    pub bytes: u64,
}

#[tauri::command]
pub async fn export_song(
    state: State<'_, AppState>,
    path: String,
    include: SongExportInclude,
    format: Option<WavFormat>,
    sample_rate: Option<u32>,
    seconds: Option<f64>,
    stems: Option<bool>,
) -> CmdResult<SongExportResult> {
    let rate = sample_rate.unwrap_or(48_000);
    if !(8_000..=192_000).contains(&rate) {
        return Err(err("invalid_argument", "sample rate outside 8..192 kHz"));
    }
    let want_stems = stems.unwrap_or(true);
    let gp = Arc::new(GraphParams::default());
    let mut pending: Vec<(usize, Box<dyn audio_engine::Source>)> = Vec::new();
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();

    // ---- everything that needs the app state, under one lock ----
    let (duration, ambient_pack, rhythm_live, markers_rows, manifest_json) = {
        let mut s = state.lock();
        let p = s
            .project
            .as_ref()
            .ok_or_else(|| err("no_project", "No project is open"))?;
        let manifest = p.manifest().clone();
        let song_map = crate::project_cmds::derived_song_map(&manifest);
        let mut duration = seconds.filter(|x| *x > 0.0).unwrap_or(0.0);

        // Clips.
        if include.clips {
            if let Ok(a) = crate::arrangement_cmds::arrangement_mut(&mut s).map(|a| a.clone()) {
                if a.duration() > 0.0 {
                    let clips = build_clips(&mut s, &a, rate)?;
                    let params = Arc::new(ClipsParams::default());
                    apply_tracks(&params, &a, rate);
                    params.transport.loop_on.store(false, Ordering::Relaxed);
                    params.transport.playing.store(true, Ordering::Relaxed);
                    pending.push((ARRANGE_SLOT, Box::new(ClipSource::new(params, clips))));
                    gp.layers[ARRANGE_SLOT].gain_db.set(0.0);
                    duration = duration.max(a.duration());
                    groups.push(("clips".to_string(), vec![ARRANGE_SLOT]));
                }
            }
        }
        // Drone.
        if let Some(set) = include.drone.as_ref() {
            let r = crate::graph_cmds::resolve(&s, set);
            let params = Arc::new(DroneParams::default());
            crate::graph_cmds::apply(&params, &gp.layers[DRONE_SLOT], set, &r);
            gp.layers[DRONE_SLOT]
                .gain_db
                .set(bus_gain_offline(&s, BUS_DRONE, set.level_db));
            apply_bus_strip(&s, BUS_DRONE, &gp.layers[DRONE_SLOT], 0.0, 20_000.0, 10.0);
            pending.push((DRONE_SLOT, Box::new(DroneSource::new(params))));
            groups.push(("drone".to_string(), vec![DRONE_SLOT]));
        }
        // Pad.
        if let Some(set) = include.pad.as_ref() {
            let params = Arc::new(PadParams::default());
            let _ = crate::pad_cmds::apply(&s, &params, set);
            gp.layers[PAD_SLOT]
                .gain_db
                .set(bus_gain_offline(&s, BUS_PAD, set.level_db));
            apply_bus_strip(&s, BUS_PAD, &gp.layers[PAD_SLOT], 0.0, 20_000.0, 10.0);
            pending.push((
                PAD_SLOT,
                Box::new(PadSource::new(params).with_sample_rate(rate)),
            ));
            groups.push(("pad".to_string(), vec![PAD_SLOT]));
        }
        // Atmosphere (rack + its program): sources now, parameters per block below.
        let ambient_pack = if include.ambient {
            let a = s.ambient.clone().filter(|a| !a.layers.is_empty());
            match a {
                Some(a) => {
                    let sources = offline_sources(&mut s, &a, rate)?;
                    let pitch_sets: Vec<Vec<f32>> = a
                        .layers
                        .iter()
                        .map(|l| match &l.source {
                            LayerSource::Texture {
                                tonal, register, ..
                            }
                            | LayerSource::Bells {
                                tonal, register, ..
                            }
                            | LayerSource::Pluck {
                                tonal, register, ..
                            }
                            | LayerSource::Resonator {
                                tonal, register, ..
                            }
                            | LayerSource::Choir {
                                tonal, register, ..
                            }
                            | LayerSource::Bloom {
                                tonal, register, ..
                            } => pitch_set(&s, *tonal, *register, l.seed),
                            _ => Vec::new(),
                        })
                        .collect();
                    let mut live = Vec::new();
                    let mut slots = Vec::new();
                    for (i, (params, source)) in sources.into_iter().enumerate() {
                        pending.push((AMBIENT_SLOT0 + i, source));
                        live.push(params);
                        slots.push(AMBIENT_SLOT0 + i);
                    }
                    groups.push(("atmosphere".to_string(), slots));
                    let end = a.program.end();
                    if end > 0.0 {
                        duration = duration.max(end);
                    }
                    Some((a, live, pitch_sets))
                }
                None => None,
            }
        } else {
            None
        };
        // Rhythm.
        let rhythm_live = if let Some(set) = include.rhythm.as_ref() {
            let (bpm, meter) = crate::rhythm_cmds::timing(&s)?;
            let (root, pcs) = crate::rhythm_cmds::chord_pcs(&s, set)?;
            let ctx = SectionContext {
                bpm,
                meter: &meter,
                root,
                pcs: &pcs,
            };
            let (live, sources) = offline_parts(&s, set, &ctx)?;
            let lvl = |db: f32| {
                let own = if db <= -39.5 { -120.0 } else { db };
                bus_gain_offline(&s, BUS_RHYTHM, own)
            };
            apply_params(&gp.layers, &live, &lvl);
            for slot in DRUMS_SLOT..audio_engine::MAX_LAYERS {
                apply_bus_strip(&s, BUS_RHYTHM, &gp.layers[slot], 0.0, 20_000.0, 10.0);
            }
            pending.extend(sources);
            for (i, slot) in [DRUMS_SLOT, BASS_SLOT, OSTINATO_SLOT, ARP_SLOT]
                .into_iter()
                .enumerate()
            {
                let on = [set.drums.on, set.bass.on, set.ostinato.on, set.arp.on][i];
                if on {
                    groups.push((PART_NAMES[i].to_string(), vec![slot]));
                }
            }
            for (i, part) in set.extras.iter().enumerate().take(MAX_EXTRAS) {
                if part.on {
                    groups.push((extra_stem_name(part, i), vec![EXTRA_SLOT0 + i]));
                }
            }
            Some(live)
        } else {
            None
        };
        if duration <= 0.0 {
            let end = song_map.tick_to_seconds(song_map.end_tick());
            duration = if end > 0.0 { end } else { 60.0 };
        }
        duration = duration.min(4.0 * 3600.0);
        gp.master.gain_db.set(s.master_db.unwrap_or(0.0).min(0.0));
        // Markers and manifest from the song map (seconds derived from its tempo map).
        let markers_rows: Vec<(f64, u64, String, String)> = song_map
            .sections
            .iter()
            .map(|sec| {
                (
                    song_map.tick_to_seconds(sec.start_tick),
                    song_map.tick_to_bar_beat(sec.start_tick).0,
                    sec.name.clone(),
                    sec.role.clone(),
                )
            })
            .collect();
        let manifest_json = serde_json::json!({
            "app": "EthnoFlow Studio",
            "project": manifest.name,
            "sample_rate": rate,
            "format": format.unwrap_or_default(),
            "seconds": duration,
            "bar_offset_seconds": 0.0,
            "tempo_map": song_map.tempo_map,
            "meter_map": song_map.meter_map,
            "sections": song_map.sections,
            "stems": groups.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        });
        (
            duration,
            ambient_pack,
            rhythm_live,
            markers_rows,
            manifest_json,
        )
    };
    if groups.is_empty() {
        return Err(err(
            "empty",
            "Nothing to export: no clips and no background layers chosen",
        ));
    }
    // ---- render ----
    let frames = (duration * rate as f64).round() as usize;
    let ch = 2usize;
    let mut out = vec![0.0f32; frames * ch];
    let mut stem_bufs: Vec<Vec<f32>> = if want_stems {
        (0..audio_engine::MAX_LAYERS)
            .map(|_| vec![0.0f32; frames * ch])
            .collect()
    } else {
        Vec::new()
    };
    let offline = Inner::default();
    let apply_ambient = |done: usize| {
        let Some((a, live, pitch_sets)) = ambient_pack.as_ref() else {
            return;
        };
        let t = done as f64 / rate as f64;
        let program_end = a.program.end();
        let program_t = if program_end > 0.0 {
            Some(a.program.wrap(t).min(program_end))
        } else {
            None
        };
        let (layers, macros) = effective_rack(a, program_t, t);
        let any_solo = layers.iter().any(|l| l.solo);
        for (i, l) in layers.iter().enumerate() {
            if let Some(p) = live.get(i) {
                apply_source(&offline, p, l, &macros);
                match p {
                    LiveParams::Texture(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Bells(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Pluck(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Resonator(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Choir(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Bloom(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Loop(x) => {
                        if let LayerSource::Loop {
                            start,
                            end,
                            crossfade,
                            ..
                        } = &l.source
                        {
                            x.start_frames
                                .store((start * rate as f64).round() as u64, Ordering::Relaxed);
                            x.end_frames
                                .store((end * rate as f64).round() as u64, Ordering::Relaxed);
                            x.crossfade_frames
                                .store((crossfade * rate as f64).round() as u32, Ordering::Relaxed);
                        }
                    }
                    _ => {}
                }
            }
            apply_layer_strip(
                &gp.layers[AMBIENT_SLOT0 + i],
                l,
                &macros,
                any_solo,
                &offline,
            );
        }
        gp.master.space.set(macros.space as f32);
        gp.master.space_size.set(macros.space_size as f32);
        gp.master.shimmer.set(macros.shimmer as f32);
        gp.master.shimmer_st.set(macros.shimmer_st as f32);
    };
    // Every parameter is at its first state before the graph is built: no ramp-in.
    apply_ambient(0);
    let mut g = Graph::new(Arc::clone(&gp), rate);
    for (slot, src) in pending {
        g.set_source(slot, src);
    }
    if let Some(live) = rhythm_live.as_ref() {
        live.clock.start(0, 0.0);
    }
    let block = 512usize;
    let mut done = 0usize;
    while done < frames {
        let n = (frames - done).min(block);
        apply_ambient(done);
        let slice = &mut out[done * ch..(done + n) * ch];
        if want_stems {
            let mut parts: Vec<Vec<f32>> = (0..audio_engine::MAX_LAYERS)
                .map(|_| vec![0.0f32; n * ch])
                .collect();
            g.render_with_stems(slice, ch, &mut parts);
            for (sb, part) in stem_bufs.iter_mut().zip(parts.iter()) {
                sb[done * ch..(done + n) * ch].copy_from_slice(part);
            }
        } else {
            g.render(slice, ch);
        }
        done += n;
    }

    // ---- files ----
    let mut p = std::path::PathBuf::from(&path);
    if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("wav")) {
        p.set_extension("wav");
    }
    let format = format.unwrap_or_default();
    let mix = AudioBuffer::new(rate, ch as u16, out);
    write_wav(&p, &mix, format)?;
    let dir = p.with_extension("");
    std::fs::create_dir_all(&dir).map_err(|e| err("io", e.to_string()))?;
    let mut stem_paths = Vec::new();
    if want_stems {
        for (name, slots) in &groups {
            let mut sum = vec![0.0f32; frames * ch];
            for slot in slots {
                for (k, v) in stem_bufs[*slot].iter().enumerate() {
                    sum[k] += v;
                }
            }
            let sp = dir.join(format!("{name}.wav"));
            write_wav(&sp, &AudioBuffer::new(rate, ch as u16, sum), format)?;
            stem_paths.push(sp.display().to_string());
        }
    }
    let markers = if markers_rows.is_empty() {
        None
    } else {
        let mut csv = String::from("seconds,bar,name,role\n");
        for (sec, bar, name, role) in &markers_rows {
            csv.push_str(&format!(
                "{sec:.3},{bar},{},{role}\n",
                name.replace(',', " ")
            ));
        }
        let mp = dir.join("markers.csv");
        std::fs::write(&mp, csv).map_err(|e| err("io", e.to_string()))?;
        Some(mp.display().to_string())
    };
    let manp = dir.join("manifest.json");
    std::fs::write(
        &manp,
        serde_json::to_string_pretty(&manifest_json).map_err(|e| err("io", e.to_string()))?,
    )
    .map_err(|e| err("io", e.to_string()))?;
    let peak = mix.samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
    Ok(SongExportResult {
        path: p.display().to_string(),
        stems: stem_paths,
        markers,
        manifest: manp.display().to_string(),
        seconds: duration,
        sample_rate: rate,
        peak,
        bytes,
    })
}
