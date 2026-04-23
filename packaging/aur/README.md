# AUR packaging

Canonical source for the `clickup-cli-bin` package on the [Arch User Repository](https://aur.archlinux.org/packages/clickup-cli-bin).

## Files

- `PKGBUILD` — Arch package recipe. The `-bin` variant pulls prebuilt binaries from our GitHub Releases (no Rust toolchain required on the user's box).
- `.SRCINFO` — machine-readable metadata AUR uses for search and dependency resolution. Regenerated from `PKGBUILD` on every release.

## Release flow

On each non-prerelease tag, `.github/workflows/publish-aur.yml` does:

1. Bumps `pkgver` in `PKGBUILD` to match the tag.
2. Runs `updpkgsums` to recompute `sha256sums_*` from the new release tarballs.
3. Regenerates `.SRCINFO`.
4. Pushes the commit to `ssh://aur@aur.archlinux.org/clickup-cli-bin.git` using the `AUR_SSH_KEY` secret.

The workflow is skipped for prerelease tags (anything containing `-`), matching the main release workflow's publish guards.

## Initial submission (one-time)

AUR creates the package repository on first push. The GitHub Actions workflow can't do this for us because it doesn't know how to create the AUR repo from nothing. Bootstrap locally:

```sh
# Clone the (not yet existing) AUR repo
git clone ssh://aur@aur.archlinux.org/clickup-cli-bin.git /tmp/aur-clickup-cli-bin
cd /tmp/aur-clickup-cli-bin

# Copy the package files from this repo
cp /path/to/clickup-cli/packaging/aur/PKGBUILD .
cp /path/to/clickup-cli/packaging/aur/.SRCINFO .

# Push to create the AUR package
git add PKGBUILD .SRCINFO
git commit -m "Initial upload: clickup-cli-bin 0.8.2"
git push origin master
```

After that first push succeeds, every future tag auto-updates the AUR package via the workflow.

## Manual test

If you want to sanity-check the `PKGBUILD` locally on an Arch/Alpine system:

```sh
docker run --rm -it -v "$PWD":/pkg archlinux:latest bash -c '
  pacman -Sy --noconfirm base-devel &&
  useradd -m builder &&
  chown -R builder /pkg &&
  su builder -c "cd /pkg && makepkg -si --noconfirm"
'
```
