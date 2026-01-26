# nixy - シンプルな宣言的 Nix パッケージ管理

**再現性のある Nix パッケージを、シンプルなコマンドで。** コマンド一つでインストール、すべてのマシンで同期。

```bash
nixy install ripgrep    # これだけ。シンプルな Nix 生活。
```

nixy は宣言的な `flake.nix` で Nix パッケージを管理します。再現性の仕組みがない `nix profile` とは異なり、nixy はどのマシンでも同じパッケージ、同じバージョンを保証します。薄いラッパースクリプトなので、ロックインも複雑な仕組みもありません。

## なぜ nixy？

**Homebrew、asdf などに不満を感じているユーザー**のために：
- マシン間で再現可能な環境（「俺の環境では動く」から卒業）
- システムを壊さないアトミックなアップグレード
- 全パッケージを一つのロックファイルで管理（バージョンのずれを防止）

**nixy は Nix の上に構築されたパッケージ管理レイヤーです。** Nix の全機能（dev shells、ビルドシステム、NixOS）を置き換えるものではなく、Homebrew のようにグローバルにインストールされるパッケージの管理に特化しています。

### nixy が提供するもの：
- **シンプルなコマンド**: `nixy install`、`nixy uninstall`、`nixy upgrade`
- **真の再現性**: `flake.nix` + `flake.lock` = どこでも同一の環境
- **複数プロファイル**: 仕事用、個人用、プロジェクト用に分離したパッケージセット
- **ロックインなし**: 中身は普通の Nix - いつでも離脱可能
- **クロスプラットフォーム**: macOS と Linux で同じワークフロー

### nixy が提供しないもの：
- Home Manager や NixOS の代替
- 開発環境ツール（それには `nix develop` を使おう）
- ビルドシステム

CLI ツールに対して、Homebrew のシンプルさと Nix の再現性を両立させたいなら、nixy がぴったりです。

## 仕組み

nixy はシンプルな Nix の機能だけを使います - Home Manager も NixOS も不要。パッケージは `~/.config/nixy/profiles/<name>/` の `flake.nix` で定義され、`nix build` でビルドされます。

nixy は**純粋に宣言的** - `flake.nix` が唯一の真実の源です。可変な状態を持つ `nix profile` とは異なり、nixy は `nix build --out-link` を使ってビルド済み環境へのシンボリックリンク（`~/.local/state/nixy/env`）を作成します。これにより：
- 同期が狂う隠れたプロファイル状態がない
- `flake.nix` にあるものが、そのままインストールされているもの
- 理解しやすく、デバッグしやすく、バージョン管理しやすい

nixy は flake.nix を編集して標準の `nix` コマンドを実行するだけ。生成される flake.nix は普通の Nix なので、直接読んだり編集したり、`nix` コマンドを直接使うこともできます。

## nixy と nix profile

nixy は `nix profile` の代替ではなく、再現性を追加する補助ツールです。

`nix profile` は単一マシンでの手軽なパッケージ管理に最適です。nixy は Nix の上に宣言的なレイヤーを追加し、以下が必要な場合に役立ちます：

- **統一されたロックファイル**: 全パッケージを同じ nixpkgs バージョンに固定
- **簡単な同期**: `flake.nix` を新しいマシンにコピーして `nixy sync` を実行、それだけ
- **バージョン管理可能な設定**: `flake.nix` は git での管理に最適

nixy と `nix profile` は別々のパス（`~/.local/state/nixy/env` と `~/.nix-profile`）を使うため、互いに干渉しません。`nix profile` は手軽な実験用に、nixy は再現可能なベース環境用に - あるいは両方を組み合わせて使えます。

## クイックスタート

nixy は**プロファイル**を使ってパッケージを整理します。初回使用時に「default」プロファイルが自動作成されます。後から仕事用、個人用、プロジェクト用など、追加のプロファイルを作成できます。

### 1. Nix をインストール（まだの場合）

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

### 2. nixy をインストール

**クイックインストール（推奨）:**

```bash
curl -fsSL https://raw.githubusercontent.com/yusukeshib/nixy/main/install.sh | bash
```

**cargo でインストール（crates.io から）:**

```bash
cargo install nixy
```

**nix でインストール:**

```bash
nix profile install github:yusukeshib/nixy
```

### 3. シェルを設定

シェル設定ファイル（`.bashrc`、`.zshrc` など）に追加：

```bash
eval "$(nixy config zsh)"
```

fish の場合は `~/.config/fish/config.fish` に追加：

```fish
nixy config fish | source
```

### 4. パッケージをインストール

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

### パッケージ管理

