# Calculator

The Calculator is one of [Pane](https://github.com/pane-app/pane)'s
official extensions. Typing a calculation into Pane's root search lists
the answer first — `2 + 2`, `20% of 50`, `5 km in miles`, `days until
christmas` — and invoking the answer copies it to the clipboard, as it is
shown; the Actions panel also copies it without its thousands grouped.

It reads what Raycast's calculator reads, offline, and converts
currencies with rates it fetches once a day:

- arithmetic: `+ - * / ^` and `×`, `x`, `÷`, `−`, `mod`, the words (`plus`,
  `times`, `divided by`), parentheses, a leading sign, `5!`, `5²`, `√16`,
  and multiplication without a sign (`2pi`, `3(4 + 5)`);
- numbers as `1,000`, `1e3`, `10k`, `2M`, `0xff`, `0b1010` and `0o17`;
- constants (`pi`, `e`, `tau`, `phi`) and functions (`sqrt`, `cbrt`, `sin`,
  `cos`, `tan` and their inverses, `ln`, `log`, `log2`, `exp`, `abs`,
  `round`, `floor`, `ceil`), in radians unless a unit says degrees;
- percentages: `20% of 50`, `50 + 10%`, `10% off 50`, `5% on 30`,
  `10 is what % of 50`, `what % of 50 is 10`, `10 as a % of 50`;
- units of length, mass, volume, area, time, speed, temperature, data
  (`MB` is 1000², `MiB` 1024²), angle and CSS pixels, converted with `in`,
  `to` or `as` (`100 f to c`, `1 GB in MB`, `5 ft 3 in in cm`);
- number bases: `255 in hex`, `0xff in decimal`, `99 in binary`;
- dates and times: `now + 3 days`, `35 days ago`, `next monday`,
  `days until christmas`, `days between 3 march and 30 may`,
  `time in Tokyo`, `5pm London in New York`, `1700000000 to date`,
  copied from the Actions panel as ISO 8601 or a Unix timestamp too;
- colours: `#3aa`, `#FF8800`, `rgb()`, `rgba()`, `hsl()`, `hsla()` and
  `oklch()`, answered as hex with a swatch and copied as hex, RGB, HSL or
  OKLCH;
- currencies: `100 usd in vnd`, `100 usd to vnd`, `$100 in eur`,
  `€50 to usd`, `50€ in £`, `1000 yen to dong`, `100 dollars in euros`,
  `usd to vnd`, and crypto such as `5 btc in eur` and `1 eth to usd`, in
  any case, by code, symbol or name; an amount alone (`100 eur`) goes into
  the currency the **Your currency** preference names (USD unless set).
  The answer is written in the target's digits (`2,583,200 VND`,
  `86.10 EUR`, `0.00093 BTC`), its section names the day of its rates
  ("Currency · rates from 23 Oct"), and the Actions panel copies the
  number alone.

[`src/expression.rs`](src/expression.rs), [`src/dates.rs`](src/dates.rs),
[`src/units.rs`](src/units.rs), [`src/zones.rs`](src/zones.rs) and
[`src/colour.rs`](src/colour.rs) say exactly what each reads, and
[`src/currency.rs`](src/currency.rs) the currencies. It is a *root
provider*: the answer appears in root search, and the package has no
screen of its own.

## Exchange rates

Answers never wait for the network: a currency answer is computed from the
rates the calculator keeps in its cache, and works offline once they have
been fetched. They are fetched by the package's second command, **Refresh
Currency Rates**, which runs in the background as a continuing service:
when there are no rates, then once the ones kept are a day old (it looks
every hour, and tries again an hour after a fetch that failed), and at once
when you run it from root search. Before any rates have been fetched, a
currency query says they are not loaded yet.
[`src/rates.rs`](src/rates.rs) holds the sources, the kept form and the
timing.

The rates come from
[fawazahmed0's currency API](https://github.com/fawazahmed0/exchange-api)
on jsDelivr (no key, about 340 currencies with crypto, updated daily, its
data in the public domain under CC0), and when it fails from
[ExchangeRate-API](https://www.exchangerate-api.com)'s open access endpoint
(no key, about 165 currencies, updated daily), whose terms ask for
attribution: an answer computed from its rates says
"rates from … by Exchange Rate API" —
[Rates By Exchange Rate API](https://www.exchangerate-api.com). Both are
reached over HTTPS through Pane, which lists the calculator as using the
network.

`pane.json` names the same component twice, `dist/calculator.wasm` for the
calculator and `dist/rates/calculator.wasm` for the refresh: Pane runs one
call at a time in the instance of a component file, so with a file of its
own a fetch waiting on the network never holds an answer behind it.

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

    https://github.com/pane-app/calculator@v0.6.4

or, from a terminal, `pane --install git:github.com/pane-app/calculator@v0.6.4`.
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
`pane.json`, the images and the built component (`dist/calculator.wasm`
and its copy `dist/rates/calculator.wasm`), and it
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
`dist/calculator.wasm` and `dist/rates/calculator.wasm` for a revision
users can install — what the Release workflow does for every component
`pane.json` names. `rust-toolchain.toml` pins the toolchain Pane builds its
own extensions with. `cargo test` runs the calculator's tests on the
computer that builds it: the language needs neither Pane's SDK nor
WebAssembly.

The component builds against Pane's Rust SDK,
[`pane-extension`](https://crates.io/crates/pane-extension), from crates.io
(`pane-extension = "0.2"` in [`Cargo.toml`](Cargo.toml)); building it needs
no Pane checkout.

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
