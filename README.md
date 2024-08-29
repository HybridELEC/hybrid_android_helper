# Hybrid Android helper

**WORK IN PROGRESS, DO NOT USE**

A helper program that encapsules all root operations needed by HybridELEC Rebooter.

This includes but is not limited to:
- Find the partitions on which CoreELEC and EmuELEC system images are stored on
- Update u-boot envs to next boot to on-eMMC CoreELEC or EmuELEC
- Reboot in various ways

All these happen purely in user space, without external binary or library dependency. The program understands FATFS and u-boot env by itself.

## Build

In most cases this should be built as a staticlly-linked binary called by HybridELEC Rebooter. For maximum compatibility it's recommended to build it against ARMv6 hard-float MUSL toolchain.

### Cross-complication toolchain setup

**This is only needed before first build**

Install both rustup and ARM linker, on Arch Linux:
```sh
sudo pacman -Syu rustup arm-none-eabi-binutils
```

Configure stable as the default toolchain
```sh
rustup default stable
```

Use rustup to add an ARM hard-float MUSL toolchain
```sh
rustup target add arm-unknown-linux-musleabihf
```

Add the following section to your `~/.cargo/config.toml` so cargo knows which linker to use for the ARM hard-float MUSL toolchain
```toml
[target.arm-unknown-linux-musleabihf]
linker = "arm-none-eabi-ld.gold"
```

### Cross build
```sh
cargo build --release --target arm-unknown-linux-musleabihf
```

The built binary would be `target/arm-unknown-linux-musleabihf/release/hybrid_android_helper`

## License
**hybrid_android_helper**, HybridELEC Android helper

Copyright (C) 2024-present Guoxin "7Ji" Pu

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as
published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.