# Brief: host-protocol

## Problem
v4 で SHIORI とホストが交わすメッセージの語彙は、`shiori4::host_protocol` に「暫定。ホスト側プロトコル仕様で確定させる」として置かれているだけである。今あるのはプロパティの読み・post の `Command` と `Tag`・完了通知 `OnHostComplete` の名前だけで、足りないものが多い。

- プロパティの書き込みが無い。
- post のコマンドが 1 つも決まっていない。
- 拒否の返し方が決まっていない。
- ホストが必ず付けるヘッダが決まっていない。
- SHIORI の名前と版をホストが知る手段が無い。

SHIORI 実装者（pasta）にとって v4 の最大の利点は「SHIORI から今すぐ台本を出せる」ことだが、それを表す語が無い。ベースウェア実装者（areka）は、自分の spec `shiori4-api` の上流として参照できる正本を必要としている。

## Current State
- `shiori4/src/lib.rs` の `host_protocol` モジュール（`ID_PROPERTY`・`HEADER_VALUE`・`HEADER_COMMAND`・`HEADER_TAG`・`ID_HOST_COMPLETE`・`HEADER_STATUS`）。
- `shiori4_host` の `Host` trait と `Bridge` の組み込みの処理:
  - `property` に 200 ＋ `Value` か 204 で答える（値の改行は取り除く）
  - `accept_post` は `Command` の有無だけを見る
  - 完了通知は request か notify のどちらかでホストが届ける

2026-10-23 に、pasta と areka の意見に shiori4 側の推奨を返して合意した（下の「合意した論点」）。

## Desired Outcome
- ホスト側プロトコル仕様（文書）が shiori4 リポジトリにあり、pasta・areka・shiori4 の 3 者の正本になっている。
- `host_protocol` の定数と `shiori4_host` の組み込みの処理が、その仕様どおりに動く。
- SHIORI 側の高水準層（`shiori4`）から、プロパティの読み書きと SEND / NOTIFY の post を型のついた呼び方で使える。

## 合意した論点（pasta = P、areka = A）
1. **プロパティの読み書き**（A1）:
   - 書き込み: `ID: setproperty` / `Reference0: 名前` / `Reference1: 値` → 204 成功・400 不正。
   - 読み: ukadoc の語に寄せて `property` を `getproperty` に改名し、get / set の対にする。この改名は 0.0.1 の動きを壊す唯一の変更だが、利用者はまだいないので 0.0.x のうちに入れる。
2. **post の語彙**（A2・P4）: SSTP の語に寄せる。post は「同じプロセスからの SSTP」として扱う。pasta の `Command: script` 案は取り下げられた。
   - `Command: SEND` ＋ `Script: <さくらスクリプト>`: 今すぐ再生する。
   - `Command: NOTIFY` ＋ `Event` / `Reference*`: ホストがそのイベントを SHIORI に送る。
3. **post の宛先**（P 追加 a）: 宛先は「post したインスタンス」に固定する。SSTP の `IfGhost` などの宛先指定は読まない。
4. **プロパティ名の決まり**（A4）:
   - 名前は ukadoc のプロパティシステムの名前をそのまま使う。
   - ベースウェア固有の名前は接頭辞（例 `areka.`）で分ける。
   - 大文字小文字は区別しない。
   - 値の改行は取り除く（エスケープして残すことはしない）。改行を含む値が要るプロパティが出たら、別の名前で足す。
