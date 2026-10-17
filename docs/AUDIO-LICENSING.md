# Audio licensing and attribution (F1A.8)

Rules for every sound that ships inside EthnoFlow Studio or is suggested to users.
The machine-readable record is `assets/asset-manifest.json`; the human-readable one is
`assets/ATTRIBUTION.md`. A test (`crates/project/src/licensing.rs`) fails the build
when an audio file in the repository's shipped folders has no manifest entry, when an
entry's licence is not allowed, or when a removed asset is still on disk.

## What is allowed in the installer

| Licence | Allowed | Conditions |
|---|---|---|
| Original (synthesized in code, or recorded by the project for the project) | yes | `creator` and `origin` filled in; the recording's author agrees in writing (a line in ATTRIBUTION.md) |
| CC0 1.0 | yes | **only with documented provenance**: the exact source URL, the uploader, the download date and the SHA-256 of the file as downloaded. "Found as free" is not provenance. |
| CC BY 4.0 / 3.0 | yes | attribution text exactly as the author asks, in ATTRIBUTION.md and in the app's "About / Licences" panel; no suggestion of endorsement |
| CC BY-SA | no | share-alike would reach the project's own assets |
| CC BY-NC, CC BY-ND, "free for personal use", unknown | **no** | not compatible with distribution of a commercial product |
| Sample packs with their own EULA | case by case | only after reading the EULA; the EULA text is kept next to the manifest entry |

Sources the specification names as **inspiration only** (interaction patterns, feature
ideas), never as a source of code, UI or recordings: myNoise.net, Sonic Pi, Freesound
as a platform. Individual Freesound files may be used only under the CC0 / CC BY rules
above, with the uploader's own licence line copied verbatim; respect the platform's
API terms and each uploader's stated preferences (including about model training).

## Phase 1A status

Every sound in 0.2.x–0.3.x is **synthesized at run time** (noise colours, wind,
resonator textures, FM/modal bells, Karplus–Strong strings, the drone and the pad).
No recorded audio is bundled. The manifest therefore lists no files; the test still
scans the shipped folders so the first bundled file cannot slip in unrecorded.

User recordings never enter the installer. A user's own take used as a loop layer is
referenced by its take id inside the project; `license_ref` on the layer is free text
the user may fill in when the recording is not their own.

## Adding an asset

1. Put the file under `assets/audio/` (or the Tauri `resources` folder if it must be
   loaded at run time).
2. Add an entry to `assets/asset-manifest.json`:
   `{"path", "sha256", "license", "creator", "origin", "attribution", "added", "status": "active"}`.
3. Add the attribution line to `assets/ATTRIBUTION.md`.
4. Run `cargo test -p ethnoflow-project licensing`.

## Removing a questionable asset

1. Delete the file.
2. Keep its manifest entry and set `"status": "removed"` with a `"removed_reason"`.
   The test fails if a removed asset's file is still present, so a release cannot
   ship it by accident. The entry stays as a record of what was distributed before.
3. Note the removal in `docs/CHANGELOG.md`.
