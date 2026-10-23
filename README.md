# shiori4 workspace

| クレート | 役割 | リンク |
|---|---|---|
| [`shiori4_abi`](shiori4_abi/README.md) | ABI 定義（C ABI の関数テーブル、ShioriBytes、版、規約）。C ヘッダ `include/shiori4_abi.h` 付き | [crates.io](https://crates.io/crates/shiori4_abi) |
| [`shiori4`](shiori4/README.md) | SHIORI 側の高水準層（`Ghost` trait、v3/v4 変換、Headers、Properties、PendingTags、host_protocol） | [crates.io](https://crates.io/crates/shiori4) |
| [`shiori4_host`](shiori4_host/README.md) | ホスト側の高水準層（`Host` trait、`GhostRuntime`、post キュー、完了通知、pump） | [crates.io](https://crates.io/crates/shiori4_host) |

要件: Rust 1.85 以上（edition 2024）

```sh
cargo test
```

## SHIORI を作る
`shiori4::Ghost` を実装し、`shiori4::export_ghost!(MyGhost);` でエクスポート（crate-type = ["cdylib"]）。

## ホストを作る
`shiori4_host::Host` を実装し、`GhostRuntime::load(lib, ghost_dir, host)` で読み込む（`ghost_dir` は絶対パス）。
`Host::wake` が呼ばれたらイベントループから `GhostRuntime::pump()` を呼ぶ。
