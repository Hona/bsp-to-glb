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
- texture UVs and material names
- lightmap UVs supplied by existing atlas metadata
- hidden-but-preserved sky, trigger and disabled brush models
- LZMA-compressed BSP lumps

Unsupported domains are detected or reported explicitly:

- displacement geometry aborts export instead of being silently dropped
- static and dynamic prop MDL geometry resolution
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
- 235/235 TF2 `sprp` v10 static prop identities
- 73/73 solid static props
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
  --lightmaps path/to/lightmap_data.json \
  --props-out path/to/props.json
```

`--lightmaps` is optional. The current input format is produced by the tf2jump map pipeline and
will be replaced by direct atlas generation as the exporter matures.

`--props-out` is optional. Prop metadata is always embedded under
`asset.extras.props` and on reference-only GLB nodes; this flag also writes the same
`bsp-to-glb.props` schema as a versioned JSON sidecar. Static prop nodes preserve dictionary/model
identity, transforms, leaf membership, skin, solidity, flags, fade and lighting fields. Supported
later layouts preserve uniform scale. Dynamic prop entities remain separate nodes with their
original entity index and ordered key/value state. MDL paths are reusable asset references only;
the exporter reports model resolution as unsupported and never fabricates missing geometry.

## Verification

```bash
cargo fmt --check
cargo test --release
cargo clippy --all-targets -- -D warnings
```

Tests use synthetic BSP fixtures and do not include game assets.

Set `BSP_TO_GLB_HYDROGEN_BSP` to a local `jump_hydrogen_rc1_bmv.bsp` to run the gated
TF2 `sprp` v10 identity, solidity and timing benchmark during `cargo test`.

## Design Principles

- Compiled BSP is the authority for render geometry and model boundaries.
- Named brush entities are never flattened into worldspawn.
- Unsupported geometry fails closed.
- Render, collision and visibility data remain separate domains.
- Accuracy claims are scoped and machine-verifiable.
- No game assets or proprietary source excerpts are included.

## Roadmap

1. Displacements and overlays
2. Direct lightmap atlas generation, including directional bump channels
3. Static prop game lumps and reusable model references (metadata implemented; MDL resolution pending)
4. VMT/VTF material package integration
5. Collision brush and physics sidecars
6. Leaf/cluster/PVS sidecars
7. Versioned output manifests and runtime integration

## Acknowledgements

- [ValveSoftware/source-sdk-2013](https://github.com/ValveSoftware/source-sdk-2013) for the publicly
  available Source SDK and BSP definitions. Its own license applies to that repository.
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