5. **v4 の要求ヘッダの語彙**（A7・P7。3 者会談で確定）:
   - v4 のヘッダの名前と意味は SHIORI/3.0（ukadoc）と同じ。
   - ホストが送るイベント（OnBoot・OnSecondChange などの ID と Reference の並び）と、SHIORI の応答のヘッダ（Value・ValueNotify・Marker・ErrorLevel・ErrorDescription など）の名前と意味も、ukadoc の SHIORI/3.0 と同じ。違いは、開始行が無いことと Charset が無いことだけ（pasta の最終確認より）。
   - 必須: ID・Sender（ベースウェアの名前）・SenderType（ukadoc の語）・SecurityLevel（`local` / `external` の 2 語だけ）・SecurityOrigin（ukadoc の形。`null` か起点）。
   - 状況による: Status（伝える状態があるときだけ。ukadoc の語・カンマ区切り）・BaseID（ukadoc が定めるイベントだけ）・Reference*。
   - Charset は付けない（abi-conventions）。
   - post から起きた要求（SEND の中の `\![raise]`、NOTIFY で起こしたイベント）には `SenderType: internal`・`SecurityLevel: local` を付ける。
6. **SHIORI の名前と版**（P7。3 者会談で確定）: SHIORI/3.0 と同じ。`ID: version`（ほかに `name`・`craftman`・`craftmanw`）の request に、200 ＋ `Value:` で答える。答えなければ 204。ABI の版はヘッダに載せない（`shiori_get_api` の引数と `abi_version` で分かる）。
7. **OnHostComplete**（A8。並びは 3 者会談で確定）:
   - 届け方の既定は request（結果を台本で返す必要がありうるため）。SHIORI は request と notify の両方を受けられなければならない。
   - 共通のヘッダ: `Tag`・`Status`・`Reference0: <コマンド名>`。
   - SEND の完了は「再生が終わった時」に出す。
     - 200: 再生した。Reference1 は `completed` か `interrupted`。
     - 204: 一度も再生せずに捨てた（nobreak で待っている間に終了・切り替えが来た、ホストが受けられない状態だった）。Reference1 は無し。
     - 400 ＋ `Reason`: 不正・拒否。
   - NOTIFY: Reference1 は無し。起こせたかどうかを 200 / 400 で返す。
   - OnHostComplete への応答のスクリプトは、`Option: nobreak` の SEND と同じに扱う。つまり再生中なら終わるまで待ち、待つ間に終了・切り替えが来たら捨てる。理由: interrupted の通知は、割り込んだトークの再生中に届く。通常の応答と同じに扱うと、割り込んだ側をさらに置き換えてしまう。
8. **拒否の返し方**（A3。3 者会談で確定）: 400 ＋ `Reason: forbidden`。これで「存在しない（204）」と区別する。403 は足さない。理由: ステータスを SHIORI/3.0 の集合に収め、v3 と v4 で要求と応答のコードを分けないため。
9. **SEND の割り込み**（P 追加 b。3 者会談で確定。shiori4 の当初の推奨「既定は待つ」から変えた）:
   - 既定と待ち方は SSTP に揃える。
     - post の SEND は、既定で割り込む。
     - `Option: nobreak` のときだけ、今のスクリプトが終わるまで待つ。
     - 新しいヘッダ（Interrupt）は作らない。
     - 理由: post は「同じプロセスからの SSTP」なので、語・既定・ukadoc の参照・areka の既存の裁定（mcp-kanade-tools の 1〜3）を 1 つにできる。
   - 優先度は Owned SSTP と同じ（SHIORI 自身の依頼）。`\t` が入っていても割り込める。
   - 待つための列はホストの仕事（areka が引き受ける）。
   - 割り込まれた側への知らせに OnBalloonBreak は使わない（ukadoc では「SSTP 以外でブレイクされた際」のイベント）。
     - 切られたトークが Tag 付きの SEND から始まったものなら、その OnHostComplete が `interrupted` を運ぶ。
     - 通常のイベントの応答として出たトークが SEND に切られたときは、何も出さない（SSP と同じ）。
     - 利用者の中断なら、今どおり OnBalloonBreak。

10. **SEND が読む SSTP の Option**（areka C）:
    - `nobreak`・`notranslate`・`strict` は ukadoc どおりに効く。
    - `nodescript` は SSTP の表示の話なので読まない。
    - 知らない語は無視する（SSTP と同じ）。
