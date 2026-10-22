# shiori4_abi

SHIORI 次世代 ABI の定義クレートです。

## 役割

SHIORI とホスト（ベースウェア）の間で、バイト列をどう運ぶかだけを定めた C ABI です。関数テーブル、`ShioriBytes`、版（`abi_version`）、所有権とスレッドの規約を持ち、C ヘッダが付属します。ペイロードの中身は解釈しません。

## 想定する利用者

ABI を直接使う実装者です。Rust 以外の言語で SHIORI やホストを作る人も含みます。Rust で SHIORI やホストを作るなら、上位の [shiori4](https://crates.io/crates/shiori4) や [shiori4_host](https://crates.io/crates/shiori4_host) を使うほうが簡単です。

## 使い始めの入口

- SHIORI が公開するシンボルは `shiori_get_api` の 1 つだけです。ホストはこれに版を渡して関数テーブルを受け取ります
- Rust で ABI を直接実装するなら、`Shiori` trait を実装し `export_shiori!` で `shiori_get_api` を生成します
- Rust 以外の言語からは C ヘッダ [include/shiori4_abi.h](https://github.com/ekicyou/shiori4/blob/main/shiori4_abi/include/shiori4_abi.h) を使います

## 他の 2 クレートとの関係

何にも依存しない最下層です。[shiori4](https://crates.io/crates/shiori4) がこのクレートを再エクスポートし、[shiori4_host](https://crates.io/crates/shiori4_host) は shiori4 を通してこのクレートを使います。

## 詳しいドキュメント

ABI の規約（版、所有権、戻り値、スレッド）と設計方針は [docs.rs](https://docs.rs/shiori4_abi) のクレートのドキュメントにあります。

## ライセンス

MIT ライセンスです。本文はクレートに同梱の [LICENSE](https://github.com/ekicyou/shiori4/blob/main/shiori4_abi/LICENSE) にあります。
