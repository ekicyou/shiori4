# shiori4

shiori4_abi の上に載る、SHIORI 側の高水準層です。

## 役割

SHIORI を作るためのクレートです。v3（SHIORI/3.0 全文）と v4（ヘッダ部分だけ）の文字列形式の差を吸収し、`Ghost` にはヘッダ部分の原文を渡します。中身は解釈しません。

ヘッダの型（`Headers`・`HeaderWriter`）、プロパティの取得（`Properties`）、Tag の待ち合わせ（`PendingTags`）、語の定数（`ID_GETPROPERTY` など）と組み立て・読み分けの関数は、便利関数のモジュール `shiori4::protocol` にあります。使わなくても仕様に従えます。語の意味は [ホスト側プロトコル仕様](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md) が定めます。

## 想定する利用者

SHIORI 作者です。

## 使い始めの入口

`Ghost` trait を実装し（`request`・`notify` は原文の `&str` を受けます。解析が要るなら `protocol::Headers::parse` を使います）、`export_ghost!` でエクスポートします。クレートの種類は `crate-type = ["cdylib"]` にします。

## 他の 2 クレートとの関係

- [shiori4_abi](https://crates.io/crates/shiori4_abi) に依存し、それを再エクスポートします
- ホスト側の [shiori4_host](https://crates.io/crates/shiori4_host) と、便利関数のモジュール `protocol` を共有します（`shiori4_host::protocol` は `shiori4::protocol` の再エクスポートです）

## 詳しいドキュメント

使用例、v3 と v4 の扱いの違い、設計方針は [docs.rs](https://docs.rs/shiori4) のクレートのドキュメントにあります。

規約の理由や書き方の手引きは [shiori4 の ABI の教科書](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/README.md) にあります（読む順は目次にあります）。SHIORI の DLL の名前と置き場所は [配布の決まり](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/distribution.md) に、SHIORI/3.0 からの移り方は [移行の手引き](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/migration.md) にあります。

## ライセンス

MIT ライセンスです。本文はクレートに同梱の [LICENSE](https://github.com/ekicyou/shiori4/blob/main/shiori4/LICENSE) にあります。
