# shiori4_host

shiori4 のホスト（ベースウェア）側の高水準層です。

## 役割

SHIORI（v4）を読み込んで動かすホストを作るためのクレートです。SHIORI のライブラリの読み込みとライフタイムの管理、SHIORI からの同期の要求と post を原文のまま `Host` に渡すこと、post のキュー、完了やイベントの配送を引き受けます。中身は解釈しません。語の意味は [ホスト側プロトコル仕様](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md) が定めます。

## 想定する利用者

ベースウェア作者です。

## 使い始めの入口

`Host` trait を実装し（`name` は必須です。`request` は原文を受け、既定ではどの要求にも 400 を返します。getproperty には `protocol::getproperty_response` で答えます）、`GhostRuntime::load` で SHIORI を読み込みます（ゴーストのディレクトリは絶対パスで渡します）。`Host::wake` が呼ばれたら、イベントループから `GhostRuntime::pump` を呼びます。

v4 と v3 互換モードの DLL を同じ手順で読みたいときは `load_ghost` を、v3 互換モードの DLL だけを読むときは `V3Runtime` を使います。x86 の Windows では、従来の SHIORI/3.0 の DLL（`load`・`loadu`・`request`・`unload`）も `load_ghost_with` で見つけ、`LegacyRuntime` で読めます（ライブラリのパスも絶対パスで渡します）。

## 他の 2 クレートとの関係

[shiori4](https://crates.io/crates/shiori4) に依存し、それを再エクスポートして使います。ABI の定義（[shiori4_abi](https://crates.io/crates/shiori4_abi)）は shiori4 を通して使い、ヘッダの型・語の定数・完了通知の組み立て（`host_complete`）などの便利関数は `shiori4_host::protocol`（`shiori4::protocol` の再エクスポート）から使えます。

## 詳しいドキュメント

使用例、スレッドモデル、設計方針は [docs.rs](https://docs.rs/shiori4_host) のクレートのドキュメントにあります。

規約の理由や書き方の手引きは [shiori4 の ABI の教科書](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/README.md) にあります（読む順は目次にあります）。ID・ヘッダ名・ステータスの語は [語の表と確定の範囲](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/vocabulary.md) に、SHIORI を別のプロセスで動かすときの注意は [別のプロセスの注意](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/process.md) に、CI で SHIORI を試す方法は [試験用のホストの使い方](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/testhost.md) にあります。

## ライセンス

MIT ライセンスです。本文はクレートに同梱の [LICENSE](https://github.com/ekicyou/shiori4/blob/main/shiori4_host/LICENSE) にあります。
