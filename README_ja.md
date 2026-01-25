# nixy - Nix を Homebrew のように使う

**今日から Nix を使い始めて、少しずつ学んでいこう。** Homebrew と同じ感覚で、コマンド一つでパッケージをインストール。

```bash
nixy install ripgrep    # これだけ。シンプルな Nix 生活。
```

nixy は Nix の強力な機能（再現可能なビルド、ロールバック、依存関係の衝突なし）を Homebrew のシンプルさで提供します。薄いラッパースクリプトなので、ロックインも複雑な仕組みもありません。

## なぜ nixy？

Nix は強力ですが、学習曲線が急です。Nix を学びたいけど、パッケージを一つインストールするために flake.nix を書くのは最初はハードルが高い。

nixy はその移行をスムーズにします。シンプルなコマンドから始めて、生成された `flake.nix` を読むことで Nix のパターンを自然に学べます。Nix のメリットをすべて享受できます：
- **再現性**: どのマシンでも同じパッケージ、同じバージョン
- **衝突なし**: プロジェクトごとに異なるバージョンのツールを使用可能
- **アトミックな更新**: 更新は完全に成功するか、何も変わらないか
- **ロールバック**: アップグレードで問題が起きても即座に戻せる
- **クロスプラットフォーム**: macOS と Linux で同じワークフロー

`nixy install <package>` から始めて、準備ができたら生成された flake.nix を読んでみよう。

## 仕組み

nixy はシンプルな Nix の機能だけを使います - Home Manager も NixOS も不要：

- **グローバルパッケージ**（デフォルト）: `~/.config/nix/` に `flake.nix` + `nix profile`
- **プロジェクトパッケージ**（`--local`）: プロジェクトディレクトリに `flake.nix` のみ

nixy は flake.nix を編集して標準の `nix` コマンドを実行するだけ。生成される flake.nix は普通の Nix なので、直接読んだり編集したり、`nix` コマンドを直接使うこともできます。

## Homebrew vs nixy

| Homebrew | nixy |
|----------|------|
| `brew install ripgrep` | `nixy install ripgrep` |
| `brew uninstall ripgrep` | `nixy uninstall ripgrep` |
| `brew list` | `nixy list` |
| `brew search git` | `nixy search git` |
| `brew upgrade` | `nixy upgrade` |

同じシンプルさで、裏側は Nix の信頼性。ロックインなし - 標準の Nix そのものです。

## クイックスタート

### 1. Nix をインストール（まだの場合）

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

### 2. nixy をインストール

```bash
curl -fsSL https://raw.githubusercontent.com/yusukeshib/nixy/main/install.sh | bash
```

### 3. Homebrew のように使い始める

```bash
nixy install ripgrep    # 初回実行時に ~/.config/nix/flake.nix を自動作成
nixy install nodejs
nixy install git

nixy list               # インストール済みパッケージを表示
nixy search python      # パッケージを検索
nixy uninstall nodejs   # パッケージを削除
nixy upgrade            # 全パッケージをアップグレード
```

Homebrew と同じように、パッケージはグローバルにインストールされ、すべてのターミナルセッションで利用可能になります。

## コマンド

| コマンド | 説明 |
|---------|------|
| `nixy install <pkg>` | パッケージをグローバルにインストール |
| `nixy uninstall <pkg>` | パッケージをアンインストール |
| `nixy list` | インストール済みパッケージを一覧表示 |
| `nixy search <query>` | パッケージを検索 |
| `nixy upgrade` | 全パッケージをアップグレード |
| `nixy sync` | flake.nix からプロファイルを同期（新しいマシン用） |
| `nixy gc` | 古いパッケージを削除 |
| `nixy version` | nixy のバージョンを表示 |
| `nixy self-upgrade` | nixy を最新版にアップグレード |

## 複数マシンで同期

パッケージリストはただのテキストファイル（`~/.config/nix/flake.nix`）。バックアップしたり、バージョン管理したり、dotfiles と一緒に同期できます：

```bash
# パッケージリストをバックアップ
cp ~/.config/nix/flake.nix ~/dotfiles/

# 新しいマシンで：
mkdir -p ~/.config/nix
cp ~/dotfiles/flake.nix ~/.config/nix/
nixy sync    # flake.nix からすべてをインストール
```

