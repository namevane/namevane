# Contributing

Thank you for helping Namevane. By taking part you agree to the [code of conduct](CODE_OF_CONDUCT.md).

## What this project is

Namevane is one desktop app in one repository. The default branch is `master`. Pull requests target `master`.

A change should keep these properties:

- A pin affects the whole computer it is applied on, and no other computer.
- The browser keeps the website name and still checks the certificate.
- The hosts file is the mechanism. Namevane does not add a browser extension, a local DNS server, a certificate authority, telemetry, an account, an updater, or a resident administrator service.
- The window stays HTML, CSS, and TypeScript. Do not add React or a component library.
- Rust tests that touch a hosts file use a fixture. They do not write `/etc/hosts` or the Windows hosts file.

The [README](README.md) describes the product. [AGENTS.md](AGENTS.md) is the short brief for coding agents.

## Set up

Install Node.js, Rust, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your system. Then:

```bash
npm install
npm run tauri dev
```

Before you open a pull request:

```bash
npm run build
npm test
```

## Commits and pull requests

Use a [conventional commit](https://www.conventionalcommits.org/) subject: `feat`, `fix`, `chore`, `docs`, `refactor`, or `test`.

A pull request needs a short summary and a test plan. Say which operating system you ran, and whether the window still opens. If you did not run a check, say so.

Keep the change focused. A drive-by rewrite of an unrelated file is harder to review.

Do not commit `.ai/`, `.cursor/`, `.vscode/`, secrets, or built artifacts. `.gitignore` already excludes them. `package-lock.json` and `src-tauri/Cargo.lock` do belong in the commit.

## Versions

`v0.1.0` waits until the macOS apply and explanation checks pass. A beta tag may be published before that.

When a release is ready, set the same version in all three places:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

Add the entry to [CHANGELOG.md](CHANGELOG.md), commit, and push a tag `vX.Y.Z` on `master`. The release workflow builds the bundles, writes `SHA256SUMS`, signs that file with keyless cosign, and publishes the GitHub release. Do not tag from a pull-request branch.

## Reporting issues

Use the issue templates. Security reports go through [SECURITY.md](SECURITY.md), not a public issue.

Questions about behavior belong in an issue. Questions about whether a feature fits belong there too. Remote rule lists, a local DNS server, and hiding certificate warnings are outside the product.
