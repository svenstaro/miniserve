# miniserve - CLI tool to serve files and dirs over HTTP

**For when you really just want to serve some files over HTTP right now!**

[![Build Status](https://travis-ci.org/svenstaro/miniserve.svg?branch=master)](https://travis-ci.org/svenstaro/miniserve)
[![AUR](https://img.shields.io/aur/version/miniserve.svg)](https://aur.archlinux.org/packages/miniserve/)
[![Crates.io](https://img.shields.io/crates/v/miniserve.svg)](https://crates.io/crates/miniserve)
[![dependency status](https://deps.rs/repo/github/svenstaro/miniserve/status.svg)](https://deps.rs/repo/github/svenstaro/miniserve)
[![license](http://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/svenstaro/miniserve/blob/master/LICENSE)

## How to use

### Serve a directory:

    miniserve linux-distro-collection/

### Serve a single file:

    miniserve linux-distro.iso

### Require username/password:

    miniserve --auth joe:123 unreleased-linux-distros/

## Features

- Easy to use
- Just works: Correct MIME types handling out of the box
- Single binary drop in with no extra dependencies required
- Authentication support with username and password
- Mega fast and highly parallel (thanks to [Rust](https://www.rust-lang.org/) and [Actix](https://actix.rs/))

## How to install

**On Linux**: Download `miniserve-linux` from [the releases page](https://github.com/svenstaro/miniserve/releases) and run

    chmod +x miniserve-linux
    ./miniserve-linux

**On OSX**: Download `miniserve-osx` from [the releases page](https://github.com/svenstaro/miniserve/releases) and run

    chmod +x miniserve-osx
    ./miniserve-osx

**On Windows**: Download `miniserve-win.exe` from [the releases page](https://github.com/svenstaro/miniserve/releases) and double click it.

**With Cargo**: If you have a somewhat recent version of Rust and Cargo installed, you can run

    cargo install miniserve
    miniserve

## Why to use this over alternatives?

- darkhttpd: Not easily available on Windows and it's not as easy as download and go
- Python built-in webserver: Need to have Python installed and it's low performance and also doesn't do correct MIME type handling in some cases
