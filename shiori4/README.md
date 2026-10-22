# shiori4

shiori4_abi の上に載る、SHIORI 側の高水準層です。

## 役割

SHIORI を作るためのクレートです。v3（SHIORI/3.0 全文）と v4（ヘッダ部分だけ）の文字列形式の差を吸収し、ヘッダの解析と生成、プロパティの取得、post の受付 ID の待ち合わせ、ホストとの間のメッセージ名（`host_protocol`）を提供します。

## 想定する利用者

SHIORI 作者です。

## 使い始めの入口

`Ghost` trait を実装し、`export_ghost!` でエクスポートします。クレートの種類は `crate-type = ["cdylib"]` にします。

## 他の 2 クレートとの関係

- [shiori4_abi](https://crates.io/crates/shiori4_abi) に依存し、それを再エクスポートします
- ホスト側の [shiori4_host](https://crates.io/crates/shiori4_host) と、メッセージ名の語彙（`host_protocol`）を共有します

## 詳しいドキュメント

使用例、v3 と v4 の扱いの違い、設計方針は [docs.rs](https://docs.rs/shiori4) のクレートのドキュメントにあります。

## ライセンス

MIT ライセンスです。本文はクレートに同梱の [LICENSE](https://github.com/ekicyou/shiori4/blob/main/shiori4/LICENSE) にあります。
