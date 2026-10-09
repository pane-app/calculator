# Calculator

The Calculator is one of [Pane](https://github.com/pane-app/pane)'s
official extensions. Typing an arithmetic expression into Pane's root search
lists the answer first — `2 + 2`, `2 ^ 10`, `(1 + 2) * 3.5` — and invoking
the answer copies it to the clipboard.

It understands numbers, `+ - * / ^`, parentheses and a leading sign, with the
usual precedence; nothing more, by design (see
[`src/expression.rs`](src/expression.rs)). It is a *root provider*: the answer
appears in root search, and the package has no screen of its own.

This repository holds the extension's whole package, in the one-extension
shape of [ADR 0044](https://github.com/pane-app/pane/blob/main/docs/adr/0044-a-git-repository-holds-one-extension-or-a-collection.md):
[`pane.json`](pane.json), the images it names, and the Rust source
([`src/`](src), [`Cargo.toml`](Cargo.toml)) of its WebAssembly component.
The [release tags](#releases) hold the built component beside the
manifest and the images.

## Install

Install it from Git with Pane alone — no Git, compiler or other tool is
needed on your computer. In root search, choose **Install extension from
Git…** and name

    https://github.com/pane-app/calculator@v0.5.0

or, from a terminal, `pane --install git:github.com/pane-app/calculator@v0.5.0`.
A `v<version>` tag pins a release: the tag's commit holds the package
complete, at the version `pane.json` names. Without a reference, Pane
installs the default branch, which between releases holds only the source
and is explained to users as source-only.

## Releases

A release is made from GitHub: **Actions → Release → Run workflow** on the
branch to release. The workflow builds the component with the same CI,
copies it where `pane.json` names it, commits that on the `release`
branch, tags the commit `v<pane.json's version>` and pushes the branch
and the tag. The tag's commit is the *release revision*: it holds
`pane.json`, the images and the built component (`dist/calculator.wasm`), and it
is what Pane installs when a user names the tag — and the commit a Pane
release pins for its first setup (pane-app/pane#282).

To release a new version, raise `version` in `pane.json` (and in
`Cargo.toml`) through a pull request, then run the workflow. A tag is never
re-written: a released version that must change is released again as a new
version.

## Building

```
cargo build --release --target wasm32-wasip2
```

builds `target/wasm32-wasip2/release/calculator.wasm`; copy it to
`dist/calculator.wasm` for a revision users can install — what the Release
workflow does. `rust-toolchain.toml` pins the toolchain Pane builds its
own extensions with.

The component builds against Pane's Rust SDK, `pane-extension`
(`pane-extension = "0.1"` in [`Cargo.toml`](Cargo.toml)). The SDK is not
published to crates.io yet (pane-app/pane#281); until it is, point a path
at a Pane checkout's `guests/pane-extension` — as this repository's CI
does — by appending to `Cargo.toml`:

```toml
[patch.crates-io]
pane-extension = { path = "<a Pane checkout>/guests/pane-extension" }
```

pane-app/pane#287 removes the CI patch once the crate is published.

## Issues and contributing

This extension's issues live in
[this repository's tracker](https://github.com/pane-app/calculator/issues).
The extension API — everything a Pane command can do, and how a package is
put together — is documented in Pane's repository:
[the extension guests' README](https://github.com/pane-app/pane/blob/main/guests/README.md)
and [Pane's docs](https://github.com/pane-app/pane/tree/main/docs).

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or
[MIT](LICENSE-MIT), as Pane is: choose either at your option.
