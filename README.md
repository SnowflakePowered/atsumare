# 集まれ atsumare

Respectful DAT downloader for use with [shiratsu](https://github.com/SnowflakePowered/shiratsu). Supports [TOSEC](https://www.tosecdev.org/), [DAT-o-Matic](https://datomatic.no-intro.org/), and [redump](http://redump.org/).

## Usage

Optionally set the following environment variables. If these variables are set, atsumare will attempt to get an authenticated sessions to fetch private DATs.

```
ATSUMARE_DOM_USER=
ATSUMARE_DOM_PASS=
ATSUMARE_REDUMP_USER=
ATSUMARE_REDUMP_PASS=
```

```
$ atsumare (--datomatic | --redump | --tosec )
```
## Building

This is a pure Rust application with no external compilation dependencies besides Cargo and rustc. Simply clone the repository, and run

```bash
cargo build
```