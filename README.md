# QPG Desktop Application

<details>
  <summary><strong>Table of Contents</strong></summary>

- [Building](#building)
- [Running](#running)
- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
- [Development](#development)

</details>

## Building

Refer to the [Releases](https://github.com/quantum-proximity-gateway/desktop-app/releases) page to download and build the application.

> NOTE: For proper command execution, you must run it on a GNOME-based GUI (any flavour of Linux e.g. Kali-Linux).

## Running

- Ensure ollama is running and active

- Run the built application

	- This can be done either by finding the app and double clicking on it OR
	
	- By opening the terminal, navigating to the directory with the file and running `./<name of the file>`

---

<br />

In case you want to change the code, below are the instructions to install and use the application without the built versions.

## Requirements

- NodeJS

- Cargo

- Ollama

## Installation

Navigate to the `QPG-Application/` directory.

Then, to install the dependencies, run:

```bash
npm install
```

## Usage

Firstly, ensure that ollama is running. If it is not, run:

```bash
ollama serve
```

> NOTE: You can check if ollama is running by trying to access `http://localhost:11434` in a browser.

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
