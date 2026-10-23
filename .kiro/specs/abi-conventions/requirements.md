# Requirements Document

## Project Description (Input)
SHIORI 実装者（pasta）とベースウェア実装者（areka）が、`shiori4_abi.h`（main 72cff0b）の規約を読んで v4 対応を設計しようとしている。ところが、いくつかの規約は意味が一通りに読めず、手段を指定していて守れない規約や、何も書いていない所もある。このまま実装すると、両者が別の読み方をしてデッドロックや解放後の呼び出しを起こす。

いまの規約は C ヘッダ冒頭の「ABI 規約」「スレッド規約」「post の完了通知」と Rust 版 `shiori4_abi` の型にあり、高水準層（`shiori4` の `ShioriHostPoster`・`export_ghost!`、`shiori4_host` の `GhostRuntime`）はそれで作られている。2026-10-23 に pasta・areka・shiori4 の 3 者で論点 1〜17 を合意・裁定した（brief.md）。

この spec では、C ヘッダ・Rust の doc・高水準層の 3 つが同じ規約を同じ言葉で述べ、合意した論点がすべてそこに入り、高水準層を使う Ghost 実装者・Host 実装者が join や自前のスレッド検査なしで規約を守れるようにする。

## Introduction
この spec は、SHIORI 次世代 ABI の規約（スレッド・寿命・load の失敗・dir の形・Charset・DLL の選び方と読み方）を、3 者（pasta・areka・shiori4）の合意どおりに確定させる。規約の本文は C ヘッダ `shiori4_abi/include/shiori4_abi.h` の冒頭の注釈を正本とし、Rust 版 `shiori4_abi` の doc と、高水準層（`shiori4`・`shiori4_host`）の doc と振る舞いをそれに合わせる。あわせて、ホスト側の高水準層 `shiori4_host` に、DLL の選び方・見る順・読み方の口（論点 12・13・15・16・17）を足す。

要件の番号と brief.md の論点の対応は、各要件の目的の末尾に「（論点 N）」で示す。brief.md で決め切れていない所は、要件の中で **【仮定】** と明示し、末尾の「未決の論点」に挙げる。

## Boundary Context
- **In scope**:
  - C ヘッダの規約の注釈（正本）、`SHIORI_E_WRONG_THREAD`（-7）の定数（C と Rust）、Rust 版 `shiori4_abi` の doc。
  - `shiori4_abi` の SHIORI 側の入口（`shiori_load`／`shiori_unload`）: `ShioriHostPoster`（`shiori4_abi` にあり、`shiori4` が再エクスポートする）の無効化。
  - `shiori4`: `Ghost`／`export_ghost!` まわりの doc。
  - ホスト側の高水準層 `shiori4_host`: スレッドの検査と -7、load の dir の正規化、log と release の任意スレッド対応の確認、Charset を付けないこと、両方を持つ DLL の見る順と許可する口の種類、読み終えたライブラリから作る入口、ファイル名の候補とヘッダの機械の種類の確認、v3 互換モードの口。
  - 上の振る舞いのテスト。
- **Out of scope**:
  - 運ぶ中身の語彙（property・post のコマンド・必ず付けるヘッダ・version・OnHostComplete、拒否の返し方）。host-protocol が持つ。
  - 教科書としての文書（全体図・移行の手引き・最小の例・配布の決まりの章）。abi-textbook が持つ。
  - 従来の SHIORI/3.0 の DLL（HGLOBAL の load・loadu・unload・request）を読むこと。shiori3-legacy-host が持つ。
  - x86 の v4 DLL を別プロセス越しに読むこと。
  - ベースウェア・SHIORI の個別の実装（pasta のアクタースレッド・areka の専用スレッドや helper のプロセス）。
  - 版を上げること（`/release` が上げる）。
- **Adjacent expectations**:
  - host-protocol は同じ `shiori4/src/lib.rs`・`shiori4_host/src/lib.rs` に触れる。どちらかを先に統合し、後の側が取り込む。拒否は 400 ＋ `Reason` で返し、ABI の定数は足さない。
  - shiori3-legacy-host は、この spec が決める「発見の順」「許可する口の種類」「dir の形」を前提にし、`loadu`・`load` で解決した DLL を読む部分を後から足す。
  - abi-textbook は、この spec が確定させた規約の本文と doc を上流として参照する。

