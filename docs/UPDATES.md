# Ponte Mesh SDK Updates

The SDK update flow is prepared for GitHub Releases but does not assume a fixed
GitHub owner or a private machine path. The repository is selected at runtime and
is pinned locally on first verified use.

## Release publishing

Use the manual `SDK Release` GitHub Actions workflow after the repository is
public and the release version is already committed in `Cargo.toml`.

The workflow:

- reads the semver version directly from `Cargo.toml`;
- marks versions containing a semver pre-release suffix as pre-releases;
- refuses to publish if the caller is not the repository owner;
- refuses to publish if the corresponding Git tag already exists;
- builds and tests the SDK on Linux x64, Windows x64, macOS Intel, and macOS ARM;
- publishes native SDK packages;
- publishes `pontemesh-sdk-v<VERSION>-manifest.json` with SHA-256 and size for
  each asset.

## Update checking

The updater checks GitHub Releases and can stage a newer asset in the background.
It never executes downloaded content.

First verified use:

```bash
./scripts/check-sdk-update.sh \
  --repository OWNER/REPOSITORY \
  --trust-on-first-use \
  --stage \
  --asset-pattern '*linux-x64.tar.gz'
```

Regular background check:

```bash
./scripts/check-sdk-update.sh \
  --repository OWNER/REPOSITORY \
  --stage \
  --asset-pattern '*linux-x64.tar.gz' \
  --background
```

By default, checks are spaced by 24 hours. Use `--interval-seconds` to change the
interval and `--force` only for manual validation.

State and reports are written under `target/update-state` by default. Staged
downloads are written under `target/update-staging`. Both locations can be
changed with `PONTEMESH_UPDATE_STATE_DIR` and `PONTEMESH_UPDATE_STAGE_DIR`.

Security boundaries:

- repository trust is pinned in `trusted-repository`;
- switching repositories fails unless the local state is intentionally replaced;
- downloaded assets must match the release manifest product, version, size, and
  SHA-256;
- no binary or library is loaded automatically by the updater.
