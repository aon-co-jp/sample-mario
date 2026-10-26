# sample-mario

マリオ風2Dプラットフォーマーのショートゲームプロトタイプ。

[`open-directx`](https://github.com/aon-co-jp/open-directx) の
DXBC→SPIR-V翻訳済みスプライトパイプライン
(`directx-shader-translate` / `directx-graphics-vulkan`)を
Cargoの依存(GitHubの`open-directx`)として再利用し、実ウィンドウ+Vulkanスワップチェーンで
動作する。

## 現在のスコープ(Phase 0)

重力+ジャンプ+地面/足場の当たり判定+ゴール到達判定のみの最小限の
プラットフォーマー。複数ステージ・敵・アイテム・スクロールする
広いマップは未実装。

## 実行方法

```bash
cargo run --release
```

矢印キー/AD左右移動、Space/Upでジャンプ、Escapeで終了。

## ディレクトリ構成

`open-directx` はGitHubから自動取得されます(隣にcloneする必要はありません)。

## 注意 / Notice

任天堂(Nintendo)の「スーパーマリオ」シリーズとは無関係の、独自実装の小さなプロトタイプです。キャラクター・音・画像は一切使っていません(すべて矩形などの自作描画)。

This is an independent prototype unrelated to Nintendo's Super Mario series. It uses none of its characters, sounds or images (everything is self-drawn shapes).

## ライセンス / License

MIT(`LICENSE`参照)。 / MIT (see `LICENSE`).

## 配布物 / Releases

Windows (x86_64) のビルド済みバイナリは Releases から。Vulkan対応のGPUとドライバが必要です。他のOSは `cargo run --release`(`open-directx` はGitHubから自動取得されます)。 / Prebuilt Windows (x86_64) binary on the Releases page; a Vulkan-capable GPU and driver are required. Other OSes: `cargo run --release` (`open-directx` is fetched from GitHub automatically).
