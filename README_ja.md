# nixy - シンプルな宣言的 Nix パッケージ管理

**再現性のある Nix パッケージを、シンプルなコマンドで。** コマンド一つでインストール、すべてのマシンで同期。

```bash
nixy install ripgrep    # これだけ。シンプルな Nix 生活。
```

nixy は宣言的な `flake.nix` で Nix パッケージを管理します。再現性の仕組みがない `nix profile` とは異なり、nixy はどのマシンでも同じパッケージ、同じバージョンを保証します。薄いラッパースクリプトなので、ロックインも複雑な仕組みもありません。

## なぜ nixy？

Nix は強力ですが、パッケージ管理は複雑であるべきではありません。flake ファイルを手書きすることなく、Nix のメリットを享受したい。

nixy が提供するもの：
- **シンプルなコマンド**: `nixy install`、`nixy uninstall`、`nixy upgrade` - これだけ
- **再現性**: どのマシンでも同じパッケージ、同じバージョン
- **隠れた状態なし**: `flake.nix` が唯一の真実の源
- **アトミックな更新とロールバック**: 更新は完全に成功するか、何も変わらないか
- **クロスプラットフォーム**: macOS と Linux で同じワークフロー
- **複数プロファイル**: 仕事用、個人用、プロジェクト用に分離したパッケージセット

`nix profile` と異なり、nixy は `flake.nix` + `flake.lock` で完全な再現性を実現。設定を新しいマシンにコピーして `nixy sync` を実行、それで完了。

## 仕組み

nixy はシンプルな Nix の機能だけを使います - Home Manager も NixOS も不要。パッケージは `~/.config/nixy/profiles/<name>/` の `flake.nix` で定義され、`nix build` でビルドされます。

nixy は**純粋に宣言的** - `flake.nix` が唯一の真実の源です。可変な状態を持つ `nix profile` とは異なり、nixy は `nix build --out-link` を使ってビルド済み環境へのシンボリックリンク（`~/.local/state/nixy/env`）を作成します。これにより：
- 同期が狂う隠れたプロファイル状態がない
- `flake.nix` にあるものが、そのままインストールされているもの
- 理解しやすく、デバッグしやすく、バージョン管理しやすい

nixy は flake.nix を編集して標準の `nix` コマンドを実行するだけ。生成される flake.nix は普通の Nix なので、直接読んだり編集したり、`nix` コマンドを直接使うこともできます。

## なぜ nix profile じゃないの？

`nix profile` は命令的パッケージ管理のための標準 Nix ツールです。単一マシンでの使用には問題ありませんが、再現性の面で不十分です：

| | nix profile | nixy |
|---|-------------|------|
| パッケージリスト | `manifest.json` に隠れている | 読みやすい `flake.nix` |
| バージョン固定 | パッケージ個別のみ、統一ロックなし | 単一の `flake.lock` で全パッケージ管理 |
| 新マシンへの同期 | 手動で再インストール | `nixy sync` |
| ロールバック | プロファイル世代のみ | Git + `flake.lock` |

1台のマシンだけで使い、再現性が不要なら `nix profile` の方がシンプル。どこでも同じ環境が欲しいなら nixy を使おう。

## クイックスタート

### 1. Nix をインストール（まだの場合）

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

### 2. nixy をインストール

```bash
curl -fsSL https://raw.githubusercontent.com/yusukeshib/nixy/main/install.sh | bash
```

### 3. パッケージをインストール

```bash
nixy install ripgrep    # 初回実行時にデフォルトプロファイルを自動作成
nixy install nodejs
nixy install git

nixy list               # インストール済みパッケージを表示
nixy search python      # パッケージを検索
nixy uninstall nodejs   # パッケージを削除
nixy upgrade            # 全パッケージをアップグレード
```

パッケージはグローバルにインストールされ、すべてのターミナルセッションで利用可能になります。

## コマンド

| コマンド | 説明 |
|---------|------|
| `nixy install <pkg>` | パッケージをグローバルにインストール |
| `nixy uninstall <pkg>` | パッケージをアンインストール |
| `nixy list` | インストール済みパッケージを一覧表示 |
| `nixy search <query>` | パッケージを検索 |
| `nixy upgrade` | 全パッケージをアップグレード |
| `nixy sync` | flake.nix から環境をビルド（新しいマシン用） |
| `nixy gc` | 古いパッケージを削除 |
| `nixy config <shell>` | シェル設定を出力（PATH 設定用） |
| `nixy version` | nixy のバージョンを表示 |
| `nixy self-upgrade` | nixy を最新版にアップグレード |

## 複数マシンで同期

パッケージリストはただのテキストファイル。バックアップしたり、バージョン管理したり、dotfiles と一緒に同期できます：

```bash
# パッケージリストをバックアップ（デフォルトプロファイル）
cp ~/.config/nixy/profiles/default/flake.nix ~/dotfiles/

# 新しいマシンで：
mkdir -p ~/.config/nixy/profiles/default
cp ~/dotfiles/flake.nix ~/.config/nixy/profiles/default/
nixy sync    # flake.nix からすべてをインストール
```

