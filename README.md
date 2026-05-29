# Open Remote URL

English | [简体中文](README.zh_CN.md) | [日本語 (Original)](README.ja_JP.md) | [日本語 (Non-technical)](README.ja_EZ.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

> ⚠️ **Note on Translations**
> Since this document was AI-translated from Japanese, some parts may be inaccurate or difficult to understand. If you find any parts unclear, please translate the [Japanese (Original) version](README.ja_JP.md) and refer to it instead.

> 💡 **About Project Development**
> Everything in this project except the concept was developed by the AI models **Gemini 3.5 Flash** and **Claude Sonnet 4.6**. For details, see [About Project Development](#about-project-development).

> ⚠️ **Current Operational Status**
> - **The Linux version has not been tested at this time.**
> - The Windows and macOS versions have been verified for URL forwarding only (opening a URL in the Host's browser). The reverse proxy functionality (OAuth callback forwarding) has not been verified.
> - The repository was made public before completion in order to avoid GitHub Actions CI/CD build time limits.

A system that forwards web page display and OAuth login authentication to a browser on another device (Host) for centralized management.

It intercepts browser open requests on a local **Client** machine (inside VMs, Docker containers, or secondary PCs), forwards the URL to the **Host** machine, and opens the website in the Host's browser.

### Solved Problems & Use Cases

- **Smooth OAuth Logins in VM / Docker Container Dev Environments**
    - When running CLI commands such as `wrangler login` or `supabase login` in browser-less (or clean) environments like Docker containers, the tool tries to launch a browser. You would normally copy the URL and open it in the Host's browser, but the OAuth callback to a local redirect port such as `http://localhost:8976/oauth/callback` cannot be received after login, causing authentication to fail midway.
    - This tool not only detects when a URL is opened on the Client and displays it on the Host, but also **automatically scans for ports that were allocated at the time the URL was opened (history up to 15 seconds prior) and ports newly opened after the URL was launched (checked for up to 3 seconds), and automatically establishes a temporary reverse proxy from the Host to the Client**. This allows the containerized development environment to automatically receive the authentication token simply by clicking the authentication button in the Host's browser, completing the login seamlessly.

- **Unified Browser Sessions & Credentials in 2-PC Setups (Streaming/Gaming)**
    - When clicking Discord or social media links on a gaming/sub PC (Client), they automatically open in the browser of your main PC (Host)—where your login credentials, password manager, and browser extensions are all configured.
    - This eliminates the need to repeatedly log in on the secondary PC, and is also more secure since login credentials are never stored on the sub PC.

### ※ Development Concept

This tool was born from the developer's strong personal need: "When logging into accounts on a gaming PC, I want the login page to open on my main PC." Pull requests that break or deviate from this core concept and behavior will not be approved.

## Table of Contents

- [How It Works](#how-it-works)
- [Configuration](#configuration)
    - [Host Config (host/inactive.env)](#host-config-hostinactiveenv)
    - [Client Config (client/inactive.env)](#client-config-clientinactiveenv)
- [Installation](#installation)
- [Status Check](#status-check)
- [Uninstallation](#uninstallation)
- [About Project Development](#about-project-development)

---

## Flowchart

![Flowchart](docs/flowchart.webp)

1. **URL Interception**: Intercepts browser open requests when a URL is opened on the Client.
2. **URL Display**: The Client daemon immediately forwards only the URL to the Host and opens it in the Host's browser.
3. **Port Detection & Proxy Addition**: Sends the list of ports allocated on the Client up to 15 seconds before the URL was opened to the Host. The Host automatically spawns temporary proxy servers for those ports.
4. **Dynamic Monitoring for Additional Ports**: After the URL is opened, monitors the Client for 3 seconds for any newly added ports, and spawns additional proxies on the Host side if found.
5. **Automatic Proxy Cleanup**: When a proxied port is closed on the Client side, a delete request is immediately sent to the Host to safely shut down the unnecessary proxy server (any proxies that were not deleted are also automatically removed after a 5-minute timeout).

---

## Download

[Download the latest release here](https://github.com/NaeCqde/open-remote-url/releases/latest)

Download the `.zip` file matching your OS and architecture.

## Configuration

Configuration is loaded from system environment variables and the `.env` file.

### Priority and Configuration File Search Paths

At runtime, the configuration file (`.env`) is searched and loaded in the following order of priority:

1. **OS-Specific Configuration Folder** (Loaded from here once installed)
    - **Windows**: `%APPDATA%\open-remote-url\<client|host>\.env`
    - **macOS**: `/Users/<user>/Applications/OpenRemoteURLClient.app/.env` (or `OpenRemoteURLHost.app/.env`)
    - **Linux**: `~/.config/open-remote-url/<client|host>/.env`
2. **Current Working Folder or its Parent Folders**
    - If the `.env` file does not exist in the OS-specific folder above, the program looks for a `.env` file in the folder where it was executed (or its parent folders). This is useful during development or when running the tool manually.

_Note: If a setting is defined in both system environment variables and the `.env` file, the value in the **`.env` file takes precedence (overrides)**._

### Installation Behavior

The distribution package includes an `inactive.env` template.
Running the installer script automatically copies the `inactive.env` from the installation folder to the OS-specific configuration folder and renames it to `.env`.

- **Before running the installer**, please open the `inactive.env` file in the package folder using a text editor, customize the settings for your environment, and save it.
- If the installer is run without `inactive.env` present in the folder, a default `.env` file containing default configuration values will be generated automatically in the configuration folder.

### Host Config (`host/inactive.env`)

```env
LISTEN=0.0.0.0:4000
PASSPHRASE=some-shared-secret
```

| Variable | Description | Default |
|---|---|---|
| `LISTEN` | Bind address and port (`<host>:<port>`). | `0.0.0.0:4000` |
| `PASSPHRASE` | Shared passphrase. Leave empty to disable authentication. | _(empty)_ |

### Client Config (`client/inactive.env`)

```env
LISTEN=0.0.0.0:3000
HOST_URL=http://<host_ip>:4000
RELAY_URL=http://<client_ip>:3000
PASSPHRASE=some-shared-secret
```

| Variable | Description | Default |
|---|---|---|
| `LISTEN` | Bind address and port for the Client daemon (`<host>:<port>`). | `0.0.0.0:3000` |
| `HOST_URL` | URL of the remote Host daemon. Supports `http://` and `https://` (TLS via rustls, no OpenSSL required). | `http://localhost:4000` |
| `RELAY_URL` | URL that the Host uses to call back into this Client for reverse proxying. Must be reachable from the Host machine — use the Client's LAN/Tailscale IP, not `0.0.0.0`. | `http://localhost:3000` |
| `PASSPHRASE` | Key matching the Host's passphrase. Leave empty to disable authentication. | _(empty)_ |

---

## Installation

The tool is installed to OS-specific directories as detailed below, and is configured to automatically launch the background daemon on OS boot.

- **Windows**: `%LOCALAPPDATA%\Programs\open-remote-url\<client|host>\`
- **macOS**: `~/Applications/OpenRemoteURL<Client|Host>.app/`
- **Linux**: `~/.local/bin/open-remote-url/<client|host>/`

**⚠ Before running the installer, open `inactive.env` inside the folder in a text editor, write your settings, and save the file.**

To register and start the daemons:

- **Windows**: Double-click the executable (`open-remote-url-client.exe` or `open-remote-url-host.exe`) to open the GUI Control Panel, then click the **Install Service** button.
- **macOS**: Double-click the app bundle (`OpenRemoteURLClient.app` or `OpenRemoteURLHost.app`) to open the GUI Control Panel, then click the **Install Service** button.
- **Linux**:
  - **Desktop Environment (with GUI)**: Double-click the executable (`open-remote-url-client` or `open-remote-url-host`) to open the GUI Control Panel, then click the **Install Service** button.
  - **CLI/Headless Environment (without GUI)**: Run the setup script in the release folder: `./install.sh`

_After installation, set **Open Remote URL Client** as the default web browser in your Client OS settings:_
- **Windows**: Settings → Apps → Default apps → Web browser
- **macOS**: System Settings → Desktop & Dock (or General) → Default web browser → **Open Remote URL Client**
- **Linux**: `xdg-settings set default-web-browser open-remote-url-client.desktop` (or use your DE's settings)

---

## Verification

Double-clicking the app bundle (or executable) opens the **GUI Control Panel**, which shows:
- Installation status (Installed / Not Installed)
- Daemon running status (Running / Stopped) — updates automatically after pressing Install
- Executable path and configuration file path
- Buttons: **Install Service**, **Uninstall Service**, **Open Settings Folder**

You can also run the binary from the command line for a quick status check:

- **Host**:

```bash
$ ./open-remote-url-host
Open Remote URL - Host Status

[Status]
- Installed:  Yes
- Running:    Yes
- Listen:     http://0.0.0.0:4000/
- Auth:       Enabled
- Executable: /Users/<username>/Applications/OpenRemoteURLHost.app/Contents/MacOS/open-remote-url-host
- Config:     /Users/<username>/Applications/OpenRemoteURLHost.app/.env

[Usage]
- To install / start host:
  Double-click the OpenRemoteURLHost.app bundle (macOS) or the executable (Windows/Linux GUI) to open GUI Control Panel, or run `./install.sh` (Linux CLI)

- To uninstall / clean registrations:
  Open GUI Control Panel and click Uninstall, or run `./uninstall.sh` (Linux CLI)
```

- **Client**:

```bash
$ ./open-remote-url-client
Open Remote URL - Client Status

[Status]
- Installed:  Yes
- Running:    Yes
- Listen:     http://0.0.0.0:3000/
- RELAY:      http://192.168.1.20:3000/
- HOST:       http://192.168.1.10:4000/
- Auth:       Enabled
- Executable: /Users/<username>/Applications/OpenRemoteURLClient.app/Contents/MacOS/open-remote-url-client
- Config:     /Users/<username>/Applications/OpenRemoteURLClient.app/.env

[Usage]
- To install / start client:
  Double-click the OpenRemoteURLClient.app bundle (macOS) or the executable (Windows/Linux GUI) to open GUI Control Panel, or run `./install.sh` (Linux CLI)

- To uninstall / clean registrations:
  Open GUI Control Panel and click Uninstall, or run `./uninstall.sh` (Linux CLI)
```

---

## Uninstallation

To completely remove autostart registrations, browser associations, plist files, and systemd user service settings and terminate the background process:

- **Windows**: Open the GUI Control Panel by double-clicking the executable and click the **Uninstall Service** button.
- **macOS**: Open the GUI Control Panel by double-clicking the `.app` bundle and click the **Uninstall Service** button.
- **Linux**:
  - **Desktop Environment (with GUI)**: Open the GUI Control Panel by double-clicking the executable and click the **Uninstall Service** button.
  - **CLI/Headless Environment (without GUI)**: Run `./uninstall.sh` in the release folder.

---

## About Project Development

Everything in this project except the concept (source code implementation, refactoring, and documentation) was developed and organized by the AI models **Gemini 3.5 Flash** and **Claude Sonnet 4.6**, which have high-level design, coding, and structuring capabilities.

Additionally, the system architecture diagram (flowchart) was created by feeding a handwritten sketch drawn on iPad Freeform into **Gemini 3.1 Flash (Multimodal Feature)**, which neatly transcribed the text and adjusted the box placement and overall layout (with considerable trial and error in prompt tuning).

These AI models are available on Antigravity and Antigravity IDE provided by Google.
This project was completed using the Google One AI Pro plan (approx. $20 / month) and the Claude Pro plan (approx. $25 / month).

- **Development Started (First Prompt)**: 2026/05/26 15:35 JST
- **Last Updated**: 2026/05/30 JST

Note: Since the prompt input and code verification were done in parallel while watching anime, and Gemini's response was extremely fast, it could have been completed in an even shorter time if focused solely on development.
