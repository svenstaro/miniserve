# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->

## [Unreleased] - ReleaseDate
- Skip hash calculation when crypto.subtle is not available [#1507](https://github.com/svenstaro/miniserve/pull/1507) (thanks @outloudvi)

## [0.31.0] - 2025-06-27
- Fix filtering symlinks when hosting WebDAV [#1502](https://github.com/svenstaro/miniserve/pull/1502) (thanks @ahti)
- Enable renaming during file upload if duplicate exists [#1453](https://github.com/svenstaro/miniserve/pull/1453) (thanks @Atreyagaurav)

## [0.30.0] - 2025-06-26
- Add `--file-external-url` to generate links pointing to another server [#1492](https://github.com/svenstaro/miniserve/pull/1492) (thanks @jankeymeulen)
- Add date pill and sort links for mobile views [#1473](https://github.com/svenstaro/miniserve/pull/1473) (thanks @Flat)
- Add upload progress bar and allow for multiple concurrent file uploads [#1431](https://github.com/svenstaro/miniserve/pull/1431) (thanks @AlecDivito)
- Add `--size-display` to allow for toggling file size display between `human` and `exact` [#1261](https://github.com/svenstaro/miniserve/pull/1261) (thanks @Lzzzzzt)
- Add well-known healthcheck route at `/__miniserve_internal/healthcheck` (of `/<prefix>/__miniserve_internal/healthcheck` when using `--route-prefix`)
- Add asynchronous recursive directory size counting [#1482](https://github.com/svenstaro/miniserve/pull/1482)
- Add link to miniserve GitHub page to footer
- Add `--directory-size` flag to enable directory size counting
- Fix --no-symlinks not filtering files and dirs nested in symlinks [#1495](https://github.com/svenstaro/miniserve/pull/1495) (thanks @ahti)

## [0.29.0] - 2025-02-06
- Make URL encoding fully WHATWG-compliant [#1454](https://github.com/svenstaro/miniserve/pull/1454) (thanks @cyqsimon)
- Fix `OVERWRITE_FILES` env var not being prefixed by `MINISERVE_` [#1457](https://github.com/svenstaro/miniserve/issues/1457)
- Change `font-weight` of regular files to be `normal` to improve readability [#1471](https://github.com/svenstaro/miniserve/pull/1471) (thanks @shaicoleman)
- Add webdav support [#1415](https://github.com/svenstaro/miniserve/pull/1415) (thanks @ahti)
- Move favicon and css to stable, non-random routes [#1472](https://github.com/svenstaro/miniserve/pull/1472) (thanks @ahti)

## [0.28.0] - 2024-09-12
- Fix wrapping text in mobile view when the file name too long [#1379](https://github.com/svenstaro/miniserve/pull/1379) (thanks @chaibiq)
- Fix missing drag-form when dragging file in to browser [#1390](https://github.com/svenstaro/miniserve/pull/1390) (thanks @chaibiq)
- Improve documentation for the --header parameter [#1389](https://github.com/svenstaro/miniserve/pull/1389) (thanks @orwithout)
- Don't show mkdir option when the directory is not upload allowed [#1442](https://github.com/svenstaro/miniserve/pull/1442) (thanks @Atreyagaurav)

## [0.27.1] - 2024-03-16
- Add `Add file and folder symbols` [#1365](https://github.com/svenstaro/miniserve/pull/1365) (thanks @chaibiq)

## [0.27.0] - 2024-03-16
- Add `-C/--compress-response` to enable response compression [1315](https://github.com/svenstaro/miniserve/pull/1315) (thanks @zuisong)
- Refactor errors [#1331](https://github.com/svenstaro/miniserve/pull/1331) (thanks @cyqsimon)
- Add `-I/--disable-inexing` [#1329](https://github.com/svenstaro/miniserve/pull/1329) (thanks @dyc3)

## [0.26.0] - 2024-01-13
- Properly handle read-only errors on Windows [#1310](https://github.com/svenstaro/miniserve/pull/1310) (thanks @ViRb3)
- Use `tokio::fs` instead of `std::fs` to enable async file operations [#445](https://github.com/svenstaro/miniserve/issues/445)
- Add `-S`/`--default-sorting-method` and `-O`/`--default-sorting-order` flags [#1308](https://github.com/svenstaro/miniserve/pull/1308) (thanks @ElliottLandsborough)

## [0.25.0] - 2024-01-07
- Add `--pretty-urls` [#1193](https://github.com/svenstaro/miniserve/pull/1193) (thanks @nlopes)
- Fix single quote display with `--show-wget-footer` [#1191](https://github.com/svenstaro/miniserve/pull/1191) (thanks @d-air1)
- Remove header Content-Encoding when archiving [#1290](https://github.com/svenstaro/miniserve/pull/1290) (thanks @5long)
- Prevent illegal request path from crashing program [#1285](https://github.com/svenstaro/miniserve/pull/1285) (thanks @cyqsimon)
- Fixed issue where serving files with a newline would fail [#1294](https://github.com/svenstaro/miniserve/issues/1294)

## [0.24.0] - 2023-07-06
- Fix ANSI color codes are printed when not a tty [#1095](https://github.com/svenstaro/miniserve/pull/1095)
- Allow parameters to be provided via environment variables [#1160](https://github.com/svenstaro/miniserve/pull/1160)

## [0.23.2] - 2023-04-28
- Build Windows build with static CRT [#1107](https://github.com/svenstaro/miniserve/pull/1107)

## [0.23.1] - 2023-04-17
- Add EC key support [#1080](https://github.com/svenstaro/miniserve/issues/1080)

## [0.23.0] - 2023-03-01
- Update to clap v4
- Show localized datetime [#949](https://github.com/svenstaro/miniserve/pull/949) (thanks @IvkinStanislav)
- Fix sorting breaks subdir downloading [#991](https://github.com/svenstaro/miniserve/pull/991) (thanks @Vam-Jam)
- Fix wget footer [#1043](https://github.com/svenstaro/miniserve/pull/1043) (thanks @Yusuto)

## [0.22.0] - 2022-09-20
- Faster QR code generation [#848](https://github.com/svenstaro/miniserve/pull/848) (thanks @cyqsimon)
- Make `--readme` support not only `README.md` but also `README` and `README.txt` rendered as
  plaintext [#911](https://github.com/svenstaro/miniserve/pull/911) (thanks @Atreyagaurav)
- Change `-u/--upload-files` slightly in the sense that it can now either be provided by itself as
  before or receive a file path to restrict uploading to only that path. Can be provided multiple
  times for multiple allowed paths [#858](https://github.com/svenstaro/miniserve/pull/858) (thanks
  @jonasdiemer)

## [0.21.0] - 2022-09-15
- Fix bug where static files would be served incorrectly when using `--random-route` [#835](https://github.com/svenstaro/miniserve/pull/835) (thanks @solarknight)
- Add `--readme` to render the README in the current directory after the file listing [#860](https://github.com/svenstaro/miniserve/pull/860) (thanks @Atreyagaurav)
- Add more architectures (and also additional container images)

## [0.20.0] - 2022-06-26
- Fixed security issue where it was possible to upload files to locations pointed to by symlinks
  even when symlinks were disabled [#781](https://github.com/svenstaro/miniserve/pull/781) (thanks @sheepy0125)
- Added `--hide-theme-selector` flag to hide the theme selector functionality in the frontend [#805](https://github.com/svenstaro/miniserve/pull/805https://github.com/svenstaro/miniserve/pull/805) (thanks @flamingoodev)
- Added `--mkdir` flag to allow for uploading directories [#781](https://github.com/svenstaro/miniserve/pull/781) (thanks @sheepy0125)

## [0.19.5] - 2022-05-18
- Fix security issue where `--no-symlinks` would only hide symlinks from listing but it would
  still be possible to follow them if the path was known

## [0.19.4] - 2022-04-02
- Fix random route leaking on error pages [#764](https://github.com/svenstaro/miniserve/pull/764) (thanks @steffhip)

## [0.19.3] - 2022-03-15
- Allow to set the accept input attribute to arbitrary values using `-m` and `-M` [#755](https://github.com/svenstaro/miniserve/pull/755) (thanks @mayjs)

## [0.19.2] - 2022-02-21
- Add man page support via `--print-manpage` [#738](https://github.com/svenstaro/miniserve/pull/738)

## [0.19.1] - 2022-02-16
- Better MIME type guessing support due to updated mime_guess

## [0.19.0] - 2022-02-06
- Fix panic when using TLS in some instances [#670](https://github.com/svenstaro/miniserve/issues/670) (thanks @aliemjay)
- Add `--route-prefix` to add a fixed route prefix [#728](https://github.com/svenstaro/miniserve/pull/728) (thanks @aliemjay and @Jikstra)
- Allow tapping the whole row in mobile view [#729](https://github.com/svenstaro/miniserve/pull/729)

## [0.18.0] - 2021-10-26
- Add raw mode and raw mode footer display [#508](https://github.com/svenstaro/miniserve/pull/508) (thanks @Jikstra)
- Add SPA mode [#515](https://github.com/svenstaro/miniserve/pull/515) (thanks @sinking-point)

## [0.17.0] - 2021-09-04
- Print QR codes on terminal [#524](https://github.com/svenstaro/miniserve/pull/524) (thanks @aliemjay)
- Fix mobile layout info pills taking whole width [#591](https://github.com/svenstaro/miniserve/issues/591)
- Fix security exploit when uploading is enabled [#590](https://github.com/svenstaro/miniserve/pull/590) [#518](https://github.com/svenstaro/miniserve/issues/518) (thanks @aliemjay)
- Fix uploading to symlink directories [#590](https://github.com/svenstaro/miniserve/pull/590) [#466](https://github.com/svenstaro/miniserve/issues/466) (thanks @aliemjay)

## [0.16.0] - 2021-08-31
- Fix serving files with backslashes in their names [#578](https://github.com/svenstaro/miniserve/pull/578) (thanks @Jikstra)
- Fix behavior of downloading symlinks by upgrading to actix-web 4 [#582](https://github.com/svenstaro/miniserve/pull/582) [#462](https://github.com/svenstaro/miniserve/issues/462) (thanks @aliemjay)
- List directory if index file not found [#583](https://github.com/svenstaro/miniserve/pull/583) [#275](https://github.com/svenstaro/miniserve/pull/583) (thanks @aliemjay)
- Add special colors for visited links [#521](https://github.com/svenstaro/miniserve/pull/521) (thanks @raffomania)
- Switch from structopt to clap v3 [#587](https://github.com/svenstaro/miniserve/pull/587)

  This enables slightly nicer help output as well as much better completions.
- Fix network interface handling [#500](https://github.com/svenstaro/miniserve/pull/500) [#470](https://github.com/svenstaro/miniserve/issues/470) [#405](https://github.com/svenstaro/miniserve/issues/405) [#422](https://github.com/svenstaro/miniserve/issues/422) (thanks @aliemjay)
- Implement show symlink destination [#542](https://github.com/svenstaro/miniserve/pull/542) [#499](https://github.com/svenstaro/miniserve/issues/499) (thanks @deantvv)
- Fix error page not being correctly themed [#529](https://github.com/svenstaro/miniserve/pull/529) [#588](https://github.com/svenstaro/miniserve/issues/588) (@aliemjay)

## [0.15.0] - 2021-08-27
- Add hardened systemd template unit file to `packaging/miniserve@.service`
- Fix qrcodegen dependency problem [#568](https://github.com/svenstaro/miniserve/issues/568)
- Remove animation on QR code hover (it was kind of annoying as it makes things less snappy)
- Add TLS support [#576](https://github.com/svenstaro/miniserve/pull/576)

## [0.14.0] - 2021-04-18
- Fix breadcrumbs for right-to-left languages [#489](https://github.com/svenstaro/miniserve/pull/489) (thanks @aliemjay)
- Fix URL percent encoding for special characters [#485](https://github.com/svenstaro/miniserve/pull/485) (thanks @aliemjay)
- Wrap breadcrumbs at any char [#496](https://github.com/svenstaro/miniserve/pull/496) (thanks @aliemjay)
- Add separate flags for compressed and uncompressed tar archives [#492](https://github.com/svenstaro/miniserve/pull/492) (thanks @deantvv)
- Bump deps
- Fix Firefox becoming confused when opening a `.gz` file directly [#160](https://github.com/svenstaro/miniserve/issues/160)
- Prefer UTF8 for text responses [#263](https://github.com/svenstaro/miniserve/issues/263)
- Resolve symlinks on directory listing [#479](https://github.com/svenstaro/miniserve/pull/479) (thanks @aliemjay)

## [0.13.0] - 2021-03-28
- Change default log level to `Warn`
- Change some messages a bit to be more clear
- Add `--print-completions` to print shell completions for various supported shells [#482](https://github.com/svenstaro/miniserve/pull/482) (thanks @rouge8)
- Don't print some messages if not attached to an interactive terminal
- Refuse to start if not attached to interactive terminal and no explicit path is provided

  This is a security consideration as you wouldn't want to run miniserve without an explicit path
  as a service. You could end up serving `/` or `/root` in case those working directories are set.

## [0.12.1] - 2021-03-27
- Fix QR code not showing when using both `--random-route` and `--qrcode` [#480](https://github.com/svenstaro/miniserve/pull/480) (thanks @rouge8)
- Add FreeBSD binaries

## [0.12.0] - 2021-03-20
- Add option `-H`/`--hidden` to show hidden files
- Start instantly in case an explicit index is chosen
- Fix DoS issue when deliberately sending unconforming URL paths
- Add footer [#456](https://github.com/svenstaro/miniserve/pull/456) (thanks @levaitamas)
- Switched from failure to thiserror for error handling

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
[Unreleased]: https://github.com/svenstaro/miniserve/compare/v0.31.0...HEAD
[0.31.0]: https://github.com/svenstaro/miniserve/compare/v0.30.0...v0.31.0
[0.30.0]: https://github.com/svenstaro/miniserve/compare/v0.29.0...v0.30.0
[0.29.0]: https://github.com/svenstaro/miniserve/compare/v0.28.0...v0.29.0
[0.28.0]: https://github.com/svenstaro/miniserve/compare/v0.27.1...v0.28.0
[0.27.1]: https://github.com/svenstaro/miniserve/compare/v0.27.0...v0.27.1
[0.27.0]: https://github.com/svenstaro/miniserve/compare/v0.26.0...v0.27.0
[0.26.0]: https://github.com/svenstaro/miniserve/compare/v0.25.0...v0.26.0
[0.25.0]: https://github.com/svenstaro/miniserve/compare/v0.24.0...v0.25.0
[0.24.0]: https://github.com/svenstaro/miniserve/compare/v0.23.2...v0.24.0
[0.23.2]: https://github.com/svenstaro/miniserve/compare/v0.23.1...v0.23.2
[0.23.1]: https://github.com/svenstaro/miniserve/compare/v0.23.0...v0.23.1
[0.23.0]: https://github.com/svenstaro/miniserve/compare/v0.22.0...v0.23.0
[0.22.0]: https://github.com/svenstaro/miniserve/compare/v0.21.0...v0.22.0
[0.21.0]: https://github.com/svenstaro/miniserve/compare/v0.20.0...v0.21.0
[0.20.0]: https://github.com/svenstaro/miniserve/compare/v0.19.5...v0.20.0
[0.19.5]: https://github.com/svenstaro/miniserve/compare/v0.19.4...v0.19.5
[0.19.4]: https://github.com/svenstaro/miniserve/compare/v0.19.3...v0.19.4
[0.19.3]: https://github.com/svenstaro/miniserve/compare/v0.19.2...v0.19.3
[0.19.2]: https://github.com/svenstaro/miniserve/compare/v0.19.1...v0.19.2
[0.19.1]: https://github.com/svenstaro/miniserve/compare/v0.19.0...v0.19.1
[0.19.0]: https://github.com/svenstaro/miniserve/compare/v0.18.0...v0.19.0
[0.18.0]: https://github.com/svenstaro/miniserve/compare/v0.17.0...v0.18.0
[0.17.0]: https://github.com/svenstaro/miniserve/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/svenstaro/miniserve/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/svenstaro/miniserve/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/svenstaro/miniserve/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/svenstaro/miniserve/compare/v0.12.1...v0.13.0
[0.12.1]: https://github.com/svenstaro/miniserve/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/svenstaro/miniserve/compare/v0.11.0...v0.12.0
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