どのマシンでも同じパッケージ、同じバージョン。

---

## FAQ

**パッケージ名がわからない**
`nixy search <キーワード>` を使ってください。パッケージ名は予想と異なることがあります（例：`rg` ではなく `ripgrep`）。

**パッケージは実際にどこにインストールされる？**
Nix ストア（`/nix/store/`）にインストールされます。nixy は統合された環境をビルドし、`~/.local/state/nixy/env` にシンボリックリンクを作成します。`nixy config` コマンドでこの場所を PATH に追加する設定を行います。

**flake.nix を手動で編集できる？**
はい！nixy はカスタムマーカーを提供しており、再生成時に保持される独自の inputs、packages、paths を追加できます：

```nix
# [nixy:custom-inputs]
my-overlay.url = "github:user/my-overlay";
# [/nixy:custom-inputs]
```

これらのマーカー外のコンテンツは、nixy が flake を再生成する際に上書きされます。詳細なカスタマイズについては、付録の「flake.nix のカスタマイズ」を参照してください。

**nixy をアップデートするには？**
`nixy self-upgrade` を実行します。更新を確認し、最新版をダウンロードして自動的に置き換えます。すでに最新版でも再インストールしたい場合は `--force` オプションを使用してください。

**nixy をアンインストールするには？**
`nixy` スクリプトを削除するだけ。flake.nix ファイルはそのまま残り、標準の `nix` コマンドで使えます。

**なぜ `nix profile` を直接使わないの？**
`nix profile` には再現性の仕組みがありません - パッケージをエクスポートして別のマシンで同じ環境を再現する公式の方法がないのです。nixy は `flake.nix` を真実の源として使うため、コピー、バージョン管理、共有が可能です。

---

## 付録

### flake.nix のカスタマイズ

nixy はカスタムマーカーを提供しており、flake 再生成時に保持される独自のコンテンツを追加できます：

**カスタム inputs** - 独自の flake inputs を追加：
```nix
# [nixy:custom-inputs]
my-overlay.url = "github:user/my-overlay";
home-manager.url = "github:nix-community/home-manager";
# [/nixy:custom-inputs]
```

**カスタム packages** - カスタムパッケージ定義を追加：
```nix
# [nixy:custom-packages]
my-tool = pkgs.writeShellScriptBin "my-tool" ''echo "Hello"'';
patched-app = pkgs.app.overrideAttrs { ... };
# [/nixy:custom-packages]
```

**カスタム paths** - buildEnv に追加のパスを指定：
```nix
# [nixy:custom-paths]
my-tool
patched-app
# [/nixy:custom-paths]
```

これらのマーカー**外**のコンテンツを編集すると、上書き前に nixy が警告します：
```
Warning: flake.nix has modifications outside nixy markers.
Use --force to proceed (custom changes will be lost).
```

### 既存の Nix ユーザー向け

すでに独自の `flake.nix` を管理していて、nixy のパッケージリストを使いたい場合は、インポートできます：

```nix
{
  inputs.nixy.url = "path:~/.config/nixy";

  outputs = { self, nixpkgs, nixy }: {
    # nixy.packages.<system>.default は全 nixy パッケージを含む buildEnv
    # 依存関係として使ったり、独自の環境とマージできます
  };
}
```

こうすることで、nixy がパッケージリストを管理しつつ、flake の完全なコントロールを維持できます。

### nix profile との共存

nixy と `nix profile` は別のパスを使用するため、競合しません：
- nixy: `~/.local/state/nixy/env/bin`
- nix profile: `~/.nix-profile/bin`

両方が PATH にある場合、先にリストされた方が、両方にインストールされたパッケージで優先されます。異なる目的で両方のツールを使用できます。

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
| `~/.config/nixy/profiles/<name>/flake.nix` | プロファイルのパッケージ |
| `~/.config/nixy/active` | 現在のアクティブプロファイル名 |
| `~/.config/nixy/profiles/<name>/packages/` | プロファイルのカスタムパッケージ定義 |
| `~/.local/state/nixy/env` | ビルド済み環境へのシンボリックリンク（`bin/` を PATH に追加） |
| `~/.config/nixy/flake.nix` | レガシーの場所（デフォルトプロファイルに自動移行） |

### 環境変数

| 変数 | デフォルト | 説明 |
|------|-----------|------|
| `NIXY_CONFIG_DIR` | `~/.config/nixy` | グローバル flake.nix の場所 |
| `NIXY_ENV` | `~/.local/state/nixy/env` | ビルド済み環境へのシンボリックリンク |

### 制限事項

- パッケージ名は Nix の命名規則に従います（`nixy search` で正確な名前を確認）
- GUI アプリのサポートはまだありません（Homebrew Cask のような機能）
- flakes が有効な Nix が必要（Determinate インストーラーはデフォルトで有効化）

## ライセンス

MIT
