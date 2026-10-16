# QA fixtures

Synthetic files are generated, never committed:

    cargo run -p ethnoflow-audio-engine --example gen_fixtures

| file | purpose |
|---|---|
| sine_a4_440hz_48k.wav | tuner / pitch reference, -6 dBFS |
| silence_1s_stereo_44k1.wav | silence handling, -∞ meters |
| clipped_220hz_48k.wav | clipping detection |
| sweep_50_8000hz_48k.wav | resampling / frequency response |
| synthetic_flute_d5_44k1.wav | breathy tone with vibrato for analysis tests (Phase 2) |

Real recordings (e.g. the user's kaval take `re.mp3`) are used only locally and with
permission. Never assume their key or mode; tonic/mode analysis arrives in Phase 3 and
must present probabilities and alternatives.
