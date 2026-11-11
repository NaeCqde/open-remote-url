# open-remote-url 仕様

別の端末(Sender)で開こうとした URL を、手元の端末(Receiver)のブラウザで開く仕組み。OAuth のコールバック用のローカルのポートは、Receiver から Sender へ一時的にリバースプロキシで転送する。
Windows と macOS で動作を確認している。Linux は未確認。
仕様を変えたときは、このファイルも必ず更新する。README(`README*.md`)は利用者向けの説明、このファイルは動作の決まり。

## 構成(Cargo ワークスペース)

| クレート | 役割 |
|---|---|
| `sender/` | Sender の常駐プログラム(`open-remote-url-sender`)。OS の URL ハンドラーとして URL を受け取り、Receiver へ送る。待ち受けポートを監視して Receiver へ知らせる。`/proxy` で Receiver からの中継を受ける |
| `receiver/` | Receiver の常駐プログラム。`/open` で URL を受けてブラウザで開く。`/ports` でポートの転送(一時プロキシ)を管理する |
| `shared/` | 設定の読み込み、インストーラー/アンインストーラー、GUI(eframe)、URL スキームのハンドラー登録、共通の型と補助関数(`secure_eq`、`bearer_matches`、`is_openable_url`) |

## 設定

`.env` の探し方: OS 別の設定フォルダー(Windows `%APPDATA%\open-remote-url\<sender|receiver>\.env`、macOS はアプリ内、Linux `~/.config/open-remote-url/<sender|receiver>/.env`)→ 実行したフォルダーとその親。`.env` の値が環境変数より優先される。
インストール時に、配布物の `.env`(テンプレート)を設定フォルダーの `.env` にコピーする。設定フォルダーにすでに `.env` があれば上書きしない。配布物に `.env` がなければ既定値の `.env` を作る。アンインストールしても設定フォルダーと `.env` は消さない。サービスをインストールしていなくても、設定フォルダーに `.env` があれば設定の表示・読み込みにそれを使う(なければ配布物のテンプレート)。

Receiver: `LISTEN`(既定 `0.0.0.0:40000`)、`PASSPHRASE`(空なら認証なし)、`ALLOWED_SCHEMES`(`http`/`https` のほかに開いてよい URL スキームのカンマ区切り。既定は空)。
Sender: `LISTEN`(既定 `0.0.0.0:30000`)、`HOST_URL`(Receiver の URL。`http://` か `https://`)、`SELF_URL`(Receiver から Sender へ中継するときの URL)、`PASSPHRASE`、`SCHEME`(OS のハンドラーに登録する追加のスキーム)、`HTTP`(`false` で http/https のハンドラー登録をしない。既定 `true`)。

## 通信(HTTP / JSON)

認証: `PASSPHRASE` が設定されているとき、すべての POST で `Authorization: Bearer <PASSPHRASE>` が必要(定数時間で比較する)。合わなければ 401。`PASSPHRASE` が空のときは認証しない(Receiver は起動時に警告を出す)。

### Receiver

| メソッド・パス | 内容 |
|---|---|
| `GET /` | `alive` |
| `POST /open` | `{url}`。`http://`・`https://` か `ALLOWED_SCHEMES` のスキームの URL だけを、OS に渡して開く(別スレッドで)。それ以外は 400 `URL scheme is not allowed`。成功は 200 `OK` |
| `POST /ports` | `{ports:[u16], action:"add"|"delete", self_url}`。`add`: 各ポートについて、Receiver の `127.0.0.1:<ポート>` に一時プロキシを立てる(すでにあれば再利用)。`delete`: そのプロキシを止める |

一時プロキシ: 受けたリクエスト(メソッド、パスとクエリ、ヘッダー、本文を Base64)を `POST <self_url>/proxy` に `ProxyRequest` として送り、返ってきた `ProxyResponse`(状態、ヘッダー、本文を Base64)をそのまま応答にする(`content-length` と `transfer-encoding` は除く)。パスフレーズがあれば `Authorization` を付ける。中継の取得は接続 10 秒、全体 30 秒で打ち切る。失敗は 502。
プロキシは**5 分**たつと自動で止まる。ポートがすでに使われていて立てられなければ、登録を消す。

### Sender

| メソッド・パス | 内容 |
|---|---|
| `POST /open` | `{url}`。受けた URL を内部のキューへ入れる(OS のハンドラーから直接来る URL と同じ扱い) |
| `POST /proxy` | `ProxyRequest`。`path_and_query` が `/` で始まっていなければ 400。`http://localhost:<port><path_and_query>` に同じメソッド・ヘッダー・本文で要求し、結果を `ProxyResponse` で返す。本文の Base64 が壊れていれば 400 |

## Sender の動作(URL を受けてから)

1. OS が URL を Sender に渡す(登録した `http`/`https` と `SCHEME` のスキーム)。
2. その URL を Receiver の `POST /open` へ送る。
3. `http`/`https` の URL なら(カスタムスキームは転送なし)、OAuth のコールバック用のポートを探して Receiver に転送を頼む:
   - 1 秒ごとに待ち受けポートの一覧を取り、15 秒分の履歴を持つ。URL を開く前の 15 秒間に開いたポートを、`POST /ports`(`add`)で Receiver に送る。
   - URL を開いたあとの 3 秒間、1 秒ごとに新しく開いたポートを調べ、あれば同じく送る。
4. 送ったポートが Sender 側で閉じられたら、`POST /ports`(`delete`)を送る。

## 常駐とインストール

コマンド: `--config`(設定の表示)、`--install`、`--uninstall`、`--start`、`--stop`。引数なしで起動すると GUI(Windows、または macOS/Linux でダブルクリック起動のとき)。
インストール先: Windows `%LOCALAPPDATA%\Programs\open-remote-url\<sender|receiver>\`、macOS `~/Applications/OpenRemoteURL<Sender|Receiver>.app/`、Linux `~/.local/bin/open-remote-url/<sender|receiver>/`。OS の起動時に常駐プログラムが自動で始まるよう登録する。
ポートがすでに使われていれば「ほかのインスタンスが動いている」というメッセージを出して終了する。

## セキュリティ上の注意

- 通信は既定で平文の HTTP。信頼できる LAN か VPN(Tailscale など)の中だけで使うこと。`https://` の `HOST_URL` / `SELF_URL` も使える(rustls)。
- `PASSPHRASE` を必ず設定すること。空だと、届く相手は誰でも Receiver のブラウザに URL を開かせ、Sender の `localhost` のポートへ要求を中継させられる。
- Receiver が OS に渡す URL は、`http`/`https` と `ALLOWED_SCHEMES` のものだけ。ファイルのパスや実行ファイルは開かない。カスタムスキームを使うときは、Sender の `SCHEME` と Receiver の `ALLOWED_SCHEMES` の両方に書く。
- 中継されるリクエストには、OAuth のコードなどが含まれる。パスフレーズは Receiver から Sender への中継にも使われるので、2 台で同じ値にする。
