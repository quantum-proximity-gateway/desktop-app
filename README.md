# QPG Desktop Application

<details>
  <summary><strong>Table of Contents</strong></summary>

- [Building](#building)
- [Running](#running)
- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
- [Development](#development)
- [Misc.](#misc)
- [License](#license)

</details>

## Building

Refer to the [Releases](https://github.com/quantum-proximity-gateway/desktop-app/releases) page to download and build the application.

> NOTE: For proper command execution and application startup, you must run it on a GNOME-based GUI (any flavour of Linux e.g. Kali-Linux).

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

To build the application for production, run:

```bash
npm run tauri build
```

The built application can be found in `QPG-Application/src-tauri/target/release/`.

## Development

To start the application in development mode instead, run:

```bash
npm run tauri dev
```

## Misc.

- If you want to change the ollama and server URLs, they are located in `QPG-Application/.env.example`.

- Instead of using the commands listed above individually, you can run `make dev` or `make build` from the root directory of this project to install the necessary packages and run/build the project.

## License

This project is licensed under the terms of the MIT license. Refer to [LICENSE](LICENSE) for more information.