## Requirements

### Requirement 1: 規約の正本と 3 か所の一致
**Objective:** As a SHIORI 実装者・ベースウェア実装者（Rust 以外の実装者を含む）, I want C ヘッダ・Rust の doc・高水準層の doc が同じ規約を同じ言葉で述べていること, so that どれを読んでも同じ一通りの意味に読める（全論点）

#### Acceptance Criteria
1. The C ヘッダ shall この spec の要件 2〜11 で定める規約の本文（要件 11 は発見の順の要約）を冒頭の注釈に、要件 14 の配布の決まりを規約の後の節に、日本語で持つ（規約の正本）。
2. The shiori4_abi shall クレート冒頭の doc に、C ヘッダと同じ番号・同じ言葉で同じ規約を持つ。
3. The shiori4_abi・shiori4・shiori4_host と steering の `tech.md` shall 規約に触れる doc（`Ghost`・`Shiori`・`ShioriHostPoster`・`Host`・`GhostRuntime`・`tech.md` のスレッド規約など）で、C ヘッダの規約と食い違う記述（「Drop で止めて join すること」「unload の中で post するスレッドをすべて止めてから返る」など）を持たない。
4. If C ヘッダと Rust の doc の一方にだけある規約がある（例: Rust にだけある「エクスポートする関数はパニックを SHIORI_E_PANIC に変換する」）, the C ヘッダ と shiori4_abi shall 両方に同じ規約を持つ。パニックの規約は、言語によらない形（「エクスポートする関数は、例外・パニックを境界の外へ出さず `SHIORI_E_PANIC` を返す」）で C ヘッダの規約に入れる（議題 3）。
5. The C ヘッダ と shiori4_abi shall 構造体のレイアウト（`SHIORI_STATIC_ASSERT` とコンパイル時の検査が確かめる大きさ・オフセット）を変えない。
6. The C ヘッダ と shiori4_abi shall 既存の公開シンボル・関数テーブル・定数の値を変えず、足すのは定数と注釈（関数テーブルは末尾追加の規則に従う）だけにする。

### Requirement 2: host->request を呼べるスレッドと期間
**Objective:** As a SHIORI 実装者, I want `host->request` を呼べるスレッドと期間が一通りに決まっていること, so that ワーカースレッドから呼んでデッドロックを起こさない（論点 1・9）

#### Acceptance Criteria
1. The C ヘッダ shall スレッド規約 2 を「SHIORI が `host->request` を呼べるのは、load / request / notify を呼ばれているスレッドから、その呼び出しが返るまでの間だけ（v4）」と述べる。
2. The C ヘッダ と shiori4_abi shall ABI エラー `SHIORI_E_WRONG_THREAD` を値 -7 で定義し、「規約 2 に反する `host->request` の呼び出し」を意味すると述べる。
3. The C ヘッダ shall 「ホストは、規約 2 に反する呼び出しを見分けたら `SHIORI_E_WRONG_THREAD` を返してよい（out は空）」と述べる。
4. The C ヘッダ shall スレッド規約 1 の「ホストが決めた単一のスレッド」が、プロセスで 1 つではなくインスタンスごとであること、同じ DLL の 2 つのインスタンスが別々のスレッドから呼ばれうること、`SHIORI_E_WRONG_THREAD` の判定もインスタンスごとであることを述べる。
5. The shiori4 と shiori4_host shall `SHIORI_E_WRONG_THREAD` と既存のすべての `SHIORI_E_*`（今 `shiori4_host` の再エクスポートから漏れている `SHIORI_E_INVALID_UTF8` を含む）を、利用者が 1 クレートを use するだけで使えるように再エクスポートする。

### Requirement 3: ホスト側の高水準層による規約 2 の違反の検出
**Objective:** As a ベースウェア実装者, I want `shiori4_host` が規約 2 の違反を見分けて -7 を返すこと, so that 自前でスレッドを検査しなくても規約を守れ、SHIORI 実装者も違反に気づける（論点 1・9）

