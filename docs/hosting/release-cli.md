# Refresh the website CLI downloads

The website installer reads `site/public/dl`, not GitHub release tarballs. A new
backend or site deployment does not rebuild those files. Package them after all
release PRs have merged; keep deployment manual.

1. Run the **package-site-cli** workflow on the intended merged `main` revision.
   Wait for all three builds and the bundle checksum check to pass. PR runs are
   build verification only; they are not a release of the other pending fixes.
2. Download that run's `site-cli-<revision>` artifact to a temporary directory.
   Confirm `BUILD.json` names the intended revision and run
   `sha256sum -c SHA256SUMS.txt` (macOS: `shasum -a 256 -c SHA256SUMS.txt`).
3. Replace all three matching binaries, `SHA256SUMS.txt`, and `BUILD.json` in
   `site/public/dl` together. Upload/extraction can lose executable bits;
   `chmod +x site/public/dl/snapshotdb-*`. Commit the generated files in the
   release update, then deploy the site. Keep the previous bundle for rollback.
4. Deploy a backend built from the same source revision. `snapshotdb --version`
   prints its Git revision; authenticated `/v1/health` exposes `revision` too.
5. Download `/dl/BUILD.json`, `/dl/SHA256SUMS.txt`, and all three binaries from the
   deployed site. Compare them to the artifact and run the installer into a
   temporary `SNAPSHOTDB_PREFIX`; check `snapshotdb --version` and one real API
   operation. A green workflow alone does not prove the served bytes changed.

The workflow only builds and uploads artifacts to its GitHub Actions run. It does
not publish releases, alter the repository, install binaries, or deploy anything.
Source archives without Git can set `SNAPSHOTDB_BUILD_SHA` to the source's full
40-character commit. An unavailable revision displays `unknown`, never a guessed
revision. Local modified builds can share a commit identifier: release only clean
CI builds, with the bundle checksum manifest as the artifact identity.
