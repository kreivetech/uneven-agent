---
name: snapcraft-classic-packaging
description: >-
  Use this skill when packaging, building, or debugging Snapcraft snaps with
  confinement: classic (stage-packages, patchelf RPATH/interpreter isolation,
  linter checks, and Snap Store classic-confinement approval).
---

# Snapcraft Classic Confinement Packaging

Standards for building, isolating, and publishing `confinement: classic` snaps that pass Canonical Snap Store review.

## 1. Clean `snap/snapcraft.yaml`

- Keep `snap/snapcraft.yaml` public-ready without hardcoded local filesystem paths so store reviewers can inspect it directly (keep local-only overrides/scripts excluded from public repos).

## 2. Self-Contained Runtime Dependencies (`stage-packages`)

- Every `-dev` library listed in `build-packages` that is used by a dynamically linked binary must have its non-`-dev` runtime equivalent declared in `stage-packages`.

## 3. ELF `patchelf` Isolation

- Patch every ELF binary inside `override-build` using `patchelf` (`--set-interpreter` and `--set-rpath`) pointing strictly to the base snap (`/snap/<base>/current/...`) and `$SNAP` (`$ORIGIN/../...`) so host libraries and host `libc` are never loaded at runtime.

## 4. Snapcraft Linter Verification

- Run the Snapcraft linter during snap builds and resolve all missing-library, `RPATH`, or interpreter warnings before uploading to the Snap Store.

## 5. Store Approval & Publisher Verification

- `confinement: classic` requires per-snap approval via `forum.snapcraft.io` (`store-requests` -> `classic-confinement`) using the official request template after uploading an initial `classic` revision.
- The organization/publisher account must also request **publisher vetting / verification** through `forum.snapcraft.io` as part of getting `classic` confinement approved.