#### Acceptance Criteria
1. When SHIORI が、そのインスタンスの load / request / notify を呼ばれているスレッドとは別のスレッドから `host->request` を呼ぶ, the shiori4_host shall `SHIORI_E_WRONG_THREAD` を返し、その要求を `Host` に渡さない。
2. When SHIORI が、そのインスタンスの load / request / notify の処理中に、呼ばれているスレッドから `host->request` を呼ぶ, the shiori4_host shall これまでどおり要求を `Host` に振り分けて応答を返す。
3. If SHIORI が、そのインスタンスの load / request / notify の処理中ではないときに `host->request` を呼ぶ, the shiori4_host shall `SHIORI_E_WRONG_THREAD` を返し、その要求を `Host` に渡さない。同じスレッドでも、呼び出しの外は規約 2 の期間の外なので -7 とする（論点 1）。
4. While 同じプロセスで複数の `GhostRuntime` が別々のスレッドで動いている, the shiori4_host shall 判定をインスタンスごとに行い、あるインスタンスの正しい呼び出しを、別のインスタンスのスレッドを理由に拒否しない。
5. The shiori4_host shall この検出のために、Host 実装者・Ghost 実装者に新しい手続き（スレッドの登録など）を求めない。

### Requirement 4: unload の約束と ShioriHostPoster の無効化
**Objective:** As a SHIORI 実装者, I want unload の約束が手段ではなく結果で書かれ、高水準層がそれを守ってくれること, so that join を書かなくても解放後の呼び出しを起こさない（論点 2・10）

#### Acceptance Criteria
1. The C ヘッダ shall スレッド規約 5 から「SHIORI は unload の中で、post を呼ぶ自分のスレッドをすべて止めてから返る」という手段の指定を除き、「SHIORI は、unload が返った後に ShioriHostApi のどの関数も呼ばない」という結果の約束に置き換える。
2. The C ヘッダ shall 「unload は長く止まらないこと。ホストは OS の終了の猶予の中で、各インスタンスの unload を呼ぶことがある」という注意を、時間の数字を書かず、SHIORI に join を求めない言い回しで持つ。
3. When ホストがそのインスタンスの unload を始める, the shiori4_abi（`shiori_unload`） shall そのインスタンスから取り出したすべての `ShioriHostPoster` を無効にする。
4. While `ShioriHostPoster` が無効である, the shiori4_abi shall その `post` の呼び出しでホストに触れず、`SHIORI_E_SHUTDOWN` を返す。
5. When unload が返る, the shiori4_abi shall それ以後、そのインスタンスの `ShioriHostPoster` を通じて ShioriHostApi のどの関数も呼ばれない状態にしている（unload を始める前に始まっていた post の呼び出しも含む）。そのため unload は、門を閉じた時点で実行中の post の呼び出しが返るのを待ってよい。待つのは post 1 回分（規約 4 でコピーしてすぐ返る）だけで、Ghost 実装者のスレッドの終わりを待つ join とは別であり、「join を求めない」に反しない（議題 2）。
6. The shiori4_abi と shiori4 shall Ghost 実装者・`Shiori` を直接実装する人に、unload（Drop）の中で post するスレッドを止めて join することを求めない。そのスレッドが unload の後も動き続けて post を呼んでも、`SHIORI_E_SHUTDOWN` が返るだけで安全である。
7. The shiori4_abi shall `ShioriHostPoster` を使う既存の Ghost のコード（取り出し・複製・別スレッドへの持ち出し・`PendingTags::post`）を、複製の書き方を除いて書き換えなしでビルドできるように保つ。無効化の印を共有するため `ShioriHostPoster` から `Copy` を外し、`Clone` は残す（複製は `.clone()` で書く）。これは 0.0.x の破壊的な変更として、リリースノートに移行の仕方を書く（議題 1）。

### Requirement 5: log と release のスレッド
**Objective:** As a SHIORI 実装者・ベースウェア実装者, I want `host->log` と `ShioriBytes.release` を呼べるスレッドが決まっていること, so that ワーカースレッドからのログや、応答を別スレッドで捨てることを安心してできる（論点 3）

