# nixy - シンプルな宣言的 Nix パッケージ管理

## Why nixy?

いつもasdfやHomebrewにいらいらしながら仕事をしていて、Nixに何度か入門したもののラーニングカーブがきつくて挑戦するたびに挫折していました。シンプルな開発環境でとりあえず欲しいのは、Nixの能力をバックエンドに使ったシンプルなasdf/Homebrewの代替だと気づいたので、Nixの巨大なリポジトリを利用できて環境が再現できてprofile機能もついたシンプルなラッパーを作ってみました。Rustで快適に動作してとても気に入っています。

---

**再現性のある Nix パッケージを、シンプルなコマンドで。** コマンド一つでインストール、すべてのマシンで同期。

```bash
nixy install ripgrep    # これだけ。シンプルな Nix 生活。
```

nixy は宣言的な `nixy.json` 設定ファイルで Nix パッケージを管理し、どのマシンでも同じパッケージ、同じバージョンを保証します。

## 前提条件

nixy には Nix が必要です：

```bash
curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
```

## クイックスタート

### 1. nixy をインストール

```bash
# クイックインストール（推奨）
curl -fsSL https://raw.githubusercontent.com/yusukeshib/nixy/main/install.sh | bash

# または cargo で
cargo install nixy

# または nix で
nix profile install github:yusukeshib/nixy
```

### 2. シェルを設定

`.bashrc`、`.zshrc` などに追加：

```bash
eval "$(nixy config zsh)"
```

fish の場合（`~/.config/fish/config.fish`）：

```fish
nixy config fish | source
```

### 3. 使い始める

```bash
nixy install ripgrep        # 最新バージョンをインストール
nixy install nodejs@20      # 特定のメジャーバージョン
nixy install python@3.11.5  # 厳密なバージョン
nixy list                   # バージョン付きでパッケージを表示
nixy search python          # パッケージ + バージョン一覧を検索
nixy uninstall nodejs       # パッケージを削除
nixy upgrade                # バージョン制約内でアップグレード
nixy upgrade nodejs         # 特定のパッケージをアップグレード
```

## コマンド

| コマンド | 説明 |
|---------|------|
| `nixy install <pkg>[@version]` | バージョン指定でインストール（エイリアス: `add`） |
| `nixy install --from <flake> <pkg>` | flake URL からインストール |
| `nixy install --file <path>` | nix ファイルからインストール |
| `nixy install <pkg> --platform <platform>` | 特定のプラットフォームのみにインストール |
| `nixy uninstall <pkg>` | パッケージをアンインストール（エイリアス: `remove`） |
| `nixy list` | バージョン付きでパッケージを表示（エイリアス: `ls`） |
| `nixy search <query>` | パッケージ + バージョン情報を検索 |
| `nixy upgrade [pkg...]` | バージョン制約内でアップグレード |
| `nixy sync` | flake.nix から再ビルド |
| `nixy profile` | プロファイル一覧 + 対話的 TUI 選択 |
| `nixy profile <name>` | プロファイルを切り替え |
| `nixy profile <name> -c` | プロファイルを作成して切り替え |
| `nixy profile <name> -d` | プロファイルを削除（確認あり） |
| `nixy file <pkg>` | パッケージのソースファイルパスを表示 |
| `nixy self-upgrade` | nixy 自体をアップグレード |

### バージョン指定

