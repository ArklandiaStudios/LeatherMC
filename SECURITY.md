# Security Policy

## Supported versions

LeatherMC is in early development (`0.0.x-alpha`). Only the **latest code on `main`** receives
security fixes. There are no long-term supported releases yet; this section will grow once we cut
stable versions.

| Version            | Supported |
| ------------------ | --------- |
| `main` (latest)    | ✅        |
| older / pre-`main` | ❌        |

## Reporting a vulnerability

**Please do not open a public issue for security vulnerabilities.** Public reports expose users
before a fix is available.

Instead, report privately through GitHub's built-in private reporting:

1. Go to the repository's **[Security advisories → Report a vulnerability](https://github.com/ArklandiaStudios/LeatherMC/security/advisories/new)**.
2. Describe the issue (see below). This opens a **private** thread visible only to you and the
   maintainers.

If you can't use GitHub's private reporting, you may open a minimal issue asking the maintainers to
get in touch — **without** any vulnerability details.

### What to include

The more of this you can provide, the faster we can act:

- A clear description of the vulnerability and its impact.
- Steps to reproduce, or a proof of concept.
- The affected commit / version and the platform you tested on.
- Any suggested fix or mitigation, if you have one.

## What to expect

This is a small, early-stage project maintained on a **best-effort** basis. We do **not** promise a
fixed response time, but we will:

- acknowledge your report as soon as we reasonably can,
- keep you informed about our assessment and progress,
- credit you in the advisory once the issue is resolved, if you'd like.

We ask that you practice **coordinated disclosure**: please give us a reasonable chance to release a
fix before disclosing the issue publicly.

## Scope

In scope: the LeatherMC server code in this repository.

Out of scope: vulnerabilities in third-party dependencies (please report those upstream, though we
appreciate a heads-up), and issues that require physical access to the server host.

## No bug bounty

We're honest about this: there is **no monetary reward**. Contributions to LeatherMC's security are
voluntary and gratefully acknowledged.