#### Acceptance Criteria
1. The C ヘッダ shall 「`host->log` は post と同じく任意のスレッドから呼べる。ホストは msg をコピーしてすぐ返る」と述べる。
2. The C ヘッダ shall 「`ShioriBytes.release` は、どのスレッドから呼んでもよい」と述べ、release を実装する側（確保した側）がそれに耐える義務を負うことを述べる。
3. When SHIORI が任意のスレッドから `host->log` を呼ぶ, the shiori4_host shall それを受けて `Host::log` に渡し、すぐ返る。
4. The shiori4_host shall `Host::log` の doc で、任意のスレッドから呼ばれることを述べる。
5. The shiori4_abi shall 自分が確保して渡す `ShioriBytes` の release が、どのスレッドから呼ばれても正しく解放されることを保つ。
6. Where shiori4_abi が別スレッドへ持ち出せるホストの口（`ShioriHostPoster`）を持つ, the shiori4_abi shall そこから log も呼べるようにし、無効化（要件 4）の後は log でもホストに触れない（議題 5）。
7. The shiori4_abi shall `ShioriOwnedBytes` と `ShioriOwnedResponse` を、別のスレッドへ渡せる（`Send`）型にする。release がどのスレッドから呼ばれてもよいという規約（5.2）を型で表すため（議題 6）。

### Requirement 6: notify の後回し
**Objective:** As a SHIORI 実装者, I want notify をすぐ返して処理を後回しにしてよいかが決まっていること, so that ホストのスレッドを止めずに重い処理ができる（論点 4）

#### Acceptance Criteria
1. The C ヘッダ shall 「notify を即 204 で返し、処理を後で行う実装は合法である」と述べる。
2. The C ヘッダ shall 「後回しにした処理の中では `host->request` を使えない（規約 2 の期間の外であるため）。ホストへの依頼には post を使う」と注記する。
3. The shiori4 shall `Ghost::notify` の doc で、同じ注意を述べる。

### Requirement 7: load の失敗と複数のインスタンス
**Objective:** As a ベースウェア実装者, I want load の失敗の理由が分かり、2 つ目のインスタンスの扱いが決まっていること, so that 読めなかったゴーストを正しく扱い、1 プロセスで複数のゴーストを動かせる（論点 5・8・16）

#### Acceptance Criteria
1. The C ヘッダ shall 「v4 の load が失敗するときは、NULL を返す前に `host->log` で理由を出す。v3 は host が NULL なので、SHIORI 自身のログに出す」と述べる。
2. The C ヘッダ shall 「SHIORI は同じプロセスでの 2 つ目のインスタンスを拒否してよい（NULL ＋ log）。拒否されたホストは、そのゴーストを読めなかったものとして扱い、ゴーストごとに別のプロセスで読むか、拒否を受け入れる」と述べる。
3. The C ヘッダ shall 「SHIORI は、できれば複数のインスタンスを受けるべきである（状態をインスタンスごとに持ち、グローバルな状態を避ける）」と推奨として述べる。
4. The C ヘッダ shall 「v3 の load は失敗の理由を返さない（NULL を返すだけ）」と述べる。
5. If v4 の Ghost の load が失敗する, the SHIORI 側の高水準層 shall NULL を返す前に、`host->log` に失敗のステータスを含む 1 行を自動で出す。Ghost の load がパニックして失敗したときも出す。詳しい理由は Ghost 実装者が自分で log する（議題 4）。
6. If SHIORI の load が NULL を返す, the shiori4_host shall `LoadError::LoadFailed` を返し、そのゴーストを読めなかったものとしてホストに知らせる（v4 では理由は `Host::log` に届いている）。
7. The shiori4 の `export_ghost!` で作った SHIORI shall 同じプロセスでの複数のインスタンスを受け付ける（インスタンスごとに別の状態を持つ）。

### Requirement 8: load の dir の形とカレントディレクトリ
**Objective:** As a SHIORI 実装者, I want load の dir の形が決まっていて、ホストの高水準層がその形で渡すこと, so that `dir + "file"` のような書き方でも、どのホストでも同じに動く（論点 6・14）

#### Acceptance Criteria
1. The C ヘッダ shall v4 と v3 互換モードの両方について、load の dir を「絶対パス・OS の区切り・末尾に区切りあり・UTF-8」と述べる。
2. The C ヘッダ shall 「カレントディレクトリはホスト次第である。SHIORI はカレントディレクトリに頼らず、load の dir を使う」と述べる。
3. When ホストが `shiori4_host` にゴーストのディレクトリを渡して load する（v4・v3 互換モードのどちらでも）, the shiori4_host shall SHIORI の load に、絶対パス・OS の区切り・末尾に区切りが 1 つ付いた UTF-8 の文字列を渡す。
4. If ホストが相対パスを渡す, the shiori4_host shall それを絶対パスにしてから渡す。**【仮定】** 呼び出した時点のプロセスのカレントディレクトリを基準にし、ディレクトリの存在は確かめない（未決の論点 8）。
5. If ゴーストのディレクトリが UTF-8 で表せない, the shiori4_host shall load を呼ばずに `LoadError::InvalidPath` を返す。
6. The shiori4_host shall 末尾に区切りが付いている入力と付いていない入力を、同じ dir として渡す（区切りを二重にしない）。

