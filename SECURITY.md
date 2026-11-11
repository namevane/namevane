# Security policy

## Supported versions

Namevane has no public release yet. No version is supported until `v0.1.0`.

| Version | Supported |
| --- | --- |
| `0.0.0` (development) | No |
| `v0.1.0` and later | After that tag exists |

## Reporting a vulnerability

Report a security issue in private. Do not open a public issue for an undisclosed vulnerability.

- Email [dev@rumenx.com](mailto:dev@rumenx.com).
- Once this repository is on GitHub, you can also use [private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability).

Say what you did, what you expected, and what happened. If the report is about a hosts-file write, a copy of the file helps. Remove addresses you do not want to share.

You will get a reply within 7 days. A fix is prepared before any public discussion.

## What belongs in a report

Namevane writes names and addresses into the hosts file of the computer it runs on, after an administrator password prompt. A pin that sends a name to an address the person typed is the product.

These are in scope:

- The helper writing outside the marked Namevane block, or writing a line that is not a valid pin.
- The password prompt, or the temporary command file, being readable by another user on the machine.
- A backup of the hosts file left where another user can read it.
- The app fetching a rule list, disabling a certificate warning, or installing a certificate authority.

A certificate warning in the browser is the browser doing its job. Namevane does not turn those warnings off.

## Checking a release

Release assets include `SHA256SUMS` and a keyless cosign bundle, `SHA256SUMS.bundle`. The verification command is in the [README](README.md#releases). A failed check means the download should not be installed.
