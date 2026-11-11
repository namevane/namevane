# Namevane

Namevane pins a website name to an address on this computer. You add the name and the address, then apply the pin. After that, this computer opens that name at that address. The browser keeps the name in the address bar and still checks the certificate.

The name is provisional. It can change any time before the first public release. The window title stays **Namevane**. The screen says **Custom addresses**.

Namevane is one repository. The window, the Rust core, and the helper that writes the hosts file ship together, on the `master` branch, as one version.

## What a pin does

A pin is one name and one or two addresses (IPv4, IPv6, or both). Applying it writes lines the operating system already understands:

```text
203.0.113.10 app.example.com
```

After that, `app.example.com` resolves to `203.0.113.10` for every program on this computer. The browser requests `https://app.example.com`, sends that name, and checks the certificate against that name. The server at the chosen address has to present a valid certificate for the name. Namevane does not install a certificate authority and does not turn browser warnings off.

The pin affects this whole computer until it is turned off. Other people, and other computers, are not affected.

The addresses above are documentation addresses. In the app they stand for the server address you were given.

## What stays out

- A browser extension, or a local DNS server. The hosts file is the mechanism browsers honor.
- Profiles, a hosts-file text editor, and remote hosts lists.
- A resident administrator service. The password prompt runs only while a change is written.
- Decrypting TLS, installing a private certificate authority, or hiding a certificate warning.
- An account, telemetry, an updater, a database, and a local web server.

## Status

The window opens. It shows **Custom addresses** and nothing else yet. Pins are not saved, and nothing writes the hosts file. There is no installable release.

The first tag is `v0.1.0`, and only after the macOS apply and explanation checks pass. Until then the version in the tree is `0.0.0`. The icon is still the Tauri template icon.

## Develop

You need Node.js, Rust, and the system libraries [Tauri documents for your platform](https://v2.tauri.app/start/prerequisites/). Development starts on macOS. A person installing a finished build does not install Node or Rust.

```bash
npm install
npm run tauri dev
```

The window talks to a Vite dev server on port 1420. Close the window to stop it.

## Tests

```bash
npm run build
npm test
```

`npm run build` typechecks the window and produces the frontend bundle. `npm test` runs the Rust tests. Those tests must not write the real hosts file.

GitHub Actions runs both on every push and pull request to `master`.

## Releases

Pushing a tag `vX.Y.Z` (optional pre-release suffix, such as `v0.1.0-rc.1`) runs the release workflow. It builds bundles for macOS (Apple silicon and Intel), Windows, and Linux, then publishes one GitHub release.

These bundles are not signed with an Apple Developer ID and are not Authenticode-signed. macOS Gatekeeper and Windows SmartScreen will warn. The macOS app is ad-hoc signed (`signingIdentity` is `-`) so a download is not reported as damaged. Platform signing is a later release.

The release also contains `SHA256SUMS` and `SHA256SUMS.bundle`. The bundle is a keyless [cosign](https://docs.sigstore.dev/cosign/signing/overview/) signature from the release workflow. To check a download, put the bundles, `SHA256SUMS`, and `SHA256SUMS.bundle` in one directory and run:

```bash
cosign verify-blob \
  --bundle SHA256SUMS.bundle \
  --certificate-identity "https://github.com/OWNER/REPO/.github/workflows/release.yml@refs/tags/vX.Y.Z" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  SHA256SUMS

sha256sum -c SHA256SUMS
```

Replace `OWNER/REPO` and `vX.Y.Z` with the repository and the tag you downloaded. `sha256sum -c` checks each file named in `SHA256SUMS`.

Before tagging, set the same version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.

## Layout

```text
src/            the window
src-tauri/      the Rust application and, later, the helper
.github/        tests and the release workflow
```

## Project documents

- [Contributing](CONTRIBUTING.md)
- [Security](SECURITY.md)
- [Changelog](CHANGELOG.md)
- [Code of conduct](CODE_OF_CONDUCT.md)
- [Funding](FUNDING.md)
- [Agents](AGENTS.md)
- [License](LICENSE.md)

## License

Apache License 2.0. Copyright 2026 Rumen Damyanov. See [LICENSE.md](LICENSE.md) and [NOTICE](NOTICE).