nixy は [Nixhub](https://nixhub.io) 経由で柔軟なバージョン指定をサポート：

```bash
nixy install nodejs           # 最新バージョン
nixy install nodejs@20        # 最新の 20.x.x（semver 範囲）
nixy install nodejs@20.11     # 最新の 20.11.x
nixy install nodejs@20.11.0   # 厳密なバージョン
```

`nixy upgrade nodejs` を実行すると、バージョン制約が尊重されます：
- `nodejs`（バージョンなし）→ 最新に更新
- `nodejs@20` → 最新の 20.x.x に更新

### プラットフォーム固有のインストール

特定のプラットフォームにのみパッケージをインストール：

```bash
nixy install terminal-notifier --platform darwin   # macOS のみ
nixy install linux-tool --platform linux           # Linux のみ
nixy install specific --platform aarch64-darwin    # Apple Silicon のみ
```

有効なプラットフォーム名：
- `darwin` または `macos` → `x86_64-darwin` と `aarch64-darwin` の両方
- `linux` → `x86_64-linux` と `aarch64-linux` の両方
- フルネーム: `x86_64-darwin`, `aarch64-darwin`, `x86_64-linux`, `aarch64-linux`

プラットフォーム固有のパッケージは `nixy list` で制限が表示されます：
```
terminal-notifier@2.0.0  (nixpkgs) [darwin]
```

## プロファイル

用途別にパッケージセットを分けて管理：

```bash
nixy profile work -c            # 新しいプロファイルを作成して切り替え
nixy install slack terraform    # 仕事用パッケージをインストール

nixy profile personal -c        # 別のプロファイル
nixy install spotify            # 別のパッケージ

nixy profile                    # 対話的プロファイル選択
nixy profile work               # 既存のプロファイルに切り替え
nixy profile old -d             # プロファイルを削除（確認あり）
```

全てのプロファイルは `~/.config/nixy/nixy.json` に保存され、生成された flake は `~/.local/state/nixy/profiles/<name>/` に配置されます。

## nixy の仕組み

nixy は**純粋に宣言的** - `nixy.json` が真実の源であり、`flake.nix` は操作のたびにそこから再生成されます。

```
┌─────────────────┐      ┌─────────────┐      ┌─────────────────────────────┐
│   nixy.json     │ ──── │  flake.nix  │ ──── │ ~/.local/state/nixy/env/bin │
│  (真実の源)      │ 生成  │ (+ flake.lock)│ nix build │   (/nix/store へのシンボリックリンク)│
└─────────────────┘      └─────────────┘      └─────────────────────────────┘
                                                            │
                                                            ▼
                                              eval "$(nixy config zsh)" で
                                              このパスを $PATH に追加
```

可変な状態を持つ `nix profile` とは異なり、nixy は：
1. 操作のたびに `nixy.json` から `flake.nix` を再生成
2. `nix build` を実行して `/nix/store` に統合された環境を作成
3. `~/.local/state/nixy/env` にその環境へのシンボリックリンクを作成
4. シェル設定が `~/.local/state/nixy/env/bin` を `$PATH` に追加

つまり同期は簡単：`nixy.json` と、使用しているプロファイルの `flake.lock` (例: `~/.local/state/nixy/profiles/<profile名>/flake.lock`) を別のマシンにコピーして `nixy sync` を実行すれば、全く同じ環境が再現できます。

## FAQ

**パッケージ名がわからない**
`nixy search <キーワード>` を使ってください。

**パッケージはどこにインストールされる？**
`/nix/store/` にインストールされます。nixy は `~/.local/state/nixy/env` にシンボリックリンクを作成します。

**flake.nix を手動で編集できる？**
できません。操作のたびに `nixy.json` から再生成されます。カスタムパッケージには `--from` や `--file` を使ってください。

**nix profile との違いは？**
nixy は Nix の上に再現性を追加します。`nixy.json` + `flake.lock` を複数マシン間で同期・バージョン管理できます。

**ロールバックするには？**
`nixy.json` と `flake.lock` を git で管理してください：
```bash
cd ~/.config/nixy
git checkout HEAD~1 -- nixy.json
nixy sync
```

---

## 詳細

<details>
<summary>ディレクトリ構造</summary>

```
~/.config/nixy/
├── nixy.json        # 真実の源（全プロファイル）
└── packages/        # グローバルカスタムパッケージ定義

~/.local/state/nixy/
├── env              # アクティブプロファイルのビルドへのシンボリックリンク
└── profiles/
    ├── default/
    │   ├── flake.nix    # 生成ファイル（編集しない）
    │   └── flake.lock   # Nix ロックファイル
    └── work/
        └── ...
```

</details>

<details>
<summary>カスタムパッケージ定義</summary>

**外部 flake から：**
```bash
nixy install --from github:nix-community/neovim-nightly-overlay neovim
```

**nix ファイルから：**
```bash
nixy install --file my-package.nix
```

`packages/` ディレクトリ内のファイルは自動検出されます。

</details>

<details>
<summary>既存の Nix ユーザー向け</summary>

nixy のパッケージリストを自分の flake にインポートできます：

```nix
{
  inputs.nixy-packages.url = "path:~/.local/state/nixy/profiles/default";

  outputs = { self, nixpkgs, nixy-packages }: {
    # nixy-packages.packages.<system>.default は全パッケージを含む buildEnv
  };
}
```

nixy と `nix profile` は別のパスを使うため競合しません。

</details>

<details>
<summary>設定ファイルの場所</summary>

| パス | 説明 |
|------|------|
| `~/.config/nixy/nixy.json` | 設定ファイル（全プロファイル） |
| `~/.config/nixy/packages/` | グローバルカスタムパッケージ定義 |
| `~/.local/state/nixy/profiles/<name>/flake.nix` | 生成された flake |
| `~/.local/state/nixy/profiles/<name>/flake.lock` | Nix ロックファイル |
| `~/.local/state/nixy/env` | 環境へのシンボリックリンク |

環境変数: `NIXY_CONFIG_DIR`, `NIXY_STATE_DIR`, `NIXY_ENV`

</details>

## ライセンス

MIT
