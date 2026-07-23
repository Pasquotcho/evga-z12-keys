# EVGA Z12 Keys

Configure the five EVGA Z12 keyboard E-keys on Linux. Every change is saved immediately
to the keyboard's active onboard profile.

Tested device: EVGA Z12 with USB ID `3842:2622`.

## Install

Download the binary from the [v1.0.0 release](../../releases/tag/v1.0.0), then:

```sh
sudo install -Dm755 evga-z12-keys /usr/local/bin/evga-z12-keys
```

Build from source on Arch Linux:

```sh
sudo pacman -S --needed rust hidapi
cargo build --release
sudo install -Dm755 target/release/evga-z12-keys /usr/local/bin/evga-z12-keys
```

## Usage

Show the current E-key assignments:

```sh
sudo evga-z12-keys status
```

Assign E1 through E5 to F13 through F17:

```sh
sudo evga-z12-keys auto
```

Assign an individual key:

```sh
sudo evga-z12-keys set E1 F17
sudo evga-z12-keys set E2 A
```

Valid target names are listed in [`keys.yaml`](keys.yaml). Unknown names are
rejected before the keyboard is opened, and nothing is written.

Disable one or all E-keys:

```sh
sudo evga-z12-keys disable E1
sudo evga-z12-keys disable all
```

Licensed under the [MIT License](LICENSE).
