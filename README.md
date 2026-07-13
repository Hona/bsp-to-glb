# bsp-to-glb

Experimental, first-principles Rust exporter for compiled Source 1 BSP brush geometry.

The project reads compiled BSP data directly and writes glTF 2.0 binary (`.glb`) without
decompiling to VMF or routing world geometry through Blender.

## Benchmark TL;DR

Direct compiled-BSP export is approximately **119x faster** while exactly preserving the supported
compiled brush-geometry domain.

| Export path | Warm median | Output size |
|---|---:|---:|
| Direct Rust BSP export | 233.5 ms | 3.46 MB |
| Blender/Plumber export | 27.84 s | 23.06 MB |

## Status

This is not yet a complete Source renderer or a drop-in replacement for the full map asset
pipeline. It is currently accurate for the supported compiled brush-rendering domain:

- BSP models and entity/model relationships
- entity origins, angles, names, classes and initial-state metadata
- TF2 static prop `GAME_LUMP` metadata and reusable MDL path references
- dynamic prop entity references and ordered key/value state metadata
- compiled face polygons and referenced primitive triangulation
- compiled vertex normals
- compiled displacement grids, vector distances, alpha and triangle tags
- generated displacement normals and Source displacement triangulation
- texture UVs and material names
- versioned Source material manifests with PAK-first VMT/VTF provenance
- VMT shader-family inputs and common render flags
- bounded BSP PAK parsing for embedded VMT/VTF resources
- bounded VTF 7.0-7.5 parsing and selected mip/frame/face decoding to lossless RGBA PNG
- content-addressed material texture packages with decoded-pixel deduplication
- lightmap UVs supplied by existing atlas metadata
- direct LDR/HDR face and lighting-lump pair selection
- exact per-face lightmap UVs from compiled vectors, mins and extents
- lossless flat and three-channel directional bump-lightmap atlases
- versioned lightmap manifests preserving face identity, styles and source offsets
- hidden-but-preserved sky, trigger and disabled brush models
- versioned direct-BSP collision sidecars with brush-model ownership
- static-prop collision identity and solid-mode metadata when `sprp` is present
- raw per-model PHYSCOLLIDE blocks and metadata (opaque, explicitly not decoded)
- LZMA-compressed BSP lumps
- exact leaf/cluster/PVS visibility sidecars with GLB chunk ownership

Unsupported domains are detected or reported explicitly:

- multi-style lightmaps abort export instead of being silently flattened
- static and dynamic prop MDL geometry resolution
- full Source shader execution and material proxy evaluation
- material proxies and animated materials (identified as metadata only)
- decoded VPhysics collision meshes
- overlays and water overlays (presence and lump versions are reported, geometry is not exported)
- cubemap samples (presence and lump versions are reported, textures are not exported)
- particles

Do not describe output as full Source parity until those domains are implemented and tested.

## Why

