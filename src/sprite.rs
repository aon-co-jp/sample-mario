//! 手描きのドット絵パターンをRGBA8テクスチャへ焼き込む(ユーザー指示、
//! 2026-08-10「PNGドット絵スプライト(事前生成)」への対応)。`font.rs`と
//! 同じ「文字→ドットパターン→RGBA8」という考え方を、キャラクター
//! スプライト(自機・亀・ボス)に適用したもの。
//!
//! パターンは1行1文字列の配列(全行同じ長さ)で表し、各文字を
//! `palette`関数でRGBA8色へマッピングする。`.`は背景色(スプライトの
//! 縁取り・下地)として扱う。実際の画像ファイル(PNG)を用意する代わりに、
//! このプログラム自身がドット絵を生成する——アセット制作ツールが無い
//! Phase 0の制約下での実務的な折衷案(正直な開示)。

use directx_graphics_vulkan::{Rgba8, TextureRgba8};

pub fn rasterize(pattern: &[&str], palette: impl Fn(char) -> Rgba8, scale: usize) -> TextureRgba8 {
    let rows = pattern.len();
    let cols = pattern[0].chars().count();
    let width = cols * scale;
    let height = rows * scale;
    let mut pixels = vec![palette('.'); width * height];

    for (row, line) in pattern.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            let color = palette(ch);
            for dy in 0..scale {
                for dx in 0..scale {
                    let x = col * scale + dx;
                    let y = row * scale + dy;
                    pixels[y * width + x] = color;
                }
            }
        }
    }

    TextureRgba8 { width: width as u32, height: height as u32, pixels }
}

/// 自機(独自デザインの小さなヒーロー、赤い服+青いオーバーオール風)。
pub const PLAYER: &[&str] = &[
    "....RRRR....",
    "...RRRRRR...",
    "...SSSSSS...",
    "..SSKKKKSS..",
    "..SKKSKSKK..",
    "...RRRRRR...",
    "..ROOOOOOR..",
    ".ROOOOOOOOR.",
    "..OO....OO..",
    "..OO....OO..",
    ".BBB....BBB.",
    "BBBB....BBBB",
];

/// 雑魚敵(亀、独自デザイン)。
pub const TURTLE: &[&str] = &[
    "....TTTT....",
    "..TTTTTTTT..",
    ".TTGGGGGGTT.",
    "TTGGWWWWGGTT",
    "TTGWWKKWWGT.",
    "TTGGWWWWGGTT",
    ".TTGGGGGGTT.",
    "..TTTTTTTT..",
    "...T....T...",
    "..TT....TT..",
];

/// ボス(甲羅の魔竜王、独自デザイン)。
pub const BOSS: &[&str] = &[
    "......NNNN......",
    "....NNNNNNNN....",
    "...NPPPPPPPPN...",
    "..NPPMMMMMMPPN..",
    ".NPPMWWWWWWMPPN.",
    "NPPMWW.WW.WWMPPN",
    "NPPMW......WMPPN",
    ".NPPMWWWWWWMPPN.",
    "..NPPMMMMMMPPN..",
    "...NPPPPPPPPN...",
    "....NN.NN.NN....",
    ".....N..N.......",
];

pub fn player_palette(c: char) -> Rgba8 {
    match c {
        'R' => Rgba8 { r: 220, g: 30, b: 30, a: 255 },
        'S' => Rgba8 { r: 245, g: 200, b: 170, a: 255 },
        'K' => Rgba8 { r: 40, g: 30, b: 20, a: 255 },
        'O' => Rgba8 { r: 220, g: 30, b: 30, a: 255 },
        'B' => Rgba8 { r: 40, g: 90, b: 220, a: 255 },
        _ => Rgba8 { r: 20, g: 20, b: 30, a: 255 },
    }
}

pub fn turtle_palette(c: char) -> Rgba8 {
    match c {
        'T' => Rgba8 { r: 30, g: 120, b: 40, a: 255 },
        'G' => Rgba8 { r: 60, g: 170, b: 70, a: 255 },
        'W' => Rgba8 { r: 240, g: 220, b: 120, a: 255 },
        'K' => Rgba8 { r: 40, g: 30, b: 20, a: 255 },
        _ => Rgba8 { r: 20, g: 20, b: 30, a: 255 },
    }
}

pub fn boss_palette(c: char) -> Rgba8 {
    match c {
        'N' => Rgba8 { r: 20, g: 70, b: 30, a: 255 },
        'P' => Rgba8 { r: 40, g: 110, b: 60, a: 255 },
        'M' => Rgba8 { r: 200, g: 60, b: 40, a: 255 },
        'W' => Rgba8 { r: 240, g: 220, b: 120, a: 255 },
        _ => Rgba8 { r: 20, g: 20, b: 30, a: 255 },
    }
}
