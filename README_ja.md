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

nixy は**純粋に宣言的** - `packages.json` が唯一の真実の源であり、`flake.nix` は操作のたびにそこから完全に再生成されます。可変な状態を持つ `nix profile` とは異なり、nixy は `nix build --out-link` を使ってビルド済み環境へのシンボリックリンク（`~/.local/state/nixy/env`）を作成します。これにより：
- 同期が狂う隠れたプロファイル状態がない
- `packages.json` にあるものが、そのままインストールされているもの
- 理解しやすく、デバッグしやすく、バージョン管理しやすい

生成される `flake.nix` は普通の Nix なので、読んだり、検査したり、`nix` コマンドを直接使うこともできます。ただし、`flake.nix` への手動編集は上書きされます。

## nixy と nix profile

nixy は `nix profile` の代替ではなく、再現性を追加する補助ツールです。

`nix profile` は単一マシンでの手軽なパッケージ管理に最適です。nixy は Nix の上に宣言的なレイヤーを追加し、以下が必要な場合に役立ちます：

- **統一されたロックファイル**: 全パッケージを同じ nixpkgs バージョンに固定
- **簡単な同期**: `packages.json` を新しいマシンにコピーして `nixy sync` を実行、それだけ
- **バージョン管理可能な設定**: `packages.json` + `flake.lock` は git での管理に最適

nixy と `nix profile` は別々のパス（`~/.local/state/nixy/env` と `~/.nix-profile`）を使うため、互いに干渉しません。`nix profile` は手軽な実験用に、nixy は再現可能なベース環境用に - あるいは両方を組み合わせて使えます。

## 他のツールとの比較

### vs devbox