| コマンド | エイリアス | 説明 |
|---------|-----------|------|
| `nixy install <pkg>` | `add` | nixpkgs からパッケージをインストール |
| `nixy install --from <flake> <pkg>` | | flake からインストール（レジストリ名または URL） |
| `nixy install --file <path>` | | カスタム nix ファイルからインストール |
| `nixy uninstall <pkg>` | `remove` | パッケージをアンインストール |
| `nixy list` | `ls` | インストール済みパッケージを一覧表示 |
| `nixy search <query>` | | パッケージを検索 |
| `nixy upgrade [input...]` | | 全 input または指定した input をアップグレード |
| `nixy sync` | | flake.nix から環境をビルド（新しいマシン用） |
| `nixy gc` | | 古いパッケージを削除 |

### プロファイル管理

| コマンド | エイリアス | 説明 |
|---------|-----------|------|
| `nixy profile` | | 現在のプロファイルを表示 |
| `nixy profile switch <name>` | `use` | プロファイルを切り替え |
| `nixy profile switch -c <name>` | | 新しいプロファイルを作成して切り替え |
| `nixy profile list` | `ls` | 全プロファイルを一覧表示 |
| `nixy profile delete <name>` | `rm` | プロファイルを削除（`--force` 必須） |

### ユーティリティ

| コマンド | 説明 |
|---------|------|
| `nixy config <shell>` | シェル設定を出力（PATH 設定用） |
| `nixy version` | nixy のバージョンを表示 |
| `nixy self-upgrade` | nixy を最新版にアップグレード |
| `nixy self-upgrade --force` | 最新版でも強制的に再インストール |

### install オプション

`install` コマンドはいくつかのオプションをサポートしています：

```bash
nixy install ripgrep              # nixpkgs からインストール（デフォルト）
nixy install --from <flake> <pkg> # 外部 flake からインストール
nixy install --file my-pkg.nix    # カスタム nix ファイルからインストール
nixy install --force <pkg>        # flake.nix を強制的に再生成
```

`--force` は、nixy マーカー外を手動で編集した場合に使用します（カスタム変更は失われます）。

## 複数プロファイル

異なる用途（仕事、個人、プロジェクト）ごとに別々のパッケージセットを管理できます：

```bash
nixy profile switch -c work   # 新しいプロファイルを作成して切り替え
nixy install slack terraform  # 仕事用パッケージをインストール

nixy profile switch -c personal  # 別のプロファイルを作成して切り替え
nixy install spotify games    # ここには別のパッケージ

nixy profile list             # 全プロファイルを表示
nixy profile                  # 現在のプロファイルを表示
```

各プロファイルは `~/.config/nixy/profiles/<name>/` に独自の `flake.nix` を持ちます。プロファイルを切り替えると、環境のシンボリックリンクがそのプロファイルのパッケージを指すように再構築されます。

**ユースケース：**
- **仕事 vs 個人**: 仕事用ツールと個人用アプリを分離
- **クライアントプロジェクト**: クライアントごとに異なるツールチェーン
- **実験**: メインのセットアップに影響を与えずに新しいパッケージを試す

**dotfiles でプロファイルを管理：**

```bash
# 全プロファイルを dotfiles にバックアップ
cp -r ~/.config/nixy/profiles ~/dotfiles/nixy-profiles

# 新しいマシンで復元して同期
cp -r ~/dotfiles/nixy-profiles ~/.config/nixy/profiles
nixy profile switch work      # 目的のプロファイルに切り替え
nixy sync                     # 環境をビルド
```

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
`nixy self-upgrade` で自動的に最新版にアップデートできます。または `cargo install nixy` やインストールスクリプトの再実行でも可能です。

**nixy をアンインストールするには？**
`nixy` スクリプトを削除するだけ。flake.nix ファイルはそのまま残り、標準の `nix` コマンドで使えます。

**なぜ `nix profile` を直接使わないの？**
`nix profile` には再現性の仕組みがありません - パッケージをエクスポートして別のマシンで同じ環境を再現する公式の方法がないのです。nixy は `flake.nix` を真実の源として使うため、コピー、バージョン管理、共有が可能です。

**以前の状態にロールバックするには？**
nixy は宣言的なので、`flake.nix` と `flake.lock` が状態そのものです。git で管理していれば（推奨）、ロールバックは簡単：

```bash
git checkout HEAD~1 -- flake.nix flake.lock  # 前のコミットに戻す
nixy sync                                     # 古い状態を適用
```

これは `nix profile rollback` より強力です - 履歴の任意の時点に戻れる、コミットメッセージで変更理由がわかる、ブランチで実験できる、といった利点があります。

**非フリーパッケージをインストールするには？**
非フリーライセンスのパッケージ（例：`graphite-cli`、`slack`）は nixy でデフォルトで許可されています。通常通りインストールできます：

```bash
nixy install slack
```

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

### 外部 flake からのインストール

`--from` を使って任意の flake からパッケージをインストールできます：

```bash
# 直接 flake URL を指定
nixy install --from github:nix-community/neovim-nightly-overlay neovim

# または nix レジストリ名を使用
nixy install --from nixpkgs hello
```

flake はカスタム input として `flake.nix` に追加され、再現性のために完全な URL が保存されます。パッケージをエクスポートする任意の flake で動作します。

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
