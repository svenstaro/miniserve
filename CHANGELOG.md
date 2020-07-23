# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->

## [Unreleased] - ReleaseDate
- Added prebuilt binaries for AARCH64, ARMv7, and ARM [#350](https://github.com/svenstaro/miniserve/pull/350)

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
[Unreleased]: https://github.com/svenstaro/miniserve/compare/v0.8.0...HEAD
[0.8.0]: https://github.com/svenstaro/miniserve/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/svenstaro/miniserve/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/svenstaro/miniserve/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/svenstaro/miniserve/compare/v0.4.0...v0.5.0