### Requirement 9: v4 の Charset
**Objective:** As a SHIORI 実装者・ベースウェア実装者, I want v4 で Charset を付けるかどうかが決まっていること, so that 両者が余計なヘッダを付け合ったり、それで失敗したりしない（論点 7）

#### Acceptance Criteria
1. The C ヘッダ shall 「v4 は UTF-8 固定なので、ホストは要求に Charset ヘッダを付けなくてよい。付いていても SHIORI は無視する。SHIORI は応答に Charset を付けない」と述べる。
2. The shiori4_host shall v4 で SHIORI に届ける要求（組み込みの完了通知を含む）と、`host->request` への応答に、Charset ヘッダを自分から付けない。
3. The shiori4 shall v4 の応答に Charset ヘッダを自分から付けない。
4. If v4 の要求に Charset ヘッダが付いている, the shiori4 shall その値によって要求を拒否しない。
5. While v3 互換モードである, the shiori4 shall これまでどおり Charset の扱い（UTF-8 以外は 400、応答には `Charset: UTF-8` を補う）を保つ。

### Requirement 10: SHIORI を読むプロセス
**Objective:** As a SHIORI 実装者, I want ホストが SHIORI を専用のプロセスで読むことがあると知っていること, so that ホストと同じプロセスにいることを前提にした実装をしない（論点 11）

#### Acceptance Criteria
1. The C ヘッダ shall 「ホストは SHIORI を専用のプロセスで読むことがある。SHIORI は、ホストと同じプロセスにいること（ホストの窓・プロセスの状態を共有すること）を前提にしない」と述べる。
2. The C ヘッダ shall 「`host->request` は IPC を往復することがあり、遅くなりうる」と述べる。
3. The C ヘッダ shall この注意で ABI の形（関数・構造体）を変えない。

### Requirement 11: DLL の口の発見の順と、許可する口の種類
**Objective:** As a ベースウェア実装者・SHIORI 実装者, I want 両方の口を持つ DLL をどの順で見るかが決まり、許可する口を選べること, so that 二重の初期化を避け、従来の口だけ・v4 と v3 だけでの試験や運用ができる（論点 13）

#### Acceptance Criteria
1. The C ヘッダ shall 「1 つの DLL が SHIORI/3.0 の書き出し（load・request・unload）と `shiori_get_api` の両方を持ってよい」と述べ、ホストの見る順（`shiori_get_api` → `loadu` → `load`）を述べる。
2. When ホストが `shiori4_host` で DLL を読み、`shiori_get_api` が見つかる, the shiori4_host shall その DLL を shiori4 の DLL として扱い、v4 を試し、v4 のテーブルが得られなければ v3 互換モードを試す。
3. If `shiori_get_api` が v4 と v3 のどちらにも NULL を返す, the shiori4_host shall そのゴーストを読めなかったものとして返し、`loadu`・`load` へはフォールバックしない。
4. If `shiori_get_api` で解決した口の load が失敗する, the shiori4_host shall `LoadError::LoadFailed` を返し、ほかの版・ほかの口へはフォールバックしない。
5. When `shiori_get_api` が見つからない, the shiori4_host shall `loadu`、無ければ `load` の書き出しを探し、見つかった従来の口を結果としてホストに知らせる。この spec の `shiori4_host` は従来の口で DLL を初期化せず、どの口で解決したかと開いたライブラリを返すだけにする。従来の口で読むのは shiori3-legacy-host が足す（議題 8）。
6. Where ホストが許可する口の種類（`shiori_get_api` の 4・`shiori_get_api` の 3・`loadu`・`load` の部分集合）を渡す, the shiori4_host shall 発見の順を保ったまま許可された口だけを探し、許可されていない口は DLL にあっても無いものとして扱う。
7. If 許可された口の中で最初に見つかった口が失敗する, the shiori4_host shall 許可の中でも次の口へフォールバックせず、失敗を返す。
8. Where ホストが許可する口の種類を渡さない, the shiori4_host shall 4 つの口をすべて許可したものとして扱う。
9. The shiori4_host shall `shiori_get_api` を知らない古いホストが、両方の口を持つ DLL を従来の口で今までどおり読めることを妨げない（DLL 側に追加の義務を課さない）。

