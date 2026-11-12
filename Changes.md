# Changes

直近のコミット（`fix: the receiver's temporary proxy no longer shadows a port that is in use`）の内容。
以降はコミットごとにまっさらにして書き直す。

## 直したもの

- **Receiver の一時プロキシが、使用中のポートを横取りしていた。**
  `tokio::net::TcpListener::bind` は Unix で `SO_REUSEADDR` を付けて待ち受ける。macOS では
  それにより、別のプログラムが `0.0.0.0:<ポート>` で待ち受けている最中でも
  `127.0.0.1:<ポート>` を立てられ、`localhost` 宛ての接続がすべてプロキシへ届いた。
  実機で、Sender 側の PC の Sunshine（47989 / 47990）の転送が、Mac 自身の Sunshine に
  届くはずの `localhost` を 5 分間覆っていた。
  対処: `receiver/src/daemon.rs` の `start_proxy` が使う待ち受けを、`SO_REUSEADDR` なしの
  `TcpSocket` で作る（`bind_proxy_listener`）。使用中のポートは `AddrInUse` で失敗し、
  既存の「立てられなければ登録を消す」の経路に入る。閉じた直後のポートが `TIME_WAIT` で
  断られることがあるので、`AddrInUse` は 1 秒おきに最大 10 回やり直す。
  テスト: `0.0.0.0` で待ち受け中のポートは覆わないこと、空きポートには立てられること。
- `docs/SPEC.md` に上記の待ち受け方を追記した。
