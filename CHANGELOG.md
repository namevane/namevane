# Changelog

All notable changes to Namevane are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Lists. Add the address of a shared text file of names and addresses, turn each list on or off, and check for a new version by hand or while Namevane is running. Apply still writes the hosts file. A name you added yourself is used instead of the same name in a list.

## [0.0.1-beta.2] - 2026-11-11

A beta tag for the release workflow. This is not `v0.1.0`. A pre-release builds a `.deb` and an AppImage on Linux, and an NSIS installer on Windows. The RPM and MSI packages stay on stable tags, because they cannot use a version such as `0.0.1-beta.2`.

### Added

- English and Bulgarian. The language is chosen in Settings, and another language is another JSON file with the same keys.
- Settings are on their own page: keep running in the background, open at login, export and import the address list, and install a newer GitHub release on macOS.
- A local macOS install script, a menu-bar icon, and two settings: keep running when the window closes, and open when you log in.
- An About panel: Namevane by RumenX, the current version, whether a GitHub release is available, and links to the source and releases.
- Apply writes enabled address records into the hosts file and asks for an administrator password. Turn all off removes that block.
- A saved list of custom addresses. Add, edit, and remove a name, and turn it on or off. A name points to an IPv4 address, an IPv6 address, or both.
- A Tauri 2 window titled Namevane by RumenX. The screen says Custom addresses.
- Public project documents, Apache-2.0 license, and GitHub Actions workflows for tests and for publishing a tagged release with `SHA256SUMS` signed by keyless cosign.