### Requirement 12: 読み終えたライブラリから作る入口
**Objective:** As a ベースウェア実装者, I want 自分で開いたライブラリから `GhostRuntime` を作れること, so that DLL を 2 回読まずに「4 → 3 → SHIORI/3.0」の順で試せる（論点 15）

#### Acceptance Criteria
1. The shiori4_host shall 読み終えたライブラリから `GhostRuntime` を作る公開の入口を持ち、今の `GhostRuntime::load` の形と振る舞いは変えない。
2. If その入口が `NotShiori4`（`shiori_get_api` が無い）か `Unsupported`（要求した版に対応しない）で失敗する, the shiori4_host shall エラーと一緒にライブラリをホストに返し、ホストが別の版や SHIORI/3.0 として読み直せるようにする。
3. If その入口が `LoadFailed`（v4 の load が NULL）で失敗する, the shiori4_host shall ライブラリを返さない。
4. The shiori4_host shall `LoadFailed` のときにライブラリを返さない理由（「v4 の SHIORI が初期化に失敗したので読み直さない。`shiori_get_api` を持つ DLL は v4 だと宣言しているので、旧来の口で取り繕うと二重の初期化か作者の意図と違う動きになる」）を doc に書く。

### Requirement 13: v3 互換モードの口
**Objective:** As a Windows 以外のベースウェアを組みたい人・SHIORI 実装者, I want `shiori4_host` で v3 互換モードの DLL を読めること, so that HGLOBAL に依存しない SHIORI/3.0 の口を、自分の CI や Windows 以外のホストで動かせる（論点 16・17）

#### Acceptance Criteria
1. The shiori4_host shall v3 互換モード（`shiori_get_api(3)`）で DLL を読み、SHIORI/3.0 の要求の全文を文字列で渡して、応答の全文を文字列で受け取る口を持つ。
2. The shiori4_host shall v3 互換モードの口に、post と完了通知を持たせない（load には host を渡さない）。
3. If v3 互換モードの load が NULL を返す, the shiori4_host shall `LoadError::LoadFailed` を返し、理由は付けない。
4. The shiori4_host shall 読み終えたライブラリの入口（要件 12）が `Unsupported` で返したライブラリを、そのまま v3 互換モードの口に渡せるようにする。
5. If SHIORI の応答が UTF-8 でない, the shiori4_host shall それをエラーとしてホストに知らせ、壊れた文字列を返さない。
6. The shiori4_host shall v3 互換モードの口でも、unload の後に未解放のものを残さず、ライブラリを最後に解放する（v4 の `GhostRuntime` と同じ後始末の順）。

### Requirement 14: SHIORI のファイル名の候補と、機械の種類の確認
**Objective:** As a ベースウェア実装者, I want descript.txt の名前から探すファイルの候補と、そのファイルの機械の種類を、DLL を読まずに得られること, so that helper のプロセスを起こす前に、どの arch のどのファイルを読むかを決められる（論点 12・17）

