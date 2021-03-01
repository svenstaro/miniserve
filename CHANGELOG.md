# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->

## [Unreleased] - ReleaseDate
- Add footer [#456](https://github.com/svenstaro/miniserve/pull/456) (thanks @levaitamas)

## [0.11.0] - 2021-02-28
- Add binaries for more architectures
- Upgrade lockfile which fixes some security issues
- Allow multiple file upload [#434](https://github.com/svenstaro/miniserve/pull/434) (thanks @mhuesch)
- Allow for setting custom headers via `--header` [#452](https://github.com/svenstaro/miniserve/pull/452) (thanks @deantvv)

## [0.10.4] - 2021-01-05
- Add `--dirs-first`/`-D` option to list directories first [#423](https://github.com/svenstaro/miniserve/pull/423) (thanks @levaitamas)

## [0.10.3] - 2020-11-09
- Actually fix publish workflow

## [0.10.2] - 2020-11-09
- Fix publish workflow

## [0.10.1] - 2020-11-09
- Now compiles on stable! :D

## [0.10.0] - 2020-10-02
- Add embedded favicon [#364](https://github.com/svenstaro/miniserve/issues/364)
- Add `--title` option which can be used to set the page title [#378](https://github.com/svenstaro/miniserve/pull/378) (thanks @ahti)
- Default title is now the same host received in the request [#378](https://github.com/svenstaro/miniserve/pull/378) (thanks @ahti)
- Client-side color-scheme handling [#380](https://github.com/svenstaro/miniserve/pull/380) (thanks @ahti)

## [0.9.0] - 2020-09-16
- Added prebuilt binaries for AARCH64, ARMv7, and ARM [#350](https://github.com/svenstaro/miniserve/pull/350)
- Remove percent-encoding in heading and title [#362](https://github.com/svenstaro/miniserve/pull/362) (thanks @ahti)
- Make name ordering case-insensitive [#362](https://github.com/svenstaro/miniserve/pull/362) (thanks @ahti)
- Give name column more space [#362](https://github.com/svenstaro/miniserve/pull/362) (thanks @ahti)
- Fix double-escaping [#354](https://github.com/svenstaro/miniserve/issues/354)
- Upgrade to actix-web 3.0
- Fix time display for files created "now" [#373](https://github.com/svenstaro/miniserve/pull/373) (thanks @imp and @KevCui)

## [0.8.0] - 2020-07-22
- Accept port 0 to find a random free port and use that [#327](https://github.com/svenstaro/miniserve/pull/327) (thanks @parrotmac)
- Show QR code in interface [#330](https://github.com/svenstaro/miniserve/pull/330) (thanks @wyhaya)
- Ported to actix-web 2 and futures 0.3 [#343](https://github.com/svenstaro/miniserve/pull/343) (thanks @equal-l2)

## [0.7.0] - 2020-05-14
- Add zip archiving [#297](https://github.com/svenstaro/miniserve/pull/297) (thanks @marawan31)

## [0.6.0] - 2020-03-14
- Add option to disable archives [#235](https://github.com/svenstaro/miniserve/pull/235) (thanks @DamianX)
- Fix minor bug when using `--random-route` [#219](https://github.com/svenstaro/miniserve/pull/219)
- Add a default index serving option [#189](https://github.com/svenstaro/miniserve/pull/189)

## [0.5.0] - 2019-06-24
- Add streaming download of tar archives (thanks @gyscos)
- Add support for hashed passwords (thanks @KSXGitHub)
- Add support for multiple auth flags (thanks @KSXGitHub)
- Some theme related bug fixes (thanks @boastful-squirrel)

<!-- next-url -->
[Unreleased]: https://github.com/svenstaro/miniserve/compare/v0.11.0...HEAD
[0.11.0]: https://github.com/svenstaro/miniserve/compare/v0.10.4...v0.11.0
[0.10.4]: https://github.com/svenstaro/miniserve/compare/v0.10.3...v0.10.4
[0.10.3]: https://github.com/svenstaro/miniserve/compare/v0.10.2...v0.10.3
[0.10.2]: https://github.com/svenstaro/miniserve/compare/v0.10.1...v0.10.2
[0.10.1]: https://github.com/svenstaro/miniserve/compare/v0.10.0...v0.10.1
[0.10.0]: https://github.com/svenstaro/miniserve/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/svenstaro/miniserve/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/svenstaro/miniserve/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/svenstaro/miniserve/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/svenstaro/miniserve/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/svenstaro/miniserve/compare/v0.4.0...v0.5.0
