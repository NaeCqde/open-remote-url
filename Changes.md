# 変更履歴

2026-11-17

## 追加

- Sender: http/https の URL を、ホストで振り分ける。`DOMAINS`(ホストの完全一致、`*.example.com`、IP、CIDR)と `DOMAIN_MODE` を使う
  (`exclude`: 列挙したホストはローカルで開き、それ以外を送る。`include`: 列挙したホストだけを送る)。ほかのスキームは、いつも送る。
- Sender: `BROWSER` と、本来の既定ブラウザーの自動記憶(Sender が http/https の OS 既定を置き換える直前に取る)。
  ローカルの URL は、そのアプリで直接起動する(macOS は `open -b`、Linux は `gtk-launch`、Windows は ProgID のコマンド)ので、
  Sender に戻ってくる循環は起きない。ブラウザーが分からないとき、または起動に失敗したときは、URL を送る。
- `shared/src/domain_filter.rs`、`shared/src/local_browser.rs`(単体テスト付き)、状態の出力、`sender/.env`、
  `docs/SPEC.md`、`README.md`、`README.en.md`。

- `BROWSER` は、Sender を起動するたびに書き込む。OS の既定が別のブラウザーのときは、それをインストール先の `.env` に
  書き(上書き)、Sender が引き継ぐ。Sender がすでに既定のときは、`BROWSER` を読む(Windows: 何も設定されていなければ、
  StartMenuInternet に最初に登録されているブラウザー)。`config::set_env_value` は、キー 1 つだけを書き換え、ほかは残す。
- アンインストール(Sender)は、Sender がまだ既定なら、`BROWSER`(なければ記憶したもの)から既定のブラウザーを戻す:
  macOS はバンドル ID、Linux は `.desktop` の ID(あわせて Sender の `.desktop` を消す)。Windows はプログラムから設定できないので、
  Sender の登録を消し、ブラウザーを選ぶようにユーザーに伝える。`BROWSER` は `.env` に残る。

## 修正

- ローカルのブラウザーを起動するとき、ランチャーの終了を待たなくした。Windows では、動いていなかったブラウザーを起動すると、
  ブラウザーを閉じるまでランチャーが残る(Brave で 318 秒を計測)。そのため、呼び出し元が止まっていた。1.5 秒以内に
  失敗したランチャーは、これまでどおりエラーとして報告する(その URL は Receiver に送る)。

## 検証

- macOS: Orion で、検出と起動を確認した。Windows 11: `cargo test -p shared --lib` と、ProgId による Brave の実際の起動
  (`BROWSER=BraveHTML.<id>`。Windows では Sender がすでに既定なので、`BROWSER` を設定する必要がある)。
- 未検証: Linux。起動時の `BROWSER` の書き込み、Windows の登録済みブラウザーへの代替、アンインストールでの復元は、
  単体テスト(set_env_value)だけで、実機では動かしていない。
