# Contributing to LeatherMC

Thanks for your interest in LeatherMC — a vanilla Minecraft server written in Rust, built from
scratch, one small brick at a time. This document describes how we work. The rules are intentionally
**lightweight**: this is an early (`0.0.1-alpha`) project, and we'd rather keep the barrier to
contributing low than drown newcomers in process. They will grow as the project grows.

## TL;DR

- Open a PR (no direct pushes to `main`).
- Keep PRs small and focused — match the "one fine-grained brick at a time" philosophy.
- Make CI green (format, lint, tests, build).
- Sign off your commits (DCO): `git commit -s`.
- Use a [Conventional Commit](https://www.conventionalcommits.org/) **PR title** (e.g. `feat: add login`).
- For anything large, **open an issue first** to discuss before writing the code.

## Workflow

1. **Fork** the repo (or, if you're a maintainer, branch from `main`).
2. Create a topic branch: `feat/...`, `fix/...`, `docs/...`, `chore/...`.
3. Make your change, with tests where it's testable.
4. Run the checks locally (see below) until they pass.
5. Open a Pull Request against `main`.
6. CI must pass. A maintainer reviews and **squash-merges** it.

`main` is protected: all changes go through a PR, CI must be green, and PRs by non-maintainers
require a maintainer's approval. Merges are **squash** merges, so `main` keeps one clean commit per
PR — that's why the **PR title** must be a Conventional Commit.

### Discuss big changes first

Small fixes: just open a PR. **Large changes** (new subsystems, protocol phases, dependencies,
anything sprawling): please **open an issue first** so we can agree on the approach. This protects
you from writing a lot of code that might not be merged. We do **not** use a heavy formal RFC
process — a plain issue discussion is enough.

## Local checks

These mirror CI. Run them before pushing:

```bash
cargo fmt --all                              # format
cargo clippy --all-targets -- -D warnings    # lint (warnings are errors)
cargo test --all                             # tests, incl. the ping integration test
```

## Coding rules

- **Rust edition / version:** we always track the **latest stable** Rust. No old-version (MSRV)
  guarantees — this is an application, not a library.
- **Formatting:** `rustfmt` is mandatory and enforced by CI.
- **Lints:** `clippy` must be clean; CI runs it with `-D warnings`.
- **`unsafe` code:** forbidden **by default** — every crate denies it with `#![deny(unsafe_code)]`.
  When `unsafe` is genuinely required (e.g. the future JVM/FFI bridge for `.jar` plugin support), it
  is allowed **locally** with an explicit `#[allow(unsafe_code)]` and a comment explaining **why it
  is sound**. Never sprinkle `unsafe` without justification.
- **Tests:** new features and bug fixes should come with tests **when the change is testable**.
  We're pragmatic, not dogmatic — purely I/O or logging glue may legitimately have none, but
  protocol/logic changes should.

## Commits & PR titles

- **PR title** must follow [Conventional Commits](https://www.conventionalcommits.org/):
  `type(scope): summary`. Common types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `chore`,
  `ci`. The title becomes the squash-commit message on `main`, and lets us auto-generate a changelog
  later — so we don't keep a manual `CHANGELOG.md` for now.
- **Individual commit messages** inside a PR can be free-form (they get squashed), but keep them in
  **English** and reasonably clear.

## Developer Certificate of Origin (DCO)

We use the [DCO](https://developercertificate.org/) — a lightweight, paperwork-free way for you to
certify you have the right to submit your contribution. There is **no CLA**; your contribution stays
under the project's [MIT license](LICENSE).

Add a sign-off line to every commit by committing with `-s`:

```bash
git commit -s -m "fix: handle empty status request"
```

This appends a trailer matching your git identity:

```
Signed-off-by: Your Name <you@example.com>
```

CI checks that every commit in a PR is signed off.

## AI-assisted contributions

Using AI tools to help write code is **allowed**. But **you, the contributor, are fully
responsible** for what you submit: that it works, that it fits the project, and — importantly — that
it does **not** carry incompatible licensing or copied code. Your DCO sign-off applies regardless of
how the code was produced. You don't have to disclose AI use, but the responsibility is yours.

## Language

Code, comments and PRs should be in **English** (it keeps the project accessible internationally).
**French is tolerated** in issues and discussions if that's easier for you — we won't turn away a
good bug report because of language.

## Governance

The sole maintainer for now is **Arklandia Studios** (`@ArklandiaStudios`), who reviews and merges
all PRs. As the project grows we'll add more maintainers via a `CODEOWNERS` file. Decisions are made
pragmatically; when in doubt, open an issue and let's talk.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By participating, you're
expected to uphold it. Be respectful.

---

Happy hacking — and welcome aboard. 🛠️
