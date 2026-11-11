# Agents

Namevane is one desktop app in one repository. The default branch is `master`.

When `.ai/plan.md` is on the machine, it is the implementation sequence. Follow it in order. Do not start a later step until the earlier step’s check passes. That file is local. It is gitignored and is not part of the public tree.

## Product rules

- A pin is one website name and one or two addresses. Applying it writes one marked block in the hosts file and leaves every other line alone.
- The browser keeps the name in the address bar and still checks the certificate. Do not install a certificate authority, decrypt TLS, or hide a certificate warning.
- The pin affects the whole computer until it is turned off. It does not affect any other computer.
- Do not add a browser extension, a local DNS server, profiles, remote rule lists, a resident administrator service, an account, telemetry, an updater, a database, or a local web server.
- The window is HTML, CSS, and TypeScript. Do not add React or a component library.
- Rust does the system work. The first version may use `serde`, `serde_json`, the operating system’s TLS stack, and Hickory for a direct public DNS lookup. `tray-icon` is enabled. The menu-bar icon is shown while Namevane is running.
- The window runs as the normal user. A helper may run only for Apply and Turn all off, then it exits.
- Reject `localhost`, an empty name, an empty address, a malformed address, and a wildcard.
- Rust tests that parse or render a hosts file use a fixture. No test writes the real hosts file.

## Repository rules

- Do not commit `.ai/`, `wiki/`, `.cursor/`, `.vscode/`, secrets, `node_modules/`, or `target/`. `wiki/` is the local source for the GitHub wiki, including the script that publishes it.
- The license is Apache-2.0. The text is `LICENSE.md`. Machine-readable license fields live in `package.json` and `src-tauri/Cargo.toml`.
- Keep `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` on the same version. Do not tag `v0.1.0` until the macOS apply and explanation checks have passed.
- Tests run from `.github/workflows/ci.yml`. A tag `vX.Y.Z` runs `.github/workflows/release.yml`, which publishes unsigned bundles plus `SHA256SUMS` and a keyless cosign bundle. Do not add Apple notarization or Authenticode to that workflow.
- The macOS bundle uses an ad-hoc signing identity (`-`). Leave it that way until a later signing release.
- Conventional commits. Pull requests target `master`.

## Checks

```bash
npm run build
npm test
```

`npm run build` typechecks and builds the window. `npm test` runs `cargo test` for `src-tauri`. After a change to the window, run `npm run tauri dev` and confirm the window opens and closes.
