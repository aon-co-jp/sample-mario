# sample-mario

マリオ風2Dプラットフォーマーのショートゲームプロトタイプ。

[`open-directx`](https://github.com/aon-co-jp/open-directx) の
DXBC→SPIR-V翻訳済みスプライトパイプライン
(`directx-shader-translate` / `directx-graphics-vulkan`)を
Cargo pathの依存として再利用し、実ウィンドウ+Vulkanスワップチェーンで
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

`open-directx` を隣に(`../open-directx`)cloneしている前提の
Cargo path依存になっている。
