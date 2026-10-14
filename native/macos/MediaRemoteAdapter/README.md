# MediaRemoteAdapter

Version 0.7.7 of [ungive/mediaremote-adapter](https://github.com/ungive/mediaremote-adapter) (commit `e3ff502`), with no changes. The license is BSD 3-Clause (`LICENSE`).

Sayso uses it on macOS to pause the media that plays while the user dictates (`audio.pause_media`). Since macOS 15.4, an app that Apple did not sign cannot read or control the media of other apps through the private MediaRemote framework. `/usr/bin/perl` can, so Sayso runs `bin/mediaremote-adapter.pl` with perl, and the script loads this framework into perl.

`scripts/bundle.sh` builds the framework with CMake and puts it in `Sayso.app/Contents/Frameworks`. The script goes to `Sayso.app/Contents/Resources`. The Rust side is `crates/sayso-platform-macos/src/media.rs`.

To update, copy `CMakeLists.txt`, `LICENSE`, `bin`, `include`, and `src` from a new release, and change the version above.
