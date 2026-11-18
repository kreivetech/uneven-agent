# Repository Dual-Remote & Commit Guide (`AGENTS.md`)

This workspace maintains two Git remotes:

| Remote | Repository | Visibility | Purpose |
| :--- | :--- | :--- | :--- |
| `origin` | `git@github.com:kreivesler/uneven.git` | Private | Full source of truth (includes `snap/`, `.snapcraftignore`, `.github/`) |
| `org` | `git@github.com:kreivetech/uneven-agent.git` | Public | Open-source distribution & free GitHub Actions CI/Release runners |

## Excluded Paths on `org` (Public Mirror)

When syncing commits to `org`, packaging-only files that belong exclusively to the private Snap build are excluded:
- `snap/`
- `.snapcraftignore`

`.github/workflows/release.yml` **is included** on `org` so release tags (`v*`) run on free public GitHub Actions runners (using `RELEASES_REPO_TOKEN` secret to publish artifacts to `kreivesler/uneven-releases`).

## Standard Commit & Dual-Push Workflow

1. **Commit normally on `origin` (`main`):**
   ```bash
   git add <files>
   git commit -m "feat(...): ..."
   git push origin main
   ```

2. **Sync `main` (and optional release tag) to `org`:**
   ```bash
   ./scripts/sync-org.sh
   # Or with a release tag:
   ./scripts/sync-org.sh --tag v2.4.0
   ```

   `scripts/sync-org.sh` creates a temporary worktree from `HEAD`, removes `snap/` and `.snapcraftignore`, creates a clean tree commit on top of `org/main` (preserving `org`'s public history without leaking excluded files), and pushes to `org main` (plus the tag if `--tag` is passed).
