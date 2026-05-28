# Open Remote URL (簡易解説版)

[English](README.md) | [简体中文](README.zh_CN.md) | [日本語 (原文)](README.ja_JP.md) | 日本語 (非技術者向け)

> 💡 **プロジェクトの開発について**
> このプロジェクトの大部分は、AIモデル **Gemini**（各種バージョン）によって開発されました。詳細は[プロジェクトの開発について](#プロジェクトの開発について)をご覧ください。

このツールは、**「手元のPCや仮想環境（クライアント）」で開いたWebサイトを、「別のPC（ホスト）」のブラウザで表示させ、必要なポートを自動でつないでくれるシステム**です。

難しい専門用語を使わずに、このツールが何をしてくれるのか、なぜ必要なのかを解説します。

## 目次

- [このツールが解決すること](#このツールが解決すること)
- [なぜこれが便利なの？（具体例）](#なぜこれが便利なの具体例)
- [どうやって動くの？（仕組み）](#どうやって動くの仕組み)
- [環境設定](#環境設定)
- [インストール（PCをつけた時に自動で起動するようにする）](#インストールpcをつけた時に自動で起動するようにする)
- [ステータス確認](#ステータス確認)
- [アンインストール（PCをつけた時の自動起動を止める）](#アンインストールpcをつけた時の自動起動を止める)
- [プロジェクトの開発について](#プロジェクトの開発について)

---

## このツールが解決すること

通常、仮想環境（Docker, VM）の内部や、別の配信用・ゲーム用PCなどの「サブPC」でプログラミングや操作をしているとき、以下のような問題が起こります。

1. ログインや認証をしようとすると、ブラウザが勝手に開くが、サブPC側のブラウザにはログイン情報が入っていないため、毎回ログインし直す必要がある。
2. 開発ツール（Dockerや仮想システム）でログイン用のURLを開くけれど、認証が終わった後の結果データがサブPC側に戻ってこないため、ログインが途中で止まってしまう。

**Open Remote URL**は、サブ環境でURLを開こうとする動作をキャッチし、**あなたのメインPCのブラウザで自動的に表示させつつ、一時的に通信の通り道（プロキシ）を自動で作成してつなぐ**ことで、これらの問題をすべて解消します。

---

## なぜこれが便利なの？（具体例）

### 1. Dockerコンテナでのスムーズな認証ログイン

開発環境で `wrangler login` を実行すると、メインPCの画面でブラウザが立ち上がります。ブラウザで認証完了ボタンを押すだけで、コンテナ内の環境が自動的に認証トークンを受け取り、そのままログインが完了します（コピペや複雑なポート設定は一切不要です）。

### 2. 2PC（ゲーム・配信用PC）環境でのブラウザ一元化

サブPCでDiscordのリンクやWebページを開いたとき、普段ログイン状態や拡張機能が揃っている「メインPCのブラウザ」で開くことができます。サブPCにログイン情報を残す必要がなく、セキュリティ的にも安心です。

---

## どうやって動くの？（仕組み）

![動作原理](docs/flowchart.webp)

1. **ブラウザ起動のインターセプト**: クライアントPC側でデフォルトのブラウザとして登録されます。URLを開こうとするすべての動作をこのツールが一時的に受け取ります。
2. **URLをメインPCへ転送**: 受け取ったURLをメインPC（ホスト）に送り、メインPCのブラウザで自動的に開きます。
3. **必要なポートの自動検出**: URLを開く15秒前までに開いていたポート、および開いてから3秒間のあいだに新しく開いたポートを自動で検知します。
4. **通り道の自動作成**: 検知したポートと同じポートをメインPC（ホスト）側にも一時的に作成し、クライアントPC側へと通信を逆転送する通り道（リバースプロキシ）を自動で作ります。
5. **終わったらお片付け**: クライアントPC側のポートが閉じられたと検知すると、メインPC側の通り道も自動で削除されます（削除されずに放置された場合も、5分後に自動消去されます）。

---

## 環境設定

設定は、PCシステムの設定（環境変数）や、設定ファイル（`.env`）から読み込まれます。

### 設定ファイル（.env）が読み込まれる場所

プログラムが実行されると、次の順番で設定ファイルを探して読み込みます。

1. **OSごとの決まったフォルダ**（インストールした後はここから読み込まれます）
   - **Windows**: `%APPDATA%\open-remote-url\<clientまたはhost>\.env`
   - **macOS**: `Applications` フォルダ内の `OpenRemoteURLClient.app`（または `OpenRemoteURLHost.app`）フォルダの直下の `.env`
   - **Linux**: `~/.config/open-remote-url/<clientまたはhost>/.env`
2. **プログラムを実行したフォルダ**
   - 上の決まったフォルダにファイルがない場合は、プログラムを実行したフォルダ（またはその上のフォルダ）にある `.env` ファイルを読み込みます（手動で動かすときなどに使います）。

※PCシステムの設定と `.env` ファイルの両方で同じ設定がある場合は、 **`.env` ファイルの内容が優先（上書き）されます**。

### インストールするときの動き

ダウンロードしたフォルダには、あらかじめ `inactive.env` という名前のテンプレートファイルが入っています。
インストール用のプログラムを実行すると、この `inactive.env` が自動的に上記の「OSごとの決まったフォルダ」へ `.env` という名前に変わってコピーされます。

- **インストールする前に**、フォルダの中にある `inactive.env` をメモ帳などで開き、自分の環境に合わせて設定を書き換えて保存しておいてください。
- もしフォルダの中に `inactive.env` がない状態でインストールを行った場合は、デフォルトの設定が入った `.env` ファイルが自动的に作成されます。

### ホスト設定（メインPC側: `host/inactive.env`）

```env
HOST=0.0.0.0
PORT=8080
PASSPHRASE=二人だけの合言葉
```

- `HOST`: 待ち受けアドレス（どこからでも受け付けるなら `0.0.0.0`）
- `PORT`: メインPCが接続を待つポート番号（デフォルト: `8080`）
- `PASSPHRASE`: クライアントPCと一致させる合言葉

### クライアント設定（サブPCや仮想環境側: `client/inactive.env`）

```env
HOST_URL=http://<ホストのIPアドレス>:8080
CLIENT_HOST=0.0.0.0
CLIENT_PORT=3000
PASSPHRASE=二人だけの合言葉
```

- `HOST_URL`: メインPCの住所（例: `http://<ホストのIPアドレス>:8080`）
- `CLIENT_HOST`: クライアントデーモンのバインドIP
- `CLIENT_PORT`: クライアントデーモンのリスニングポート（デフォルト: `3000`）
- `PASSPHRASE`: メインPC側と一致する合言葉

---

## インストール

インストーラーを実行すると、プログラムをOSのスタートアップ（自動起動システム）に登録し、**PCをつけた時（起動時）に自動でバックグラウンド実行されるように設定**します。

**※注意※**
- **インストールを行う前に、フォルダ内にある `inactive.env` をメモ帳などで開き、設定を書き換えて保存してください。**

自動起動の設定をするには、以下の手順を行います：
- **Windows**: 実行ファイル（`open-remote-url-client.exe` または `open-remote-url-host.exe`）をダブルクリックして、ボタン（Install Service）を押します。
- **macOS**: アプリケーションファイル（`OpenRemoteURLClient.app` または `OpenRemoteURLHost.app`）をダブルクリックして、ボタン（Install Service）を押します。
- **Linux**: `./install.sh` を実行します。

_インストールが終わったら、クライアントPCの設定で「デフォルトのブラウザ（一番最初に開くブラウザ）」として **Open Remote URL** を選んでください：_
- **Windows**: 設定 → アプリ → 既定のアプリ → Web ブラウザ
- **macOS**: システム設定 → デスクトップと Dock（または一般）→ デフォルトの Web ブラウザ → **Open Remote URL Client**
- **Linux**: `xdg-settings set default-web-browser open-remote-url-client.desktop`（またはデスクトップ環境の設定から変更）

---

## ステータス確認

プログラムのファイルをダブルクリックすると、**GUIコントロールパネル**が開きます。インストールボタンを押した後、自動的にステータスが更新されます。インストールが完了している場合、実行ファイルの配置先と設定ファイルのパスも表示されます。

- **ホスト側**:

```bash
$ ./open-remote-url-host
Open Remote URL - Host Status

[Status]
- Installed:  Yes (インストールされています)
- Running:    Yes (稼働中です)
- HOST:       http://0.0.0.0:8080
- Executable: /Users/<ユーザー名>/Applications/OpenRemoteURLHost.app/Contents/MacOS/open-remote-url-host
- Config:     /Users/<ユーザー名>/Applications/OpenRemoteURLHost.app/.env

[Usage]
- To install / start host:
  Double-click the OpenRemoteURLHost.app bundle to open GUI Control Panel

- To uninstall / clean registrations:
  Open GUI Control Panel and click Uninstall
```

- **クライアント側**:

```bash
$ ./open-remote-url-client
Open Remote URL - Client Status

[Status]
- Installed:  Yes (インストールされています)
- Running:    Yes (稼働中です)
- HOST:       http://localhost:8080/
- CLIENT:     http://0.0.0.0:3000
- Executable: /Users/<ユーザー名>/Applications/OpenRemoteURLClient.app/Contents/MacOS/open-remote-url-client
- Config:     /Users/<ユーザー名>/Applications/OpenRemoteURLClient.app/.env

[Usage]
- To install / start client:
  Double-click the OpenRemoteURLClient.app bundle to open GUI Control Panel

- To uninstall / clean registrations:
  Open GUI Control Panel and click Uninstall
```

---

## アンインストール（PCをつけた時の自動起動を止める）

使わなくなったら、PCをつけた時の自動起動の設定やデフォルトブラウザの登録をきれいに削除します。

アンインストールを実行するには、以下の手順を行います：
- **Windows**: 実行ファイルをダブルクリックして、ボタン（Uninstall Service）を押します。
- **macOS**: アプリケーションファイル（`.app`）をダブルクリックして、ボタン（Uninstall Service）を押します。
- **Linux**: `./uninstall.sh` を実行します。

---

## プロジェクトの開発について

このプロジェクトの大部分（ソースコードの実装、リファクタリング、ドキュメント作成）は、高度な設計・コーディング・構成能力を備えたAIモデル **Gemini 3.5 Flash** を使用して開発および整理されました。

また、システム構成図（フローチャート）の画像は、iPadのフリーボードで描いた手書きスケッチをベースに、**Gemini 3.1 Flash（マルチモーダル機能）** に読み込ませて文字を綺麗に清書し、枠の配置や全体のレイアウトを調整しながら作成されました（プロンプトの調整には試行錯誤が重ねられています）。

これらのAIモデルは、Googleが提供するAntigravityおよびAntigravity IDE上で無料で使用できます。
また、本プロジェクトはGoogle One AI Proプラン（月額約3,000円）を利用して完成しました。

- **開発開始（最初のプロンプト送信）**: 2026/05/26 15:35 JST
- **最終更新**: 2026/05/29 JST

なお、アニメの鑑賞と並行しながらのプロンプト入力・コード検証であったこと、そしてGeminiの応答が極めて高速であったことから、開発に専念していれば、本来はさらに短い時間で完成させることも可能でした。
