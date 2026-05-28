# Open Remote URL

[English](README.md) | [简体中文](README.zh_CN.md) | 日本語 (原文) | [日本語 (非技術者向け)](README.ja_EZ.md)

> 💡 **プロジェクトの開発について**
> このプロジェクトの大部分は、AIモデル **Gemini 3.5 Flash** によって開発されました。詳細は[プロジェクトの開発について](#プロジェクトの開発について)をご覧ください。

Webサイトの表示やOAuthログイン認証を、別のデバイス（ホスト）のブラウザに転送して一元管理するシステム。

ローカルネットワーク内または仮想マシン上の**クライアント**マシンでのブラウザ起動をインターセプトし、URLを**ホスト**マシンへ転送して、ホスト側のブラウザを起動してWebサイトを表示します。

### 解決する課題・ニーズ

- **仮想マシン (VM) / Dockerコンテナ開発環境でのOAuthログインの苦痛を解消**
    - Dockerコンテナなどのブラウザ非搭載（またはクリーンな）環境で `wrangler login`、`supabase login` などのCLIコマンドを実行すると、ログイン用のブラウザを起動しようとします。通常はURLをコピーしてホストのブラウザで開く必要がありますが、ログイン完了後にブラウザから `http://localhost:8976/oauth/callback` などのローカルリダイレクトポートに対して行われるコールバックを受け取れず、ログインが途中で失敗します。
    - 本ツールは、クライアント環境でURLが開かれたことを検知してホストで表示するだけでなく、**URLが開かれたタイミングで割り当てられていたポート（15秒前の履歴）およびオープン後に新しく開いたポート（3秒後までチェック）を自動で検知し、ホストからクライアントへ一時的なリバースプロキシを自動で確立**します。これにより、ホストのブラウザで認証ボタンを押すだけで、コンテナ内の開発環境が自動でトークンを受信し、何事もなかったかのようにログインが成功します。

- **2PC（配信・ゲーム）環境でのブラウザセッション・認証情報の一元化**
    - ゲームPC（クライアント）でDiscordやSNSのリンクをクリックした際、普段使いのメインPC（ホスト）のブラウザ（ログイン情報、パスワードマネージャー、拡張機能が揃っている環境）で開くことができます。
    - サブPC側でログイン作業を繰り返す手間を削減し、セキュリティ面でもログイン情報をサブPC側に残さないため安全です。

### ※開発コンセプトについて

このプログラムは、開発者の「ゲーム用PCでアカウントにログインするとき、ログイン画面をメインPCで開きたい」という強いニーズから生まれました。このコンセプトや動作を破壊するようなプルリクエストは承認されません。

## 目次

- [動作原理](#動作原理)
- [環境設定](#環境設定)
    - [ホスト設定 (host/inactive.env)](#ホスト設定-hostinactiveenv)
    - [クライアント設定 (client/inactive.env)](#クライアント設定-clientinactiveenv)
- [インストール](#インストール)
- [ステータス確認](#ステータス確認)
- [アンインストール](#アンインストール)
- [プロジェクトの開発について](#プロジェクトの開発について)

---

## フローチャート

![フローチャート](docs/flowchart.webp)

1. **URLのインターセプト**: クライアント上でURLが開かれた際のブラウザ起動要求をインターセプトします。
2. **URLの表示**: クライアントデーモンは、即座にURL情報のみをホストへ転送し、ホスト側のブラウザで開きます。
3. **ポート検出とプロキシ追加**: URLを開く15秒前までにクライアント側で割り当てられていたポート一覧を、ホストに送信します。ホスト側は自動的にそれらのポートに対する一時的なプロキシサーバーを立ち上げます。
4. **追加ポートの動的監視**: URLを開いた後、3秒間クライアント側を監視し、新しく追加されたポートがあればホスト側に追加でプロキシを立ち上げます。
5. **プロキシの自動削除**: クライアント側でプロキシ中のポートが閉じられた場合、ホストに対し即座に削除要求を送信し、不要になったプロキシサーバーを安全に破棄します（削除されなかったプロキシも、5分のタイムアウトで自動的に消去されます）。

---

## ダウンロード

[こちらから最新のリリースをダウンロード](https://github.com/NaeCqde/open-remote-url/releases/latest)

OSとアーキテクチャにあった.zipファイルをダウンロードしてください。

## 環境設定

設定は、システム環境変数および `.env` ファイルから読み込まれます。

### 設定ファイルの優先順位と読み込み場所

プログラム実行時、設定ファイル（`.env`）は以下の優先順位で探索・読み込まれます。

1. **OSごとの設定フォルダ**（インストール済みの場合はこちらから読み込まれます）
   - **Windows**: `%APPDATA%\open-remote-url\<client|host>\.env`
   - **macOS**: `/Users/<user>/Applications/OpenRemoteURLClient.app/.env`（または `OpenRemoteURLHost.app/.env`）
   - **Linux**: `~/.config/open-remote-url/<client|host>/.env`
2. **実行時のカレントディレクトリまたはその親ディレクトリ**
   - 上記の設定フォルダに `.env` が存在しない場合、プログラムを実行したディレクトリ（または親ディレクトリ）にある `.env` ファイルが読み込まれます（開発時や手動実行時に便利です）。

※システム環境変数と `.env` ファイルの両方で同じ項目が定義されている場合は、 **`.env` ファイルの内容が優先（上書き）されます**。

### インストール時の動作

配布パッケージにはテンプレートとして `inactive.env` が同梱されています。
インストールスクリプトを実行すると、インストール用フォルダ内にある `inactive.env` が上記「OSごとの設定フォルダ」へ `.env` として自動的に移動（コピーされた後、元の `inactive.env` は削除）されます。

- **インストールの実行前に**、パッケージフォルダ内にある `inactive.env` をテキストエディタで開き、環境に合わせて設定値をあらかじめ書き換えて保存しておいてください。
- もし `inactive.env` が存在しない状態でインストールを実行した場合、デフォルトの設定値が記述された `.env` ファイルが設定フォルダ内に自動生成されます。

### ホスト設定 (`host/inactive.env`)

```env
HOST=0.0.0.0
PORT=8080
PASSPHRASE=some-shared-secret
```

- `HOST`: バインドIPアドレス（デフォルト: `0.0.0.0`）
- `PORT`: リスニングポート（デフォルト: `8080`）
- `PASSPHRASE`: 共通パスフレーズ

### クライアント設定 (`client/inactive.env`)

```env
HOST_URL=http://<host_ip>:8080
CLIENT_HOST=0.0.0.0
CLIENT_PORT=3000
PASSPHRASE=some-shared-secret
```

- `HOST_URL`: リモートホストデーモンのURL (例: `http://<host_ip>:8080`)
- `CLIENT_HOST`: クライアントデーモンのバインドIP（デフォルト: `0.0.0.0`）
- `CLIENT_PORT`: クライアントデーモンのリスニングポート（デフォルト: `3000`）
- `PASSPHRASE`: ホストのパスフレーズと一致するキー

---

## インストール

インストーラーを実行すると、現在の実行ファイルが配置されているフォルダをOS標準のサービスに登録し、自動起動（バックグラウンドデーモン）の設定を行います。

**※重要※**

- **インストーラーを実行する前に、フォルダを任意の恒久的な配置場所（例: macOSでは `~/Applications/`）へ移動してください。**
- **また、インストールを行う前にフォルダ内の `inactive.env` をテキストエディタで開き、環境に応じた設定（接続先ホストURLやポート、パスフレーズ等）を書き換えて保存してください。**

デーモンを登録・起動するには、フォルダ内のOSに対応するスクリプトを実行します（インストールスクリプトは、フォルダ内のバイナリからクライアント/ホストを自動検出してインストールを行います）：

- Windows: `install.bat` を実行
- macOS: `install.command` を実行
- Linux: `./install.sh` を実行

また、フォルダにはOSに応じた `config`（設定管理）および `uninstall`（アンインストール）用のスクリプトも同梱されています。インストール時、これらの管理用スクリプト（例: macOSでは `config.command` および `uninstall.command`）は、利便性のためにインストール先フォルダ（例: macOSでは `/Users/<user>/Applications/OpenRemoteURLClient.app/` の直下）へ自動的にコピーまたは生成されます。

_インストール完了後、クライアント側OSの設定において、「デフォルトのWebブラウザ」として **Open Remote URL** を選択してください。_

---

## ステータス確認

いずれの実行ファイルも引数なしで実行すると、現在の自動起動の登録状況とデーモン起動状況が出力されます。

インストールが完了している場合、実行ファイルの配置先と設定ファイルのパスも追加で表示されます。

- **ホスト側**:

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

- **クライアント側**:

```bash
$ ./open-remote-url-client
Open Remote URL - Client Status

[Status]
- Installed:  Yes
- Running:    Yes
- HOST:       http://localhost:8080/
- CLIENT:     http://0.0.0.0:3000
- Executable: /Users/qwo/Applications/OpenRemoteURLClient.app/Contents/MacOS/open-remote-url-client
- Config:     /Users/qwo/Applications/OpenRemoteURLClient.app/.env

[Usage]
- To install / start client:
  Run install.command in the release folder

- To uninstall / clean registrations:
  Run uninstall.command in the release folder
```

---

## アンインストール

自動起動登録、ブラウザ関連付け、plist、systemdユーザーサービスなどの設定を完全に削除してバックグラウンドプロセスを終了するには、フォルダ内（またはインストール先フォルダ内）のOSに対応するスクリプトを実行します。

- Windows: `uninstall.bat` を実行
- macOS: `uninstall.command` を実行
- Linux: `./uninstall.sh` を実行

---

## プロジェクトの開発について

このプロジェクトの大部分（ソースコードの実装、リファクタリング、ドキュメント作成）は、高度な設計・コーディング・構成能力を備えたAIモデル **Gemini 3.5 Flash** によって開発および整理されました。

また、システム構成図（フローチャート）の画像は、iPadのフリーボードで描画した手書きのスケッチをベースに、**Gemini 3.1 Flash（マルチモーダル機能）** に読み込ませて文字を綺麗に清書し、枠の配置や全体のレイアウトを調整しながら作成されました（プロンプトの調整には試行錯誤が重ねられています）。

これらのAIモデルは、Googleが提供するAntigravityおよびAntigravity IDE上で無料で使用できます。
また、本プロジェクトはGoogle One AI Proプラン（月額約3,000円）を利用し、約8時間で完成しました。

- **開発開始（最初のプロンプト送信）**: 2026/05/26 15:35 JST
- **開発完了（最後のプロンプト送信）**: 2026/05/26 23:40 JST

なお、アニメの鑑賞と並行しながらのプロンプト入力・コード検証であったこと、そしてGeminiの応答が極めて高速であったことから、開発に専念していれば、本来はさらに短い時間で完成させることも可能でした。
