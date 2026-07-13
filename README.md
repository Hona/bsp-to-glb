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
- compiled face polygons and referenced primitive triangulation
- compiled vertex normals
- texture UVs and material names
- direct LDR/HDR face and lighting-lump pair selection
- exact per-face lightmap UVs from compiled vectors, mins and extents
- lossless flat and three-channel directional bump-lightmap atlases
- versioned lightmap manifests preserving face identity, styles and source offsets
- hidden-but-preserved sky, trigger and disabled brush models
- LZMA-compressed BSP lumps

Unsupported domains are detected or reported explicitly:

- displacement geometry aborts export instead of being silently dropped
- multi-style lightmaps abort export instead of being silently flattened
- static and dynamic prop model assets
- VTF pixels and VMT shader behavior
- collision brushes and physics meshes
- PVS/leaf visibility data
- overlays, particles and animated materials

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
- zero rendered-bounds error
- zero winding mismatches
- maximum position error: 0.000427 Source units

The Blender output contains props and reconstructed VMF geometry outside this strict comparison.
Exact-triangle comparisons therefore measure compiled-BSP identity, not subjective visual quality.

## Build

```bash
cargo build --release
```

## Usage

```bash
bsp-to-glb \
  --bsp path/to/compiled.bsp \
  --out path/to/map.glb \
  --lightmap-set auto \
  --lightmap-atlas path/to/lightmap.png \
  --lightmap-manifest path/to/lightmaps.json
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

## Verification

```bash
cargo fmt --check
cargo test --release
cargo clippy --all-targets -- -D warnings

# Local benchmark fixture (not distributed)
cargo test --release --test hydrogen_benchmark -- --ignored --nocapture
```

Tests use synthetic BSP fixtures and do not include game assets.

The Hydrogen benchmark requires `jump_hydrogen_rc1_bmv.bsp` in the repository root or a path in
`HYDROGEN_BSP`. It requires exactly 9,135 lit faces and 4,529 bumped lit faces.

## Design Principles

- Compiled BSP is the authority for render geometry and model boundaries.
- Named brush entities are never flattened into worldspawn.
- Unsupported geometry fails closed.
- Render, collision and visibility data remain separate domains.
- Accuracy claims are scoped and machine-verifiable.
- No game assets or proprietary source excerpts are included.

## Roadmap

1. Displacements and overlays
2. Direct lightmap atlas generation, including directional bump channels (implemented for
   single-style brush faces)
3. Static prop game lumps and reusable model references
4. VMT/VTF material package integration
5. Collision brush and physics sidecars
6. Leaf/cluster/PVS sidecars
7. Versioned output manifests and runtime integration

## Acknowledgements

- [ValveSoftware/source-sdk-2013](https://github.com/ValveSoftware/source-sdk-2013) for the publicly
  available Source SDK and BSP definitions. Its own license applies to that repository.
- Public SDK lightmap references:
  [`bspfile.h`](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/public/bspfile.h)
  and [`bspflags.h`](https://github.com/ValveSoftware/source-sdk-2013/blob/master/src/public/bspflags.h).
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
