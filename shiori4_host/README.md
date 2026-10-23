# shiori4_host

shiori4 のホスト（ベースウェア）側の高水準層です。

## 役割

SHIORI（v4）を読み込んで動かすホストを作るためのクレートです。SHIORI のライブラリの読み込みとライフタイムの管理、SHIORI からの同期リクエストの振り分け、post のキュー、完了やイベントの配送を引き受けます。

## 想定する利用者

ベースウェア作者です。

## 使い始めの入口

`Host` trait を実装し、`GhostRuntime::load` で SHIORI を読み込みます（ゴーストのディレクトリは絶対パスで渡します）。`Host::wake` が呼ばれたら、イベントループから `GhostRuntime::pump` を呼びます。

v4 と v3 互換モードの DLL を同じ手順で読みたいときは `load_ghost` を、v3 互換モードの DLL だけを読むときは `V3Runtime` を使います。

## 他の 2 クレートとの関係

[shiori4](https://crates.io/crates/shiori4) に依存し、それを再エクスポートして使います。ABI の定義（[shiori4_abi](https://crates.io/crates/shiori4_abi)）は shiori4 を通して使い、メッセージ名の語彙（`host_protocol`）は shiori4 と共有します。

## 詳しいドキュメント

使用例、スレッドモデル、設計方針は [docs.rs](https://docs.rs/shiori4_host) のクレートのドキュメントにあります。

## ライセンス

MIT ライセンスです。本文はクレートに同梱の [LICENSE](https://github.com/ekicyou/shiori4/blob/main/shiori4_host/LICENSE) にあります。
