# Alius

[![npm version](https://img.shields.io/npm/v/@alius-tech/alius)](https://www.npmjs.com/package/@alius-tech/alius)
[![homebrew version](https://img.shields.io/homebrew/v/alius?label=homebrew&url=https://raw.githubusercontent.com/AliusTech/homebrew-tap/main/Formula/alius.rb)](https://github.com/AliusTech/homebrew-tap/blob/main/Formula/alius.rb)

> LLM Agent CLI -- ソフトウェア自己進化のエンジニアリング実践

[中文](README.md) | [English](README.en.md)

Alius は、AI 駆動のソフトウェア開発のためのエンジニアリング実践プラットフォームを目指すインタラクティブな LLM コマンドラインツールです。

## コア理念 -- ソフトウェア自己進化

ソフトウェア自己進化は、Alius プロジェクトのコア設計思想です。

従来のソフトウェア開発では、開発者がコード、テスト、ドキュメントを書いてから納品します。しかし、AI 時代のソフトウェアは**自身の進化プロセスに参加できる**べきです。

Alius CLI はソフトウェア自己進化の生きた実例です -- その開発プロセスは AI アシスタンスを最大限に活用し、人間と AI の協力がソフトウェア開発サイクルを如何に加速できるかを示しています。

**自己反復** -- AI によるコード改善と新機能開発。ソフトウェアは一度きりの納品物ではなく、継続的に進化する生命体です。

**適応的設定** -- 使用シナリオに基づくインテリジェントなパラメータ調整。ツールが人に適応するのです。

**ツールベースアーキテクチャ** -- モジュラーで拡張可能なツールシステム設計。エージェントが外部機能を呼び出し、複雑なタスクを完了できます。

## 機能

- **インタラクティブ REPL** -- スラッシュコマンドとリアルタイムチャット
- **マルチモデル対応** -- OpenAI、Anthropic、Gemini など
- **柔軟な設定** -- YAML/TOML/JSON 設定ファイル
- **美しい UI** -- ASCII ロゴとカラー出力

## インストール

npm でインストール：

```bash
npm install -g @alius-tech/alius
```

Homebrew でインストール：

```bash
brew tap AliusTech/tap
brew install alius
```

更新：

```bash
# npm
npm update -g @alius-tech/alius

# Homebrew
brew update && brew upgrade alius
```

アンインストール：

```bash
# npm
npm uninstall -g @alius-tech/alius

# Homebrew
brew uninstall alius && brew untap AliusTech/tap
```

## 使い方

```bash
# インタラクティブモード
alius

# タスクを直接実行
alius run -p "Rust とは何か説明して"

# 設定を表示
alius config show
```

## REPL コマンド

| コマンド | 説明 |
|----------|------|
| `/model` | モデル選択 |
| `/config` | 設定表示 |
| `/help` | ヘルプ表示 |
| `/quit` | 終了 |

## 設定ファイル

`~/.alius/config.toml` に配置：

```toml
[llm]
provider = "openai"
model = "gpt-4o-mini"
api_key_env = "OPENAI_API_KEY"
base_url = "https://api.openai.com/v1"

[agent]
max_retries = 3
timeout_seconds = 60
```

## 対応モデル

OpenAI および Anthropic 形式の API エンドポイントをサポート。

- OpenAI: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `gpt-3.5-turbo`
- Anthropic: `claude-3-5-sonnet`, `claude-3-opus`, `claude-3-haiku`
- Google: `gemini-1.5-pro`, `gemini-1.5-flash`

## ライセンス

MIT License - [LICENSE](LICENSE) を参照
