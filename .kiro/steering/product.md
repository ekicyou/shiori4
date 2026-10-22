# Product Overview

shiori4 は、伺か（Ukagaka）の SHIORI（ゴーストの頭脳 DLL）とベースウェア（ホスト）をつなぐ**次世代 ABI** と、その両側の Rust 実装ライブラリを提供するワークスペースである。
対象は SHIORI の作者（ゴースト開発者）と、ベースウェアの作者の両方。

## Core Capabilities

- **新 ABI の定義**: 公開シンボル `shiori_get_api` 1つと `#[repr(C)]` の関数テーブルで、DLL 境界の所有権・スレッド・版の規約を固定する（C ヘッダ付き。Rust 以外の実装も対象）
- **v4 / v3 の両モード**: v4（ヘッダ部分だけ・ホストを呼べる）が既定。v3 は SHIORI/3.0 全文を運ぶ互換モード（他 OS の互換ベースウェア向け）
- **SHIORI → ホストの呼び出し**: 同期の `host.request`（プロパティ取得など）と、任意スレッドからの非同期 `host.post` + `Tag` による完了通知
- **SHIORI 側の高水準層**: `Ghost` trait を実装して `export_ghost!` するだけで、v3/v4 の文字列形式の差を吸収した SHIORI が作れる
- **ホスト側の高水準層**: `Host` trait と `GhostRuntime` で、読み込み・post キュー・完了イベントの配送・安全な unload を一括管理する

## Target Use Cases

- 新しい SHIORI を Rust で書く（`Ghost` を実装し cdylib として出す）
- 新しいベースウェア、または既存ベースウェアの新 ABI 対応を Rust で書く（`Host` を実装する）
- 旧 SHIORI と新 SHIORI の判別（`shiori_get_api` の有無）

## Value Proposition

- 従来の SHIORI/3.0 は「文字列を1往復するだけ」だった。shiori4 は SHIORI からホストへの同期・非同期の呼び出しを、**再入禁止・単一スレッド・unload 時の破棄**といった規約込みで提供する
- ABI 層（バイト列の運搬）とプロトコル層（中身の解釈）を分離し、ABI を小さく安定に保つ
- 構造体は末尾追加 + `struct_size` 判定で、版を上げずに後方互換を保つ

---
_Focus on patterns and purpose, not exhaustive feature lists_
