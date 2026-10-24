# Lumen

A fast, customizable Android launcher written in Rust.

- **Presets:** Nothing, Pixel, iOS. One tap restyles icons, clock, dock, drawer and colors; then change anything.
- **Fast:** Rust + OpenGL (egui). App icons load on worker threads and live as GPU textures.
- **Updates:** Settings → Updates looks at the newest GitHub release, downloads the APK and hands it to Android's installer (same approach as [Hearth](https://github.com/veelvoer/hearth)).

## Install

Download `Lumen-<version>.apk` from [Releases](https://github.com/veelvoer/lumen/releases), install it, then Settings → Phone → *Make Lumen the home app*.
Android 8.0+, arm64.

## Build

Needs Rust with the `aarch64-linux-android` target, `cargo-ndk`, the Android SDK (build-tools, platform 35, NDK) and a JDK 17.

```sh
scripts/build-apk.sh        # signs with the debug key unless LUMEN_KS* are set
```

## Release

1. Bump `version` in `Cargo.toml`, add `release-notes/<version>.md`.
2. `git tag v<version> && git push --tags`. CI builds, signs and publishes the APK.

Only a few lines of Java exist (`java/`): the install-result receiver and one UI-thread window call. Everything else is Rust.

## Not done yet

Widgets, folders, icon packs, notification badges, gestures beyond swipe up/down, drag-to-rearrange.

MIT licensed.