11. **NOTIFY で起こせるイベント**（areka D）: 名前に制限は掛けない（台本の `\![raise]` と同じ）。ホストは安全上の理由で、特定のイベントを 400 ＋ `Reason: forbidden` で断れる。
12. **`Reason` の語**（areka E）: 当面は `forbidden` だけを定める。未知の語は「理由不明の拒否」と読む。

上の 1〜9 は、pasta・areka の 2 者が合意し、2026-10-23 に shiori4 へ届けたもの（割れたものは無い）。10〜12 は areka の追加の提案で、3 者とも同意した。要件の段では、これを開発者が承認する。

## Approach
仕様の本文は、新しい文書 `doc/host-protocol.md` に置く。置き場所は設計で確定させる。`shiori4` の crate doc と `host_protocol` モジュールの doc はその文書を指す。

- 定数は `shiori4::host_protocol` に足す。
- `shiori4_host` の `Host` trait と `Bridge` の組み込みの処理（getproperty・setproperty・accept_post の既定・完了通知）を、仕様どおりにする。
- SHIORI 側の高水準層には、get / set のプロパティと、SEND / NOTIFY の post の呼び方を足す。

## Scope
- **In**:
  - ホスト側プロトコル仕様の文書。
  - `host_protocol` の定数（改名と追加）。
  - `shiori4` のプロパティと post の呼び方。
  - `shiori4_host` の `Host` trait の追加メソッド（setproperty など）と組み込みの処理。
  - 上のテスト。
  - pasta・areka への正本の場所の連絡。
- **Out**:
  - ABI の規約（スレッド・寿命・log・load の dir・Charset）。これは abi-conventions。
  - SSTP そのもの（ネットワーク越しの SSTP の受け口）。
  - ukadoc のプロパティの一覧を shiori4 が持つこと（名前の決まりだけを持つ）。
  - ベースウェアの再生キューの実装。

## Boundary Candidates
- 仕様の文書（語彙の正本）
- SHIORI 側の型のついた呼び方（`shiori4`）
- ホスト側の組み込みの処理と `Host` trait の拡張（`shiori4_host`）

## Out of Boundary
- ABI の関数テーブルとスレッド規約（abi-conventions）
- areka の SSTP の受け口・安全度の扱い（areka が持つ）
- pasta のシーン再生の仕組み（pasta が持つ）

## Upstream / Downstream
- **Upstream**: v0.0.1 の ABI と `host_protocol`。abi-conventions とは独立だが、同じファイルに触れる
- **Downstream**:
  - areka の spec `shiori4-api`（areka リポジトリ）が、この仕様を上流として参照する。
  - pasta の v4 対応。
  - 0.0.2 のリリース。

## Existing Spec Touchpoints
- **Extends**: なし
- **Adjacent**: abi-conventions（`shiori4/src/lib.rs`・`shiori4_host/src/lib.rs` の同時編集）

## Constraints
- 確定した語は shiori4 を正本とする。areka・pasta の文書は shiori4 の仕様を参照し、語を写して変えない。
- property の改名以外は、0.0.1 の動きを変えない（足すだけ）。
- ヘッダの書式は SHIORI/3.0 と同じ（`名前: 値`、CRLF、ヘッダ部分だけ）。
- 要件の段に入ったら、決まった語を pasta・areka の両セッションへ知らせる。

## 下流の予定（参考・2026-10-23 pasta より）
- pasta 0.4.2: shiori4 の v3 互換モード（`shiori_get_api(3)`）を出し、Windows 以外でも SHIORI/3.0 として動かす。
- pasta 0.5.0: v4（host.request・post・OnHostComplete）に本格対応する。abi-conventions と host-protocol の 2 本を前提にする。
- areka v0.2.0: 自前の COM の SHIORI（IShiori 系）と JSON-RPC の紙の契約をやめ、shiori4 に完全に移る（spec `shiori4-api`）。着手の条件は、host-protocol が入った shiori4 の版が crates.io に出ていること（2026-10-23 areka より）。
