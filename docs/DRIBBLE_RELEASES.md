# Pinning Releases in dribble.tf

The dribble.tf conversion pipeline should consume a reviewed `bsp-to-glb` release, never a moving
branch, the GitHub `latest` URL, or an unverified workflow artifact.

## Pin Contract

Commit all of the following values together in dribble.tf's tool configuration or lock data:

- Exact release tag, such as `v0.1.0`
- Exact platform archive name
- Expected lowercase SHA-256 digest copied from that release's `SHA256SUMS`

Archive names are deterministic:

| Platform | Archive |
|---|---|
| Linux x64 | `bsp-to-glb-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` |
| Windows x64 | `bsp-to-glb-v0.1.0-x86_64-pc-windows-msvc.zip` |

For example, the reviewed pin can use this shape in dribble.tf (the digest is intentionally a
placeholder, not a valid pin):

```json
{
  "version": "v0.1.0",
  "assets": {
    "linux-x64": {
      "archive": "bsp-to-glb-v0.1.0-x86_64-unknown-linux-gnu.tar.gz",
      "sha256": "<64 lowercase hex characters>"
    },
    "windows-x64": {
      "archive": "bsp-to-glb-v0.1.0-x86_64-pc-windows-msvc.zip",
      "sha256": "<64 lowercase hex characters>"
    }
  }
}
```

## Download And Verify

Construct the download URL from the pinned tag and archive, download to a temporary file, and
verify the pinned digest before extracting or executing anything:

```text
https://github.com/Hona/bsp-to-glb/releases/download/<tag>/<archive>
```

Linux verification example:

```bash
expected='<digest committed in dribble.tf>'
archive='bsp-to-glb-v0.1.0-x86_64-unknown-linux-gnu.tar.gz'
curl --fail --location --proto '=https' --tlsv1.2 \
  --output "$archive.tmp" \
  "https://github.com/Hona/bsp-to-glb/releases/download/v0.1.0/$archive"
printf '%s  %s\n' "$expected" "$archive.tmp" | sha256sum --check --strict
mv "$archive.tmp" "$archive"
```

Windows verification example:

```powershell
$expected = '<digest committed in dribble.tf>'
$archive = 'bsp-to-glb-v0.1.0-x86_64-pc-windows-msvc.zip'
$temporary = "$archive.tmp"
Invoke-WebRequest `
  -Uri "https://github.com/Hona/bsp-to-glb/releases/download/v0.1.0/$archive" `
  -OutFile $temporary
$actual = (Get-FileHash -Algorithm SHA256 $temporary).Hash.ToLowerInvariant()
if ($actual -cne $expected) { throw "bsp-to-glb checksum mismatch" }
Move-Item $temporary $archive
```

After extraction, run `bsp-to-glb --version-json` (or `bsp-to-glb.exe --version-json`) and require
its `version`, `target`, and `sourceCommit` to match the reviewed release metadata. Cache the tool by
digest rather than by a mutable filename. On any HTTP, checksum, JSON, version, target, or commit
mismatch, delete the temporary download and fail the conversion.

`SHA256SUMS` detects corruption and replacement relative to the reviewed pin; downloading a fresh
checksum from the same release at runtime is not an independent trust check. Updating the pin is a
reviewed dribble.tf change. The release currently has no detached signature, so the committed digest
and GitHub repository access controls are the trust anchors.