#### Acceptance Criteria
1. The C ヘッダ shall 規約の後に「配布の決まり」の節を持ち（議題 9）、 descript.txt の `shiori,<名前>` から名前の幹を取り、ホストの OS と arch ごとに次の順で探すことを述べる: Windows x64 は `<幹>.x64.dll` → `<幹>.dll`、Windows arm64 は `<幹>.arm64.dll` →（x64 のプロセスを起こせるホストだけ）`<幹>.x64.dll` → `<幹>.dll`、Linux x64 は `lib<幹>.x64.so` → `lib<幹>.so`、Linux arm64 は `lib<幹>.arm64.so` → `lib<幹>.so`、macOS は `lib<幹>.dylib`（ユニバーサルバイナリを推奨）。
2. The C ヘッダの「配布の決まり」 shall 接尾辞は `x64`・`arm64` の 2 つだけで `x86` は作らないこと、接尾辞なしの Windows の DLL が 32bit の SHIORI/3.0 を担うこと、Linux と macOS は `lib` の付いた名前だけを探すこと、macOS は arch の接尾辞を持たないことを述べる。
3. The shiori4_host shall 名前の幹（または `shiori,` の値）と、OS・arch・x64 のプロセスを起こせるかを受けて、候補のファイル名を上の順に並べて返す関数を持つ。この関数はファイルにもライブラリにも触れない。対象の OS・arch は引数で受ける形だけにし（areka の本体が別の arch の helper のために選ぶため）、今のプロセスの OS・arch は定数で得られるようにする。関数の doc は C ヘッダの「配布の決まり」を指す（議題 9・10）。
4. The shiori4_host shall ファイルの先頭のヘッダ（PE・ELF・Mach-O）から機械の種類を読み、指定した arch と合うかを確かめる関数を持つ。この関数はライブラリとして読み込まず、初期化コードを動かさない。
5. If ファイルが PE・ELF・Mach-O のどれでもないか、ヘッダが壊れている, the shiori4_host shall 合わないものとして結果を返し、パニックしない。
6. When macOS のユニバーサルバイナリを確かめる, the shiori4_host shall 中に指定した arch が含まれていれば合うものとして扱う。
7. The C ヘッダの「配布の決まり」 shall 「ホストは選んだファイルの機械の種類を確かめ、合わなければ次の候補へ進み、候補が尽きたらそのゴーストを読めなかったものとして扱う」と述べ、これが SHIORI/3.0 と v4 の両方に使えることを述べる。

### Requirement 15: 読み込みの持ち場・互換性・検証
**Objective:** As a 3 クレートの利用者, I want 依存と既存の API を壊さずに規約の更新が入ること, so that 0.0.2 に上げても今のコードがそのまま動き、振る舞いがテストで守られる（論点 17・Constraints）

#### Acceptance Criteria
1. The shiori4_abi shall 型と定数だけを持ち、ライブラリの読み込みをせず、外部の依存を持たない。
2. The shiori4_host shall ライブラリの読み込み・ファイル名の候補・ヘッダの確認を持ち、新しい外部の依存を足さない（ライブラリを開く所は既存の依存で行う）。
3. The shiori4_host shall v4 と v3 互換モードの口で、要求と応答を文字列で受け渡し、バイト列と UTF-8 の検査は口の内側で扱う。
4. The 3 クレート shall 版を上げず、同じ版のまま揃える。
5. The 3 クレート shall この spec の振る舞い（-7 の検出・Poster の無効化・log の任意スレッド・dir の正規化・Charset を付けないこと・発見の順と許可・`from_library` の入口・v3 互換モードの口・ファイル名の候補・機械の種類の確認）を、各クレートのテストで確かめる。
6. The 3 クレート shall ubuntu・windows・macos の CI（test と clippy の `-D warnings`）を通す。

## 要件ディスカッションの決定（2026-10-23）
- **自明な修正**: Poster の置き場は `shiori4_abi`（要件 4 の主語）。呼び出しの外の `host->request` は -7（要件 3.3）。`SHIORI_E_INVALID_UTF8` の再エクスポート漏れ（要件 2.5）。steering の `tech.md` も直す対象（要件 1.3）。
- **議題 1**: `ShioriHostPoster` の `Copy` を外し、`Clone` を残す（要件 4.7）。
- **議題 2**: unload は、門を閉じた時点で実行中の post 1 回分が返るのを待ってよい。join とは別（要件 4.5）。
- **議題 3**: パニックの規約を、言語によらない形で C ヘッダに入れる（要件 1.4）。
- **議題 4**: `Ghost::load` の失敗（パニックを含む）を高水準層が自動で 1 行 log する（要件 7.5）。
- **議題 5**: `ShioriHostPoster` から log も呼べる（要件 5.6）。
- **議題 6**: `ShioriOwnedBytes`／`ShioriOwnedResponse` を `Send` にする（要件 5.7）。
- **議題 7**: dir の正規化の細部（相対パスの扱い）— 議論中。
- **議題 8**: 従来の口で解決したら、口の種類と開いたライブラリを返すだけ（要件 11.5）。
- **議題 9**: 配布の決まりは C ヘッダの規約の後の節に書き、`shiori4_host` の doc から指す（要件 1.1・14）。
- **議題 10**: 候補の関数は対象の OS・arch を引数で受ける形だけ。今のプロセスの値は定数で得る（要件 14.3）。