[devbox](https://github.com/jetify-com/devbox) は**開発環境ツール**です - asdf、nvm、pyenv の代替と考えてください。プロジェクトごとの依存関係と分離されたシェルを管理します。

nixy は**パッケージマネージャー**です - Homebrew の代替と考えてください。グローバルにインストールする CLI ツールを管理します。

用途が違うツールです。

### vs home-manager

[home-manager](https://github.com/nix-community/home-manager) はホームディレクトリ全体を管理します - dotfiles、サービス、パッケージ。強力ですが、Nix を学ぶ必要があります。

nixy はパッケージだけを管理します。完全なホーム設定が必要なら home-manager を使ってください。Nix の再現性を持つ Homebrew スタイルのパッケージ管理だけが欲しいなら、nixy を使ってください。

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
| `nixy list` | `ls` | インストール済みパッケージをソース情報付きで表示 |
| `nixy search <query>` | | パッケージを検索 |
| `nixy upgrade [input...]` | | 全 input または指定した input をアップグレード |
| `nixy sync` | | flake.nix から環境をビルド（新しいマシン用） |

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
```

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

パッケージの状態は `packages.json` に保存されています。バックアップしたり、バージョン管理したり、dotfiles と一緒に同期できます：

```bash
# パッケージ状態をバックアップ（デフォルトプロファイル）
cp ~/.config/nixy/profiles/default/packages.json ~/dotfiles/
cp ~/.config/nixy/profiles/default/flake.lock ~/dotfiles/  # 正確なバージョン用
cp -r ~/.config/nixy/profiles/default/packages ~/dotfiles/ # カスタムパッケージがある場合

# 新しいマシンで：
mkdir -p ~/.config/nixy/profiles/default
cp ~/dotfiles/packages.json ~/.config/nixy/profiles/default/
cp ~/dotfiles/flake.lock ~/.config/nixy/profiles/default/   # オプション
cp -r ~/dotfiles/packages ~/.config/nixy/profiles/default/  # 必要に応じて
nixy sync    # flake.nix を再生成してすべてをインストール
```

どのマシンでも同じパッケージ、同じバージョン。

---

## FAQ

**パッケージ名がわからない**
`nixy search <キーワード>` を使ってください。パッケージ名は予想と異なることがあります（例：`rg` ではなく `ripgrep`）。

**パッケージは実際にどこにインストールされる？**
Nix ストア（`/nix/store/`）にインストールされます。nixy は統合された環境をビルドし、`~/.local/state/nixy/env` にシンボリックリンクを作成します。`nixy config` コマンドでこの場所を PATH に追加する設定を行います。

**flake.nix を手動で編集できる？**
`flake.nix` は操作のたびに nixy の状態ファイル（`packages.json`）から完全に再生成されます。手動での編集は上書きされます。

カスタムパッケージには、サポートされている方法を使用してください：
- `nixy install --from <flake> <pkg>` - 外部 flake から
- `nixy install --file <path>` - カスタム nix 定義から
- `packages/` ディレクトリにファイルを配置 - 自動検出

詳細は付録の「カスタムパッケージ定義」を参照してください。

**nixy をアップデートするには？**
`nixy self-upgrade` で自動的に最新版にアップデートできます。または `cargo install nixy` やインストールスクリプトの再実行でも可能です。

**nixy をアンインストールするには？**
`nixy` スクリプトを削除するだけ。flake.nix ファイルはそのまま残り、標準の `nix` コマンドで使えます。

**なぜ `nix profile` を直接使わないの？**
`nix profile` には再現性の仕組みがありません - パッケージをエクスポートして別のマシンで同じ環境を再現する公式の方法がないのです。nixy は `packages.json` を真実の源として使い、再現可能な `flake.nix` を生成するため、コピー、バージョン管理、共有が可能です。

**以前の状態にロールバックするには？**
nixy は宣言的なので、`packages.json` と `flake.lock` が状態そのものです。git で管理していれば（推奨）、ロールバックは簡単：

```bash
git checkout HEAD~1 -- packages.json flake.lock  # 前のコミットに戻す
nixy sync                                         # flake.nix を再生成して適用
```

これは `nix profile rollback` より強力です - 履歴の任意の時点に戻れる、コミットメッセージで変更理由がわかる、ブランチで実験できる、といった利点があります。

**非フリーパッケージをインストールするには？**
非フリーライセンスのパッケージ（例：`graphite-cli`、`slack`）は nixy でデフォルトで許可されています。通常通りインストールできます：

```bash
nixy install slack
```

**古い Nix ストアパスをクリーンアップするには？**
nixy は `nix profile` ではなく `nix build --out-link` を使用しているため、ガベージコレクションコマンドを提供していません。未使用の Nix ストアパスをクリーンアップするには、標準の Nix コマンドを直接使用してください：

```bash
nix-collect-garbage -d
```

注意：これは nixy 関連のものだけでなく、システム上のすべての未使用の Nix プロファイルとストアパスをクリーンアップします。

---

## 付録

### nixy の状態管理

nixy は各プロファイルディレクトリの `packages.json` ファイルを真実の源として使用します。`flake.nix` は操作のたびにこの状態から完全に再生成されます。

```
~/.config/nixy/profiles/default/
├── packages.json    # 真実の源（nixy が管理）
├── flake.nix        # 生成ファイル（手動編集しない）
├── flake.lock       # Nix ロックファイル
└── packages/        # カスタムパッケージ定義
    ├── my-tool.nix
    └── my-flake/
        └── flake.nix
```

この設計により：
- 同期が狂うマーカーベースの編集が不要
- 状態と生成出力の明確な分離
- 簡単なバックアップ（`packages.json` と `packages/` ディレクトリをコピーするだけ）

### 既存の Nix ユーザー向け

すでに独自の `flake.nix` を管理していて、nixy のパッケージリストを使いたい場合は、インポートできます：

```nix
{
  inputs.nixy-packages.url = "path:~/.config/nixy/profiles/default";

  outputs = { self, nixpkgs, nixy-packages }: {
    # nixy-packages.packages.<system>.default は全 nixy パッケージを含む buildEnv
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

ファイルは `packages/` ディレクトリにコピーされ、flake 生成時に自動検出されます。

**シンプルなパッケージの形式**（`my-package.nix`）：
```nix
{
  pname = "my-package";  # または "name"
  overlay = "overlay-name.overlays.default";
  packageExpr = "pkgs.my-package";
  # オプション：カスタム inputs
  input.overlay-name.url = "github:user/repo";
}
```

**flake ベースのパッケージの形式**：

`flake.nix` を含むディレクトリを `packages/` に配置：
```
packages/my-tool/flake.nix
```

nixy は自動的にパス input として追加し、デフォルトパッケージを含めます。

**自動検出**：

`packages/` ディレクトリ内のファイルは自動的に含まれます：
- `packages/*.nix` - 単一ファイルパッケージ
- `packages/*/flake.nix` - flake ベースのパッケージ

`nixy install --file` を使わずに `packages/` に直接ファイルを配置することもできます。

### 設定ファイルの場所

| パス | 説明 |
|------|------|
| `~/.config/nixy/profiles/<name>/packages.json` | パッケージ状態（真実の源） |
| `~/.config/nixy/profiles/<name>/flake.nix` | 生成された flake（編集しない） |
| `~/.config/nixy/profiles/<name>/flake.lock` | Nix ロックファイル（バージョン管理推奨） |
| `~/.config/nixy/profiles/<name>/packages/` | カスタムパッケージ定義（自動検出） |
| `~/.config/nixy/active` | 現在のアクティブプロファイル名 |
| `~/.local/state/nixy/env` | ビルド済み環境へのシンボリックリンク（`bin/` を PATH に追加） |

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
