# shiori4 workspace

| クレート | 役割 |
|---|---|
| `shiori4_abi` | ABI 定義（C ABI の関数テーブル、ShioriBytes、版、規約）。C ヘッダ `include/shiori4_abi.h` 付き |
| `shiori4` | SHIORI 側の高水準層（`Ghost` trait、v3/v4 変換、Headers、Properties、PendingTags、host_protocol） |
| `shiori4_host` | ホスト側の高水準層（`Host` trait、`GhostRuntime`、post キュー、完了通知、pump） |

要件: Rust 1.85 以上（edition 2024）

```sh
cargo test
```

## SHIORI を作る
`shiori4::Ghost` を実装し、`shiori4::export_ghost!(MyGhost);` でエクスポート（crate-type = ["cdylib"]）。

## ホストを作る
`shiori4_host::Host` を実装し、`GhostRuntime::load(lib, ghost_dir, host)` で読み込む。
`Host::wake` が呼ばれたらイベントループから `GhostRuntime::pump()` を呼ぶ。
