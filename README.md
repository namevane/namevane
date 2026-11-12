# Namevane

Point a domain name to the address you choose.

Only the computer running Namevane follows it. Other computers keep using the public record. The browser keeps the domain name in the address bar and still checks the certificate.

The name is provisional. It can change any time before the first public release. The window title is **Namevane by RumenX**. The screen says **Custom addresses**.

Namevane is one repository. The window, the Rust core, and the helper that writes the hosts file ship together, on the `master` branch, as one version.

## What a pin does

A pin is one name and one or two addresses (IPv4, IPv6, or both). Applying it writes lines the operating system already understands:

```text
203.0.113.10 app.example.com
```

After that, `app.example.com` resolves to `203.0.113.10` for every program on the machine running Namevane. The browser requests `https://app.example.com`, sends that name, and checks the certificate against that name. The server at the chosen address has to present a valid certificate for the name. Namevane does not install a certificate authority and does not turn browser warnings off.

The pin stays in effect until it is turned off. Other machines are not affected.

The addresses above are documentation addresses. In the app they stand for the server address you were given.

## Examples

### A site does not open because "DNS is not working"

Sometimes a website does not open for you, but it opens for other people. The cause can be a "misconfigured DNS server": the server your network uses to look up names gives a wrong answer, or no answer. The website itself is fine.

For example, suppose `wakanda.net` does not open on your computer. It is the website of a fictional African country, used here only as an example. You know the correct address of its server, or someone who runs the site gave it to you.

1. Open Namevane and type `wakanda.net` in the name field.
2. Type the server address in the IPv4 or IPv6 field.
3. Press **Add**, then press **Apply**.
4. Open `https://wakanda.net` in your browser.

On this computer, `wakanda.net` now leads to the address you typed. The address bar still shows `wakanda.net`, and the browser still checks the certificate for that name.

When the DNS problem is fixed, press **Turn all off**. The site then follows the public record again.

Two things to keep in mind:

- Use only an address you trust. Namevane will use whatever address you type.
- If the browser shows a certificate warning, the server at that address does not present a valid certificate for `wakanda.net`. Namevane does not hide that warning. Ask the people who run the site for the correct address.

### Check a new server before you switch the domain

You run a web application at `shop.example.com`. You are moving it from one server to another, perhaps to a new hosting provider. The new server is at `198.51.100.20`. The public record still points to the old server, so visitors still reach the old one.

Before you change the public record, you want to see that the new server works under the real name.

1. Type `shop.example.com` in the name field.
2. Type `198.51.100.20` in the IPv4 field.
3. Press **Add**, then **Apply**.
4. Open `https://shop.example.com` in your browser. Sign in, open the pages, and try what matters.
5. If something is wrong, press **Turn all off**. Only this computer was ever affected, so visitors are not affected.
6. When everything works, change the public record at your domain provider. Then press **Turn all off** on this computer, so it follows the public record again.

The new server must already have a valid certificate for `shop.example.com`. If it does not, the browser shows a warning during the test, and that is the answer you need before the switch.

## What stays out

- A browser extension, or a local DNS server. The hosts file is the mechanism browsers honor.
- Profiles, a hosts-file text editor, and remote hosts lists.
- A resident administrator service. The password prompt runs only while a change is written.
- Decrypting TLS, installing a private certificate authority, or hiding a certificate warning.
- An account, telemetry, a silent updater, a database, and a local web server. A newer release is installed only after you press the button.

## Status

The window saves a list of custom addresses. You can add, edit, and remove a name, and turn it on or off. A name points to an IPv4 address, an IPv6 address, or both. **Apply** writes those addresses into the hosts file.

You can install a local build and open that copy instead of a development window. Settings has **Keep running in the background**, **Open at login**, export and import of the address list, and **Install update** when a newer GitHub release exists. On macOS that button downloads the release, checks its SHA-256 sum, and replaces the installed app. There is no public release yet.

The first stable tag is `v0.1.0`, and only after the macOS apply and explanation checks pass. A beta tag such as `v0.0.1-beta.1` can exercise the release workflow before that. About shows the version in this build and looks for a newer GitHub release. On macOS, **Install update** downloads that release after you press the button.

## Develop

You need Node.js, Rust, and the system libraries [Tauri documents for your platform](https://v2.tauri.app/start/prerequisites/). Development starts on macOS. A person installing a finished build does not install Node or Rust.

```bash
npm install
npm run tauri dev
```

The window talks to a Vite dev server on port 1420. With **Keep running in the background** on, closing the window leaves Namevane in the menu bar. Quit from that icon to stop the development process. If port 1420 is already in use, quit the other Namevane window first.

## Install on this Mac

This builds the app and copies it to `/Applications`. The installed copy does not need Node or Rust.

```bash
script/install-macos.sh
```

The built app is also at `src-tauri/target/release/bundle/macos/Namevane.app`, inside a disk image in the same folder. It is ad-hoc signed, not notarized, so the first open uses Control-click, then Open.

Turn on **Open at login** from the installed app. Doing it from `npm run tauri dev` would start the development copy.

On Windows, `npm run tauri build` writes an installer under `src-tauri/target/release/bundle/nsis/`. On Linux it writes `.deb`, `.rpm`, and `.AppImage` packages under `src-tauri/target/release/bundle/`. The same two settings are in the window on every system.

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
  --certificate-identity "https://github.com/namevane/namevane/.github/workflows/release.yml@refs/tags/vX.Y.Z" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  SHA256SUMS

sha256sum -c SHA256SUMS
```

Replace `vX.Y.Z` with the tag you downloaded. `sha256sum -c` checks each file named in `SHA256SUMS`.

Before tagging, set the same version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.

## Layout

```text
src/            the window
src-tauri/      the Rust application. Apply runs this same binary once, then it exits
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
