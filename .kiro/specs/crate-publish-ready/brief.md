# Brief: crate-publish-ready

## Problem
3 クレートは手元でビルド・テストできるが、このままでは crates.io に出せない。開発者（ekicyou）が公開しようとしても `cargo publish` が断るうえ、PR で壊れたことに気付く仕組みも無い。

## Current State
- 各クレートの `version = "0.1.0"` がばらばらに直書きされている（`[workspace.package]` に version が無い）
- 内部依存が path だけ（`shiori4_abi = { path = "../shiori4_abi" }`）。`cargo publish` は version の無い path 依存を断る
- `repository` などのメタデータが無い。クレートごとの README が無い（ルートの README.md だけ）
- `license = "MIT OR Apache-2.0"` なのに、LICENSE ファイルは MIT だけ
- `shiori4_abi/include/shiori4_abi.h` が公開されるパッケージに入るかを確かめていない
- `.github/` が無い（CI なし）。crates.io に `shiori4`・`shiori4_abi`・`shiori4_host` の名前はまだ無い（2026-10-22 時点で 404）

## Desired Outcome
- 3 クレートで `cargo package`（`cargo publish --dry-run`）が依存の順に通る
- 版は `[workspace.package] version = "0.0.1"` の 1 か所と、`[workspace.dependencies]` の内部依存の `version` だけにある
- PR と main への push で、ubuntu / windows / macos の 3 OS で `cargo test --all` と `cargo clippy --all-targets --workspace -- -D warnings` が走り、cargo-deny が依存のライセンス・脆弱性・出どころを検査する

## Approach
pasta の Cargo.toml の流儀に揃える。`[workspace.package]` に version・authors・license・repository・homepage・documentation を置き、各クレートは `.workspace = true` で受ける。内部依存は `[workspace.dependencies]` に path と version を並べて書き、各クレートは `xxx.workspace = true` で使う。
build.yml は pasta の build.yml から pasta 固有の job（luacheck・wasm・dll・ロケール）を落とし、3 OS の matrix にする。`on:` に `workflow_call: {}` を入れて、後続の release-workflow が gate として呼べるようにする。deny.toml は pasta のものを MIT 系の許可リストに合わせて持ってくる。

## Scope
- **In**:
  - ルートと各クレートの Cargo.toml（版の一本化・内部依存・メタデータ・`license = "MIT"`）
  - 各クレートの README.md（cargo の既定で拾われる場所）。ルート README.md は入口として残す
  - C ヘッダがパッケージに入ることの確認（必要なら `include`）
  - `cargo package` が通ることの確認（手元とCI）
  - `.github/workflows/build.yml`（3 OS の test・clippy、cargo-deny、`workflow_call`）と `deny.toml`
  - steering（tech.md・structure.md）の版・メタデータ・CI の記述
- **Out**:
  - 実際の公開（初回の手動公開も含む）
  - release.yml・公開スクリプト・`/release` スキル・リリースノート
  - クレートの API や中身の変更

## Boundary Candidates
- クレートのメタデータと版の置き場所（Cargo.toml）
- 利用者向けの README（crates.io と docs.rs の顔）
- PR の CI（build.yml・deny.toml）

## Out of Boundary
- タグで動くもの・公開に関わる認証（release-workflow が持つ）
- workflow.md の「前提: 完成の形」・リモート同期の節の書き直し（release-workflow が持つ）

## Upstream / Downstream
- **Upstream**: 既存の 3 クレート、`.kiro/steering/tech.md`・`structure.md`
- **Downstream**: release-workflow（build.yml を gate として呼び、版の置き場所を書き換える）

## Existing Spec Touchpoints
- **Extends**: なし
- **Adjacent**: release-workflow（build.yml の `workflow_call` と版の置き場所が継ぎ目）

## Constraints
- 初版 0.0.1、3 クレートは同じ版
- ライセンスは MIT だけ
- Rust 1.85 / edition 2024 のまま（`rust-version` は workspace で指定済み）
- CI は 3 OS（ubuntu-latest / windows-latest / macos-latest）
- `Cargo.lock` はコミットしない方針（tech.md）。ただし release-workflow が pasta と同じく `--locked` で公開するなら、lock をコミットする方針への変更が要る。この spec の設計で決め、tech.md を直す