The initial integration target is the map conversion pipeline in
[`Hona/dribble.tf`](https://github.com/Hona/dribble.tf):

- [`convert-tempus-map.mjs`](https://github.com/Hona/dribble.tf/blob/dev/scripts/convert-tempus-map.mjs)
- [`plumber_import_vmf.py`](https://github.com/Hona/dribble.tf/blob/dev/scripts/plumber_import_vmf.py)
- [`chunk_map_glb.py`](https://github.com/Hona/dribble.tf/blob/dev/scripts/chunk_map_glb.py)
- [`inject-glb-lightmaps.mjs`](https://github.com/Hona/dribble.tf/blob/dev/scripts/inject-glb-lightmaps.mjs)

The goal is to replace the Blender/VMF route for compiled world and brush geometry. It is not
intended to replace Blender or Plumber for model, texture, armature, animation and authoring tasks
where those tools remain valuable.

Direct BSP export has two important benefits:

1. Compiled geometry, topology, normals, model identity and lightmap ownership remain authoritative.
2. Export avoids Blender startup, scene construction and Python glTF serialization costs.

## Benchmark

Reference map: `jump_hydrogen_rc1_bmv` (BSP v20, no displacements).

| Export path | Warm median | Output size |
|---|---:|---:|
| Direct Rust BSP export | 233.5 ms | 3.46 MB |
| Blender/Plumber export | 27.84 s | 23.06 MB |

Measured speedup: `119x`.

Strict supported-domain checks for that map:

- 9,136/9,136 initially rendered brush faces
- 24,475/24,475 initially rendered brush triangles
- 978/978 compiled primitive face records represented
- 57,299/57,299 compiled vertex normals exact
- 9,135/9,135 eligible lightmapped faces, zero false positives
- 151/151 BSP model identities and transforms
- 104/104 named brush entities
- 235/235 TF2 `sprp` v10 static prop identities
- 73/73 solid static props
- zero rendered-bounds error
- zero winding mismatches
- maximum position error: 0.000427 Source units

The Blender output contains props and reconstructed VMF geometry outside this strict comparison.
Exact-triangle comparisons therefore measure compiled-BSP identity, not subjective visual quality.

## Releases

Tags named `vMAJOR.MINOR.PATCH` publish tested Windows x64 and Linux x64 archives. The tag must match
the Cargo package version. Every release includes deterministic archive names, `SHA256SUMS`, a
human-readable [`CAPABILITIES.md`](CAPABILITIES.md), target-specific build metadata, and generated
release notes. Archives also contain the README, license, capability snapshot, and
`build-metadata.json`.

Verify an archive against a digest pinned by the consuming repository before extraction. See
[`docs/DRIBBLE_RELEASES.md`](docs/DRIBBLE_RELEASES.md) for the dribble.tf pin, download, checksum,
and metadata-validation contract.

## Build

```bash
cargo build --release
```

## Usage

Inspect the package version or machine-readable build metadata without a BSP input:

```bash
bsp-to-glb --version
bsp-to-glb --version-json
```

The JSON includes schema version, package version, build target/profile, release source commit, and
the release's supported, detected-only, and unsupported capability states.

```bash
bsp-to-glb \
  --bsp path/to/compiled.bsp \
  --out path/to/map.glb \
  --lightmap-set auto \
  --lightmap-atlas path/to/lightmap.png \
  --lightmap-manifest path/to/lightmaps.json \
  --material-manifest path/to/map.materials.json \
  --texture-output path/to/textures \
  --texture-manifest path/to/textures/manifest.json \
  --collision-out path/to/map.collision.json \
  --props-out path/to/props.json \
  --visibility-out path/to/map.visibility.json
```

`--lightmap-set` accepts `auto`, `ldr`, `hdr`, or `none`. `auto` prefers a complete HDR
face/lighting pair and falls back to a complete LDR pair. Explicit `ldr` and `hdr` selections fail
if either half of the requested pair is absent or has an unsupported lump version.

`--lightmap-atlas` writes the flat atlas at the requested PNG path and directional atlases beside
it as `.bump-0.png`, `.bump-1.png`, and `.bump-2.png`. The PNG RGBA bytes preserve Source
`ColorRGBExp32` samples losslessly: RGB contains the mantissa and alpha contains the signed
two's-complement exponent. The manifest identifies this as linear data and records the decode
formula, channel semantics, source pair, styles, face indices, offsets, extents, and atlas regions.
These are raw data PNGs, not directly displayable sRGB images.

The default maximum atlas row width is 4096 pixels and can be changed with `--atlas-width`.

The legacy `--lightmaps path/to/lightmap_data.json` input remains available for pipeline-produced
atlas metadata. It is mutually exclusive with direct atlas output options.

`--material-manifest` is optional. It writes schema version 2 with the original BSP material name,
canonical Source lookup paths, shader-family metadata, embedded resource inventory, per-resource
provenance, optional texture-package source indices and unresolved assets. Embedded PAK resources
always win over an external resolver.

## Material Resolution

The library exposes `MaterialResolver` and `export_bsp_with_material_resolver` for callers that can
provide resources from an installation or another asset store. A resolver receives canonical paths
such as `materials/brick/wall.vtf` and must return real bytes plus a stable provenance label. The
exporter does not include a game-asset resolver and does not emit placeholder textures.

VMT parsing records shader inputs for unlit, translucency, additive blending, alpha test, no-cull,
base texture, bump/SSBump, detail, self-illumination, envmap and surface properties. Proxies and
animated materials are retained as explicit unsupported metadata rather than represented as glTF
parity.

`--texture-output` opts into VTF decoding and writes one PNG per unique decoded image. Output names
are `sha256-<PNG digest>.png`; textures with identical dimensions and RGBA pixels share one output.
`--texture-manifest` optionally writes the `bsp-to-glb/material-textures` version 1 manifest beside
that package. `--texture-mip`, `--texture-frame`, and `--texture-face` select the image, defaulting
to zero. VTF mip zero is the full-resolution image. Material texture conversion is also available
through `VtfImageSelection`, `decode_vtf`, `inspect_vtf`, `build_source_material_package`, and
`ExportOptions::material_texture_selection`.

The decoder supports VTF 7.0 through 7.5 and RGBA8888, ABGR8888, RGB888, BGR888, BGRA8888, DXT1,
DXT1 one-bit alpha, DXT3, DXT5, I8, IA88 and A8. It handles block edges without expanding output
dimensions and follows VTF's smallest-to-largest mip storage. Unsupported image formats and volume
textures produce explicit `unsupported` package-source records with format metadata. Malformed or
truncated inputs produce `invalid` records. Neither case emits placeholder pixels or aborts other
texture conversions.

PAK parsing only exposes `materials/**/*.vmt` and `materials/**/*.vtf`. It rejects traversal,
case-insensitive duplicate paths, oversized entries and malformed ZIP data, and applies bounded
entry and decompression limits. VTF files, encoded image ranges and decoded RGBA output are each
bounded to 256 MiB; dimensions are bounded to 16,384 and resource dictionaries to 4,096 entries.

`--out` and `--collision-out` are independently optional, but at least one is required. A
collision-only export does not parse or triangulate render faces. Material, prop, lightmap and
visibility outputs require `--out`; visibility references the emitted GLB chunk indices.

## Collision Sidecar

Collision output is JSON with schema `bsp-to-glb/collision`, version `1`. It preserves Source-space
planes, brush sides, brushes, leaf-brush references, leaf and brush contents, and model ownership
derived from each BSP model's head node. `CONTENTS_PLAYERCLIP` remains present in the numeric
contents mask and is also exposed as `playerClip` for consumers.

The sidecar declares `geometrySource: "bspBrushes"` and
`renderTriangleSubstitution: false`; render triangles are never used as collision fallback.
PHYSCOLLIDE model headers and raw blocks are retained as base64, while `decodeStatus` is
`"unsupported"` until a compatible VPhysics decoder is implemented.

Static-prop collision metadata is modular library input through `CollisionExportInput`. `None`
means GAME_LUMP data was unavailable; `Some` preserves supplied prop indices, model names and
solid modes. The CLI parses supported `sprp` layouts and supplies this input automatically; an
absent `sprp` remains distinguishable from an empty parsed list.

`--props-out` is optional. Prop metadata is always embedded under
`asset.extras.props` and on reference-only GLB nodes; this flag also writes the same
`bsp-to-glb.props` schema as a versioned JSON sidecar. Static prop nodes preserve dictionary/model
identity, transforms, leaf membership, skin, solidity, flags, fade and lighting fields. Supported
later layouts preserve uniform scale. Dynamic prop entities remain separate nodes with their
original entity index and ordered key/value state. MDL paths are reusable asset references only;
the exporter reports model resolution as unsupported and never fabricates missing geometry.

`--visibility-out` is optional. It writes `bsp-to-glb.visibility` version 1 JSON. PVS rows and
face/chunk cluster memberships are flattened little-endian `u32` bitsets, with
`clusterWordCount = ceil(clusterCount / 32)`. Leaf memberships use compact offset/index arrays:
entry `n` occupies `indices[offsets[n]..offsets[n + 1]]`. World-face memberships come directly
from `LEAFFACES`; bounds are not sampled. Chunks for non-world brush models have `staticPvs=false`
and remain runtime-controlled rather than being culled by the static world PVS.

The CLI statistics include a `capabilities` object. Displacements report `exported`; overlays,
water overlays and cubemaps report `detectedOnly`. Unknown optional-feature lump versions report
`unsupportedVersion` rather than implying support.

Compiled displacement export supports version 0 displacement lumps, powers 2 through 4,
quadrilateral parent faces, remove-tag filtering, alpha attributes and generated normals. Invalid
references, orphaned records, unsupported powers or unsupported displacement lump versions abort
the render export rather than dropping geometry.

## Verification

```bash
cargo fmt --check
cargo test --release
cargo clippy --all-targets -- -D warnings

HYDROGEN_BSP=/path/to/jump_hydrogen_rc1_bmv.bsp \
  cargo test --release --test hydrogen_collision -- --ignored
HYDROGEN_BSP=/path/to/jump_hydrogen_rc1_bmv.bsp \
  cargo test --release --test hydrogen_benchmark -- --ignored --nocapture
HYDROGEN_BSP=/path/to/jump_hydrogen_rc1_bmv.bsp \
  cargo test --release --test hydrogen_materials -- --ignored --nocapture
BSP_TO_GLB_HYDROGEN_BSP=/path/to/jump_hydrogen_rc1_bmv.bsp \
  cargo test --release --test hydrogen_props -- --nocapture
BSP_TO_GLB_HYDROGEN_BSP=/path/to/jump_hydrogen_rc1_bmv.bsp \
  cargo test --release --test hydrogen_visibility -- --ignored
```

Tests use synthetic BSP fixtures and do not include game assets. The Hydrogen acceptance tests use
a local map that is not distributed with this repository.

It verifies 3,511 brushes, 31,092 brush sides, 2,575 world-model brushes, 259 playerclip brushes,
151 model entries, collision ownership for zero-render model 147, and TF2 `sprp` v10 prop identity
and solidity. The direct lightmap gate additionally requires exactly 9,135 lit faces and 4,529
bumped lit faces. Visibility preserves all 450 PVS rows and 6,248 leaves; all 435 clusters owning
world render faces are represented by static GLB chunks.

## Design Principles

- Compiled BSP is the authority for render geometry and model boundaries.
- Named brush entities are never flattened into worldspawn.
- Unsupported geometry and displacement lump versions fail closed.
- Render, collision and visibility data remain separate domains.
- Accuracy claims are scoped and machine-verifiable.
- No game assets or proprietary source excerpts are included.

## Roadmap

1. Overlay projection and clipping
2. Direct lightmap atlas generation, including directional bump channels (implemented for
   single-style brush faces)
3. Static prop MDL geometry resolution (metadata and reusable references implemented)
4. VTF pixel conversion (implemented); runtime shader integration remains
5. Collision brush and opaque physics sidecars (implemented)
6. ~~Leaf/cluster/PVS sidecars~~
7. Versioned output manifests and runtime integration

## Acknowledgements

- [ValveSoftware/source-sdk-2013](https://github.com/ValveSoftware/source-sdk-2013) for the publicly
  available Source SDK and BSP definitions. Its own license applies to that repository.
- Public SDK lightmap references:
  [`bspfile.h`](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/public/bspfile.h)
  and [`bspflags.h`](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/public/bspflags.h).
- Public SDK VTF concepts and image-format identifiers from
  [`vtf.h`](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/public/vtf/vtf.h).
- [BSPSource](https://github.com/ata4/bspsrc) for extensive Source BSP tooling and research.
- [Plumber](https://github.com/lasa01/Plumber) for Source model, map, material and texture import
  tooling used by the pipeline this project is incrementally replacing.
- [Khronos glTF](https://www.khronos.org/gltf/) for the glTF specification.
- [Three.js](https://threejs.org/) for runtime GLB validation and rendering.
- The tf2jump.xyz and Source-jumping communities for maps, testing and parity references.

Team Fortress, Source and related names are trademarks of Valve Corporation. This project is not
affiliated with or endorsed by Valve.

## License

MIT. See [LICENSE](LICENSE).
