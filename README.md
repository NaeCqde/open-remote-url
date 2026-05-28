# Open Remote URL

English | [简体中文](README.zh_CN.md) | [日本語 (Original)](README.ja_JP.md) | [日本語 (Non-technical)](README.ja_EZ.md)

> ⚠️ **Note on Translations**
> Since this document was AI-translated from Japanese, some parts may be inaccurate or difficult to understand. If you find any parts unclear, please translate the [Japanese (Original) version](README.ja_JP.md) and refer to it instead.

> 💡 **About Project Development**
> Most of this project was developed by the AI model **Gemini 3.5 Flash**. For details, see [About Project Development](#about-project-development).

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

*Note: If a setting is defined in both system environment variables and the `.env` file, the value in the **`.env` file takes precedence (overrides)**.*

### Installation Behavior

The distribution package includes an `inactive.env` template.
Running the installer script automatically moves the `inactive.env` from the installation folder to the OS-specific configuration folder and renames it to `.env` (copied, then the original `inactive.env` is deleted).

- **Before running the installer**, please open the `inactive.env` file in the package folder using a text editor, customize the settings for your environment, and save it.
- If the installer is run without `inactive.env` present in the folder, a default `.env` file containing default configuration values will be generated automatically in the configuration folder.

### Host Config (`host/inactive.env`)

```env
HOST=0.0.0.0
PORT=8080
PASSPHRASE=some-shared-secret
```

- `HOST`: Bind IP address (default: `0.0.0.0`)
- `PORT`: Listening port (default: `8080`)
- `PASSPHRASE`: Shared passphrase

### Client Config (`client/inactive.env`)

```env
HOST_URL=http://<host_ip>:8080
CLIENT_HOST=0.0.0.0
CLIENT_PORT=3000
PASSPHRASE=some-shared-secret
```

- `HOST_URL`: URL of the remote Host daemon (e.g. `http://<host_ip>:8080`)
- `CLIENT_HOST`: Bind IP for the Client daemon (default: `0.0.0.0`)
- `CLIENT_PORT`: Listening port for the Client daemon (default: `3000`)
- `PASSPHRASE`: Key matching the Host's passphrase

---

## Installation

The installer registers the current executable's folder with the OS's standard service system and configures autostart.
**⚠ Place the folder in your desired permanent location before installing.**
**⚠ Also, before running the installer, open `inactive.env` inside the folder in a text editor, write your settings, and save the file.**
**If you missed this notice, redo the installation steps.**

To register and start the daemons (the script automatically detects whether client or host executable is present using wildcards):

Run the script matching your OS inside the release folder:
- Windows: Run `install.bat`
- macOS: Run `install.command`
- Linux: Run `./install.sh`

Additionally, you can find the OS-specific `config.<ext>` and `uninstall.<ext>` scripts in your release folder. During the installation phase, the installer also copies these scripts (e.g. `config.command`, `uninstall.command` on macOS) directly to the target installation directory (e.g. `/Users/<user>/Applications/OpenRemoteURLClient.app/` on macOS) for convenience.

_In your Client OS settings, select **Open Remote URL** as the default web browser._

---

## Verification

Running either binary without arguments displays its current autostart registration status and daemon status. When installed, it also prints the target executable and configuration file paths:

- **Host**:

```bash
$ ./open-remote-url-host
Open Remote URL - Host Status

[Status]
- Installed:  Yes
- Running:    Yes
- HOST:       http://0.0.0.0:8080
- Executable: /Users/qwo/Applications/OpenRemoteURLHost.app/Contents/MacOS/open-remote-url-host
- Config:     /Users/qwo/Applications/OpenRemoteURLHost.app/.env

[Usage]
- To install / start host:
  Run install.command in the release folder

- To uninstall / clean registrations:
  Run uninstall.command in the release folder
```

- **Client**:

```bash
$ ./open-remote-url-client
Open Remote URL - Client Status

[Status]
- Installed: Yes
- Running:   Yes
- HOST:      http://localhost:8080/
- CLIENT:    http://0.0.0.0:3000
- Executable: /Users/qwo/Applications/OpenRemoteURLClient.app/Contents/MacOS/open-remote-url-client
- Config:     /Users/qwo/Applications/OpenRemoteURLClient.app/.env

[Usage]
- To install / start client:
  Run install.command in the release folder

- To uninstall / clean registrations:
  Run uninstall.command in the release folder
```

---

## Uninstallation

To completely remove autostart registrations, browser associations, plist files, and systemd user service settings and terminate the background process:

Run the script matching your OS inside the release folder (or inside your installation path):
- Windows: Run `uninstall.bat`
- macOS: Run `uninstall.command`
- Linux: Run `./uninstall.sh`

---

## About Project Development

Most of this project (source code implementation, refactoring, and documentation) was developed and organized by the advanced AI model **Gemini 3.5 Flash**, which has high-level design, coding, and structuring capabilities.

Additionally, the system architecture diagram (flowchart) was created by feeding a handwritten sketch drawn on iPad Freeform into **Gemini 3.1 Flash (Multimodal Feature)**, which neatly transcribed the text and adjusted the box placement and overall layout (with considerable trial and error in prompt tuning).

These AI models are available for free on Antigravity and Antigravity IDE provided by Google.
This project was completed in approx. 8 hours using the Google One AI Pro plan (approx. $20 / month).

- **Development Started (First Prompt)**: 2026/05/26 15:35 JST
- **Development Completed (Last Prompt)**: 2026/05/26 23:40 JST

Note: Since the prompt input and code verification were done in parallel while watching anime, and Gemini's response was extremely fast, it could have been completed in an even shorter time if focused solely on development.
