# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.9] - 2026-07-07

### Added
- スニペット検索ウィンドウにおいて、検索入力が空の状態で左右矢印キー（`Left` / `Right`）を押すことでフォルダ階層を移動する機能を追加。
- タスクトレイのコンテキストメニューに「履歴をクリア」項目を追加。実行時に確認メッセージボックスを表示し、承認時にメモリおよび暗号化保存ファイルから全履歴を消去する機能を追加。

### Fixed
- ダブルタップ（ホットキー連打）によるウィンドウ起動時の誤検知防止対策を大幅に強化。
  - ホットキーの連打許容間隔（`double_tap_ms`）のデフォルト値を 500ms から 300ms に短縮。
  - キーの押し下げ時間（ホールド時間）を測定し、150ms 以上の押し下げ（長押し）はダブルタップ判定から除外。
  - キー押下中に他のキー（文字入力やショートカットキーなど）が介在した場合、ダブルタップ判定を即座にリセットする仕組みを導入。

## [0.1.8] - 2026-07-06

### Changed
- 定型文呼び出しおよび履歴呼び出しのデフォルトホットキーを、それぞれ左の `Shift` キーおよび `Ctrl` キーの連打に変更。
- ホットキーによるウィンドウ起動時、クリップボードへの `Ctrl+C` シミュレーション（自動コピー）の実行処理を廃止（安定性向上）。

[0.1.9]: https://github.com/cwatanab/Clipper/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/cwatanab/Clipper/releases/tag/v0.1.8
