# Open Remote URL

[English](README.md) | [日本語](README.ja_JP.md) | [日本語 (非技術者向け)](README.ja_EZ.md) | 简体中文

> 💡 **关于项目开发**
> 本项目的大部分内容均由 AI 模型 **Gemini 3.5 Flash** 开发。详情请参阅[关于项目开发](#关于项目开发)。

将 Web 网页的显示和 OAuth 登录认证转发给另一台设备（主机）的浏览器，进行统一管理的系统。

本系统会拦截本地**客户端 (Client)** 机器（虚拟机、Docker 容器或辅助 PC）上的浏览器打开请求，将 URL 转发给**主机 (Host)** 机器，并在主机的浏览器中打开网站。

### 解决的痛点与需求

- **解决在 VM / Docker 容器开发环境中进行 OAuth 登录的烦恼**
    - 在无浏览器（或干净隔离）的环境（如 Docker 容器）中运行 `wrangler login`、`supabase login` 等 CLI 命令时，工具会尝试启动浏览器。通常需要手动复制 URL 到主机浏览器打开，但登录完成后，浏览器对 `http://localhost:8976/oauth/callback` 等本地重定向端口的 OAuth 回调无法被接收，导致登录中途失败。
    - 本工具不仅能检测到客户端打开 URL 并在主机上显示，还会**自动扫描 URL 打开时已分配的端口（15秒前的历史记录）以及打开后新开启的端口（最多检测3秒），并自动从主机向客户端建立临时反向代理**。这样，只需在主机的浏览器中点击认证按钮，容器内的开发环境便能自动接收到认证令牌，登录过程无缝完成。

- **统一双电脑（直播/游戏）环境下的浏览器会话和凭证信息**
    - 在游戏/辅助电脑（客户端）中点击 Discord 或社交软件的链接时，它们会自动在主用电脑（主机）的浏览器（登录状态、密码管理器、浏览器扩展一应俱全的环境）中打开。
    - 这避免了在辅助电脑上重复登录的麻烦，同时也无需将登录凭证保存在辅助电脑上，安全性更高。

### ※ 开发理念

本工具源自开发者的强烈个人需求："在游戏电脑上登录账号时，希望登录页面能在主电脑上打开。" 任何破坏或偏离此核心理念和行为的合并请求（Pull Request）均不会被采纳。

## 目录

- [工作原理](#工作原理)
- [配置说明](#配置说明)
    - [主机配置 (host/.env)](#主机配置-hostenv)
    - [客户端配置 (client/.env)](#客户端配置-clientenv)
- [安装说明](#安装说明)
- [状态确认](#状态确认)
- [卸载服务](#卸载服务)
- [关于项目开发](#关于项目开发)

---

## 工作原理

![流程图](docs/flowchart.png)

1. **URL 拦截**：拦截客户端打开 URL 时发出的浏览器启动请求。
2. **URL 显示**：客户端守护进程立即仅将 URL 信息转发给主机，并在主机的浏览器中打开。
3. **端口检测与代理添加**：将客户端在 URL 打开前 15 秒内已分配的端口列表发送给主机，主机自动为这些端口启动临时代理服务器。
4. **动态监测新增端口**：URL 打开后，监测客户端 3 秒内新增的端口，若有新增则在主机侧追加代理。
5. **代理自动清理**：当客户端正在代理的端口关闭时，立即向主机发送删除请求，安全关闭不再需要的代理服务器（未被删除的代理也会在 5 分钟超时后自动清除）。

---

## 下载

[点击此处下载最新版本](https://github.com/NaeCqde/open-remote-url/releases/latest)

请下载与您的操作系统和架构匹配的 `.zip` 文件。

## 配置说明

配置从系统环境变量以及与可执行文件同目录下的 `.env` 文件中读取（若两者均有定义，则 `.env` 文件优先）。

发布包（`.zip`）中附带了 `.env.example` 模板文件，请将其重命名为 `.env` 并根据您的环境修改配置值。

### 主机配置 (`host/.env`)

```env
HOST=0.0.0.0
PORT=8080
PASSPHRASE=some-shared-secret
```

- `HOST`：绑定 IP 地址（默认：`0.0.0.0`）
- `PORT`：监听端口（默认：`8080`）
- `PASSPHRASE`：共享密码短语

### 客户端配置 (`client/.env`)

```env
HOST_URL=http://<host_ip>:8080
RELAY_HOST=0.0.0.0
RELAY_PORT=3000
PASSPHRASE=some-shared-secret
```

- `HOST_URL`：远程主机守护进程的 URL（例：`https://<host>:<port>`）
- `RELAY_HOST`：客户端中继服务器的绑定 IP（默认：`0.0.0.0`）
- `RELAY_PORT`：客户端中继监听端口（默认：`3000`）
- `PASSPHRASE`：与主机端匹配的共享密钥

---

## 安装说明

安装程序将当前可执行文件所在的文件夹注册到操作系统的标准服务系统，并配置自动启动。
**⚠ 安装前，请先将文件夹放置到您希望长期存放的位置。**
**如果忽略了此提示，请重新执行安装步骤。**

注册并启动守护进程：

- **主机侧**：在 release 目录下运行与您的操作系统匹配的脚本：
    - Windows：运行 `install-host.bat`
    - macOS：运行 `install-host.command`
    - Linux：运行 `./install-host.sh`
- **客户端**：在 release 目录下运行与您的操作系统匹配的脚本：
    - Windows：运行 `install.bat`
    - macOS：运行 `install.command`
    - Linux：运行 `./install.sh`

_请在客户端操作系统的设置中，将 **Open Remote URL** 选择为默认 Web 浏览器。_

---

## 状态确认

不带任何参数运行任一可执行文件，即可输出当前自动启动注册状态和守护进程运行状态：

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

## 卸载服务

要完全删除自动启动注册、浏览器关联、plist 文件和 systemd 用户服务等设置并终止后台进程：

- **主机侧**：在 release 目录下运行与您的操作系统匹配的脚本：
    - Windows：运行 `uninstall-host.bat`
    - macOS：运行 `uninstall-host.command`
    - Linux：运行 `./uninstall-host.sh`
- **客户端**：在 release 目录下运行与您的操作系统匹配的脚本：
    - Windows：运行 `uninstall.bat`
    - macOS：运行 `uninstall.command`
    - Linux：运行 `./uninstall.sh`

---

## 关于项目开发

本项目的大部分内容（包括源码实现、重构和文档编写）由具备高超设计、编码和组织能力的 AI 模型 **Gemini 3.5 Flash** 开发和整理。

此外，系统架构图（流程图）是以 iPad 无边记（Freeform）上绘制的手写草图为基础，通过输入给 **Gemini 3.1 Flash（多模态功能）** 进行文字清书、调整框架布局和整体版面优化而制作完成的（期间经过了反复的提示词微调与尝试）。

这些 AI 模型可以在 Google 提供的 Antigravity 和 Antigravity IDE 中免费使用。
本项目使用了 Google One AI Pro 计划（每月约 20 美元），用时约 8 小时便已完成。

- **开发开始（首个提示词）**：2026/05/26 15:35 JST
- **开发完成（最后一个提示词）**：2026/05/26 23:40 JST

此外，由于当时是在一边看动漫一边进行提示词输入和代码验证，且 Gemini 的响应速度极快，因此如果专注于开发，实际完成时间本来可以更短。
