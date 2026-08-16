# Clipper ビルド手順書 (BUILD.md)

本ドキュメントでは、**Clipper** の公式ターゲットである **MSVC (`x86_64-pc-windows-msvc`)** でのビルド手順およびクロスコンパイル環境のセットアップ方法について解説します。

---

## 📋 目次

1. [ビルドターゲット](#-ビルドターゲット)
2. [前提条件](#-前提条件)
3. [Windows ネイティブ環境でのビルド (MSVC)](#-windows-ネイティブ環境でのビルド-msvc)
4. [Linux / WSL 環境からのクロスコンパイル (MSVC)](#-linux--wsl-環境からのクロスコンパイル-msvc)
5. [アイコン・リソースの埋め込み仕様](#-アイコンリソースの埋め込み仕様)
6. [生成物の出力先](#-生成物の出力先)

---

## 🎯 ビルドターゲット

- **ターゲットトリプル**: `x86_64-pc-windows-msvc`
- **アーキテクチャ**: 64-bit Windows (x86_64)
- **ツールチェーン**: MSVC (Microsoft Visual C++ ABI)

---

## 🛠️ 前提条件

- **Rust**: 1.85.0 以上（Rust 2024 Edition 対応）
- **Git**

---

## 🪟 Windows ネイティブ環境でのビルド (MSVC)

Visual Studio または Visual Studio Build Tools（「C++ によるデスクトップ開発」ワークロード）がインストールされている Windows 環境でのビルド手順です。

### 1. リポジトリのクローン
```bash
git clone https://github.com/cwatanab/Clipper.git
cd Clipper
```

### 2. ビルドコマンド

- **デバッグビルド**:
  ```bash
  cargo build --target x86_64-pc-windows-msvc
  ```

- **最適化リリースビルド**:
  ```bash
  cargo build --release --target x86_64-pc-windows-msvc
  ```

---

## 🐧 Linux / WSL 環境からのクロスコンパイル (MSVC)

Linux / WSL 環境から Windows 向け MSVC バイナリ（`x86_64-pc-windows-msvc`）をクロスコンパイルする場合の手順です。`cargo-xwin` を使用してビルドします。

### 1. 必要なツールのインストール

- **Rust MSVC ターゲットの追加**:
  ```bash
  rustup target add x86_64-pc-windows-msvc
  ```

- **cargo-xwin**（Windows SDK / CRT ヘッダー・ライブラリ自動取得・リンクツール）:
  ```bash
  cargo install cargo-xwin
  ```

- **LLVM / Clang**（リソースコンパイラ `llvm-rc` およびプリプロセッサ用）:
  - Ubuntu / Debian:
    ```bash
    sudo apt update
    sudo apt install -y clang lld llvm
    ```

### 2. ビルドコマンド

- **デバッグビルド**:
  ```bash
  cargo xwin build --target x86_64-pc-windows-msvc
  ```

- **最適化リリースビルド**:
  ```bash
  cargo xwin build --release --target x86_64-pc-windows-msvc
  ```

※ `build.rs` がシステム内の `llvm-rc`（または `llvm-rc-19` 等のバージョン付き実行ファイル）を自動検出し、アイコンリソース（`clipper.rc`）を自動的にコンパイル・埋め込みます。

---

## 🎨 アイコン・リソースの埋め込み仕様

Clipper は Windows の実行ファイルアイコンおよびタスクトレイ通知アイコンとして以下のリソースを使用します：

- `clipper.rc`: リソース定義ファイル
- `assets/app.ico`: アプリアイコン（ライトモード用 / ID 1）
- `assets/app_inverted.ico`: アプリアイコン（ダークモード用 / ID 2）

ビルドスクリプト（`build.rs`）が `embed-resource` を介してこれらを自動的にコンパイルし、バイナリへリンクします。

---

## 📦 生成物の出力先

ビルドが完了すると、以下のパスにバイナリが出力されます。

- **MSVC リリースバイナリ**:
  `target/x86_64-pc-windows-msvc/release/clipper.exe`
- **MSVC デバッグシンボル (PDB)**:
  `target/x86_64-pc-windows-msvc/release/clipper.pdb`
