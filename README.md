# QPG Desktop Application

## Building

Refer to the [Releases](https://github.com/quantum-proximity-gateway/desktop-app/releases/tag/app-v0.1.0) page to download and build the application.

> NOTE: For proper command execution, you must run it on a GNOME-based GUI (any flavour of Linux e.g. Kali-Linux)

---

<br />

In case you want to change the code, below are the instructions to install and use the application without the built versions.

## Requirements

- NodeJS

- Cargo

## Installation

Navigate to the `QPG-Application/` directory.

Then, to install the dependencies, run:

```bash
npm install
```

## Usage

To start the application, run:

```bash
npm run tauri dev
```

## Development

To build the application for production, run:

```bash
npm run tauri build
```

The built application can be found in `QPG-Application/src-tauri/target/release/`.
