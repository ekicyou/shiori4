# shiori4 の ABI の教科書

SHIORI の作者とベースウェアの作者に向けて、shiori4 の ABI の規約と、SHIORI とホストが交わすメッセージの語を、決まった順で読めるようにまとめた教科書の目次である。

- **当てはまる版**: shiori4 0.1.0 から。`v0.0.2` までのタグにこの教科書は無く、この教科書が述べる応答の作り方（ABI 規約 5）は 0.0.2 以前と二進互換でない（[移行の手引き](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/migration.md)の「0.0.2 以前の shiori4 から」）。`main` の文書には未公開の版の内容が載りうるので、公開された版の内容は、その版のタグ `vX.Y.Z` の文書で読む（[`doc/host-protocol.md`](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md) 第 0 章と同じ扱い）。
- 読み物の本文は、このリポジトリの `doc/` の文書と、docs.rs のクレートのドキュメントだけに置く。

## 読む順

1. [全体図と最小の例](https://github.com/ekicyou/shiori4/blob/main/README.md)（ルートの README）: 誰が誰を、どのスレッドから、いつ呼べるかの表と、Rust と C の SHIORI とホストの最小の例
2. [規約の一覧と理由](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/rules.md): 規約の文と番号、各規約の理由、番号の付け方と引用の仕方
3. [語の表と確定の範囲](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/vocabulary.md): SHIORI とホストが交わすメッセージの語と、版ごとに確定している語と API
4. 細部
   - [docs.rs の `shiori4`](https://docs.rs/shiori4): SHIORI 側の例
   - [docs.rs の `shiori4_host`](https://docs.rs/shiori4_host): ホスト側の例
   - [docs.rs の `shiori4_abi`](https://docs.rs/shiori4_abi): ABI の型と規約の正本
   - [C の書き手向けの説明](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/c.md): 所有権・版の判定・スレッド・例外を C の言葉で。C の例は [`doc/textbook/c/`](https://github.com/ekicyou/shiori4/tree/main/doc/textbook/c) にある
   - [移行の手引き](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/migration.md): SHIORI/3.0 の SHIORI やホストと、0.0.2 以前の shiori4 から移るとき
   - [配布の決まり](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/distribution.md): SHIORI のファイルの名前と、ホストの選び方
5. 補い
   - [別のプロセスの注意](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/process.md): SHIORI を専用のプロセスで読むとき
   - [試験用のホストの使い方](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/testhost.md): ベースウェア無しで SHIORI を読んで試すときと、他のリポジトリの CI に取り込むとき

## 正本

教科書は正本を写し、言い直すだけである。食い違ったら正本が正しい。

- 規約: C ヘッダ [`shiori4_abi/include/shiori4_abi.h`](https://github.com/ekicyou/shiori4/blob/main/shiori4_abi/include/shiori4_abi.h) の冒頭の注釈と、`shiori4_abi` のクレート冒頭の `//!`（[docs.rs](https://docs.rs/shiori4_abi)）の 2 つで、同じ文を持つ。
- メッセージの語: [`doc/host-protocol.md`](https://github.com/ekicyou/shiori4/blob/main/doc/host-protocol.md)（ホスト側プロトコル仕様）。
- 試験用のホストの引数と終了の状態: [`shiori4_host/examples/shiori4-testhost.rs`](https://github.com/ekicyou/shiori4/blob/main/shiori4_host/examples/shiori4-testhost.rs) のコード（[試験用のホストの使い方](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/testhost.md)の「この章の読み方」）。

## 正本の欠けを見つけたら

正本の規約や語に、欠け・食い違い・二通りに読める所が見つかっても、教科書はその場で決めない。教科書の該当箇所には「規約に定めが無い」とだけ書き、見つけたものは abi-textbook の research.md の「正本の未決」に記録して、開発者の裁定へ回す。正本を直すときは、新しい spec か修正の PR になる。

## 例の注記

ビルドだけを検査する例と検査しない例は、その理由を例の直前の文に書く。実行して確かめる例も、何を相手に実行するかを書くことが多い。

- 実行して結果を確かめる例: 「この例は偽のホストで往復を実行して確かめている」「この例は DLL なしに `GhostRuntime::from_static` で往復を実行して確かめている」のように書く。
- ビルドだけを検査する例: 「この例は DLL を要するので、ビルドだけを検査する（実行しない）。」のように、実行しない理由を書く。
- 検査しない例: 「この例は検査しない（他のリポジトリの CI の中でしか動かせないため）」のように、検査しない理由を書く。

C の例（[`doc/textbook/c/`](https://github.com/ekicyou/shiori4/tree/main/doc/textbook/c)）は、CI が 3 つの OS でビルドして動かす。通らない枝があるときは、その枝がビルドだけを検査されることと理由を、例の隣に書く（[C の書き手向けの説明](https://github.com/ekicyou/shiori4/blob/main/doc/textbook/c.md)の「この章の読み方」）。
