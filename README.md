# Open Remote URL

English | [日本語](README.ja_JP.md) | [日本語 (非技術者向け)](README.ja_EZ.md) | [简体中文](README.zh_CN.md)

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
    - [Host Config (host/.env)](#host-config-hostenv)
    - [Client Config (client/.env)](#client-config-clientenv)
- [Installation](#installation)
- [Verification](#verification)
- [Uninstallation](#uninstallation)
- [About Project Development](#about-project-development)

---

## How It Works

![Flowchart](docs/flowchart.png)

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

Configuration is loaded from system environment variables and from the `.env` file located in the same directory as the executable (if both are defined, the `.env` file takes precedence).

The distribution package (`.zip`) includes a `.env.example` template. Rename it to `.env` and fill in your settings before use.

### Host Config (`host/.env`)

```env
HOST=0.0.0.0
PORT=8080
PASSPHRASE=some-shared-secret
```

- `HOST`: Bind IP address (default: `0.0.0.0`)
- `PORT`: Listening port (default: `8080`)
- `PASSPHRASE`: Shared passphrase

### Client Config (`client/.env`)

```env
HOST_URL=http://<host_ip>:8080
RELAY_HOST=0.0.0.0
RELAY_PORT=3000
PASSPHRASE=some-shared-secret
```

- `HOST_URL`: URL of the remote Host daemon (e.g. `https://<host>:<port>`)
- `RELAY_HOST`: Bind IP for the Client relay server (default: `0.0.0.0`)
- `RELAY_PORT`: Client relay listening port (default: `3000`)
- `PASSPHRASE`: Key matching the Host's passphrase

---

## Installation

The installer registers the current executable's folder with the OS's standard service system and configures autostart.
**⚠ Place the folder in your desired permanent location before installing.**
**If you missed this notice, redo the installation steps.**

To register and start the daemons:

- **Host**: Run the script matching your OS inside the release folder:
    - Windows: Run `install-host.bat`
    - macOS: Run `install-host.command`
    - Linux: Run `./install-host.sh`
- **Client**: Run the script matching your OS inside the release folder:
    - Windows: Run `install.bat`
    - macOS: Run `install.command`
    - Linux: Run `./install.sh`

_In your Client OS settings, select **Open Remote URL** as the default web browser._

---

## Verification

Running either binary without arguments displays its current autostart registration status and daemon status:

```bash
$ ./open-remote-url
Open Remote URL - Client Status

[Status]
- Installed: Yes
- Running: Yes

[Usage]
- To install / start client:
  Run install.command in the release folder

- To uninstall / clean registrations:
  Run uninstall.command in the release folder
```

---

## Uninstallation

To completely remove autostart registrations, browser associations, plist files, and systemd user service settings and terminate the background process:

- **Host**: Run the script matching your OS inside the release folder:
    - Windows: Run `uninstall-host.bat`
    - macOS: Run `uninstall-host.command`
    - Linux: Run `./uninstall-host.sh`
- **Client**: Run the script matching your OS inside the release folder:
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
