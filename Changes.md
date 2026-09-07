# Changes

直近のコミット（`style: apply cargo fmt across the workspace` と、その 1 つ前
`fix: make the status window scrollable ...`）の内容。以降はコミットごとに
まっさらにして書き直す。

## 直したもの

- **状態ウィンドウが他の DPI スケールでボタン行を見切れさせていた。**
  ウィンドウを固定サイズかつリサイズ不可（`with_resizable(false)`）にしていたが、
  すべてのモニタの DPI スケールに一度に合う固定サイズは決められない（姉妹アプリの
  monsgeek-profile-switcher で実機確認: 1440p / 125% でちょうど収まっていた同じ
  ウィンドウが 1080p / 100% ではボタン行を切り落としていた）。
  対処: `shared/src/gui.rs` の `CentralPanel` の中身を
  `egui::ScrollArea::vertical()` で囲み、`with_resizable(false)` を外した。
  収まらない内容は黙って切られる代わりにスクロールで届く。

## 整形

- ワークスペース全体に `cargo fmt` を適用（13 ファイル、整形のみ・挙動の変更なし）。
  `receiver/`・`sender/`・`shared/`（`src` と `tests` 両方）。ドキュメントコメント
  直前の空行の除去、関数引数・FFI 宣言・`format!` 呼び出しの複数行化、構造体リテラルの
  展開など。
- `cargo fmt --check` が通ることを確認済み。

## 確認

- `cargo build` 成功。
- `cargo test` 11 件パス（`host_daemon_test` 7、`port_bind_test` 4）。
- いずれも Windows 上で実行。macOS 固有パス（Apple Event、`SecTranslocate`、
  `.app` バンドル生成）はビルドのみで未実行。
