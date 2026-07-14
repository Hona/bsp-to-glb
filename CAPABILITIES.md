# Release Capabilities

This file is the human-readable capability snapshot distributed with `bsp-to-glb` releases.
The authoritative machine-readable snapshot is emitted by `bsp-to-glb --version-json` and embedded
as `build-metadata.json` in every archive.

| Capability | Status |
|---|---|
| Compiled brush geometry and BSP models | Supported |
| Compiled displacement geometry | Supported |
| Direct LDR/HDR lightmap data and manifests | Supported |
| Material and prop metadata | Supported |
| Brush collision sidecars | Supported |
| BSP plane/node traversal, leaf, cluster, and PVS visibility sidecars | Supported |
| Ordered compiled entity graph and I/O connection sidecars | Supported |
| Overlay, water-overlay, and cubemap detection | Detected only |
| Static and dynamic prop model geometry | Unsupported |
| VTF pixel conversion and content-addressed PNG packages | Supported |
| Ordered directory and native VPK v1/v2 material resolution | Supported |
| Bounded polygon PHY/PHYSCOLLIDE decoding and static-physics packages | Supported |

## Component Versions

| Component | Version |
|---|---:|
| Build metadata | 2 |
| Source material manifest | 3 |
| Material mount plan | 1 |
| Material texture manifest | 1 |
| Visibility sidecar | 2 |
| Entity graph sidecar | 1 |
| Static-physics manifest and binary | 1 |

`detectedOnly` means the input is reported but no corresponding render geometry or texture output
is produced. `unsupported` means callers must provide another implementation for that domain.
Unsupported or unknown data that could make supported output inaccurate fails closed where the
exporter can identify it. See `README.md` for detailed scope and limitations.

No game assets are included in release archives.