どのマシンでも同じパッケージ、同じバージョン。

---

## 上級編：プロジェクト別パッケージ

プロジェクト固有の依存関係が必要な開発者向け（`package.json` のようなもの、でもあらゆるツールに対応）：

```bash
cd my-project
nixy init                     # このディレクトリに flake.nix を作成
nixy install --local nodejs   # ローカル flake.nix にパッケージを追加
nixy install --local postgres

nixy shell                    # これらのパッケージが使えるシェルに入る
```

`--local`（または `-l`）を付けると、パッケージはプロジェクトの `flake.nix` に追加されますが、グローバルプロファイルにはインストールされません。`nixy shell` で全プロジェクトパッケージが利用可能な開発シェルに入れます。これにより、プロジェクトの依存関係をグローバル環境から分離できます。

`--local` を使用すると、nixy は親ディレクトリを遡って最も近い `flake.nix` を自動的に探して使用します（git が `.git` を探すのと同様）。

### プロジェクト環境の共有

```bash
# flake.nix をリポジトリにコミット
git add flake.nix flake.lock

# チームメイトも同じ環境を取得：
git clone my-project && cd my-project
nixy shell             # 全プロジェクトパッケージ入りの開発シェルに入る
```

### プロジェクト用の追加コマンド

| コマンド | 説明 |
|---------|------|
| `nixy init` | カレントディレクトリに flake.nix を作成 |
| `nixy install --local <pkg>` | ローカル flake.nix にパッケージを追加 |
| `nixy shell` | プロジェクトパッケージ入りの開発シェルに入る |

`--local`（または `-l`）を install/uninstall/list/upgrade に付けると、グローバルではなくプロジェクトの flake を操作します。

---

## FAQ

**Homebrew と nixy は一緒に使える？**
はい。競合しません。段階的に移行することも、両方使い続けることもできます。

**パッケージ名がわからない**
`nixy search <キーワード>` を使ってください。パッケージ名は予想と異なることがあります（例：`rg` ではなく `ripgrep`）。

**パッケージは実際にどこにインストールされる？**
Nix ストア（`/nix/store/`）にインストールされます。nixy はどのパッケージを PATH で使えるようにするかを管理するだけです。

**flake.nix を手動で編集できる？**
はい、ただし注意が必要です。nixy はパッケージのインストール/アンインストール時に flake.nix 全体を再生成し、`# [nixy:...]` マーカー内の内容のみ保持します。高度なカスタマイズが必要な場合は、flake.nix を手動で管理し `nix` コマンドを直接使用することを検討してください。

**nixy をアップデートするには？**
`nixy self-upgrade` を実行します。更新を確認し、最新版をダウンロードして自動的に置き換えます。すでに最新版でも再インストールしたい場合は `--force` オプションを使用してください。

**nixy をアンインストールするには？**
`nixy` スクリプトを削除するだけ。flake.nix ファイルはそのまま残り、標準の `nix` コマンドで使えます。

---

## 付録

### カスタムパッケージ定義

カスタム nix ファイルからパッケージをインストール：

```bash
nixy install --file my-package.nix
```

`my-package.nix` の形式：
```nix
{
  name = "my-package";
  inputs = { overlay-name.url = "github:user/repo"; };
  overlay = "overlay-name.overlays.default";
  packageExpr = "pkgs.my-package";
}
```

### 設定ファイルの場所

| パス | 説明 |
|------|------|
| `~/.config/nix/flake.nix` | グローバルパッケージ（デフォルト） |
| `./flake.nix` | プロジェクトローカルパッケージ（`--local` で使用） |
| `~/.config/nix/packages/` | カスタムパッケージ定義 |

### 環境変数

| 変数 | デフォルト |
|------|-----------|
| `NIXY_CONFIG_DIR` | `~/.config/nix` |

### 制限事項

- パッケージ名は Nix の命名規則に従います（`nixy search` で正確な名前を確認）
- GUI アプリのサポートはまだありません（Homebrew Cask のような機能）
- flakes が有効な Nix が必要（Determinate インストーラーはデフォルトで有効化）

## ライセンス

MIT
