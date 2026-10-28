# Security policy

## Reporting a vulnerability

Please report security problems privately, not in a public issue: open the
repository's **Security** tab and choose **Report a vulnerability**. You will
get a reply within a few days, and a fix will be released as soon as it is
ready.

Only the latest release is supported; the app updates itself.

## How releases are protected

- Releases are built by GitHub Actions from a tagged commit, never on a
  developer's machine. Workflow actions are pinned to exact commit SHAs.
- Every release includes `SHA256SUMS`. The built-in updater refuses to
  install anything whose checksum does not match, and only downloads from
  this repository's releases over HTTPS.
- Release files carry a signed build-provenance attestation. To check that
  a download was built by this repository's release workflow:

  ```bash
  gh attestation verify StorageMap-0.1.3-windows-x64-setup.exe --repo sudo-py-dev/StorageMap
  ```

- Dependencies are checked against the RustSec advisory database on every
  change (`cargo audit`) and kept current by Dependabot.

## What StorageMap does on your computer

StorageMap only reads file metadata (names, sizes, dates) to build its
views; the duplicate finder also reads file contents to compare them.
Nothing is sent anywhere except the optional daily update check to GitHub.
Files are only ever moved to the trash, after you confirm.
