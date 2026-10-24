# shiori4 workspace

| クレート | 役割 | リンク |
|---|---|---|
| [`shiori4_abi`](shiori4_abi/README.md) | ABI 定義（C ABI の関数テーブル、ShioriBytes、版、規約）。C ヘッダ `include/shiori4_abi.h` 付き | [crates.io](https://crates.io/crates/shiori4_abi) |
| [`shiori4`](shiori4/README.md) | SHIORI 側の高水準層（`Ghost` trait、v3/v4 変換）と、語に合わせた便利関数のモジュール `shiori4::protocol`（ヘッダの型、プロパティの取得、Tag の待ち合わせ、語の定数） | [crates.io](https://crates.io/crates/shiori4) |
| [`shiori4_host`](shiori4_host/README.md) | ホスト側の高水準層（`Host` trait、`GhostRuntime`、post キュー、完了通知、pump） | [crates.io](https://crates.io/crates/shiori4_host) |

SHIORI とホストが交わすメッセージの語（getproperty・post の Command・OnHostComplete など）は、[ホスト側プロトコル仕様](doc/host-protocol.md)が定めます。芯（`Ghost`・`Host` と runtime）は中身を解釈せず原文を渡し、`shiori4::protocol`（`shiori4_host::protocol` からも使える）は使いたい人が使う便利関数です。

要件: Rust 1.85 以上（edition 2024）

```sh
cargo test
```

## SHIORI を作る
`shiori4::Ghost` を実装し（`request`・`notify` はヘッダ部分の原文 `&str` を受ける。解析は `shiori4::protocol::Headers::parse`）、`shiori4::export_ghost!(MyGhost);` でエクスポート（crate-type = ["cdylib"]）。

## ホストを作る
`shiori4_host::Host` を実装し（`name` は必須。同期の要求 `request` は原文を受け、既定はどの要求にも 400。getproperty には `protocol::getproperty_response` で答える）、`GhostRuntime::load(lib, ghost_dir, host)` で読み込む（`ghost_dir` は絶対パス）。
`Host::wake` が呼ばれたらイベントループから `GhostRuntime::pump()` を呼ぶ。
