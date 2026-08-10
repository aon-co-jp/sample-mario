//! 8bit風のオリジナルBGMをループ再生する(ユーザー指示、2026-08-10
//! 「マリオ風BGMもお願い」への対応)。
//!
//! **正直な開示・著作権上の注意**: 実際の任天堂『スーパーマリオブラザーズ』
//! の楽曲(地上BGM等)を再現・転用することは著作権上できないため、それを
//! 意図的に避け、"8bit・軽快なテンポ・上昇するアルペジオ"という
//! **曲想(ジャンルの雰囲気)のみ**を参考にした完全に新規のオリジナル
//! メロディをここで作曲している。既存曲の旋律・拍数・構成の引用はない。
//!
//! 波形は矩形波(square wave)による簡易チップチューン合成。`rodio`の
//! `SamplesBuffer`にPCMサンプルを直接書き込み、`repeat_infinite()`で
//! 無限ループ再生する。

use rodio::{OutputStream, OutputStreamHandle, Sink, Source};

const SAMPLE_RATE: u32 = 44100;

/// 矩形波1音を指定周波数・長さ(秒)・音量でサンプル列へ生成する。
fn square_wave_note(freq: f32, seconds: f32, volume: f32, samples: &mut Vec<f32>) {
    let n = (SAMPLE_RATE as f32 * seconds) as usize;
    for i in 0..n {
        let t = i as f32 / SAMPLE_RATE as f32;
        let phase = (t * freq).fract();
        let value = if phase < 0.5 { volume } else { -volume };
        // 音の切れ目でのクリック雑音を避けるため、末尾を軽くフェードアウトする。
        let fade_start = n.saturating_sub(SAMPLE_RATE as usize / 200);
        let envelope = if i >= fade_start { (n - i) as f32 / (n - fade_start).max(1) as f32 } else { 1.0 };
        samples.push(value * envelope);
    }
}

/// オリジナルの8bit風メロディ(上昇アルペジオ+短い着地フレーズ)を
/// 1ループ分のPCMサンプル列として生成する。既存楽曲の引用はない。
fn compose_original_melody() -> Vec<f32> {
    // 音名は演奏用の周波数指定であり、既存曲のメロディそのものではない
    // (単純な上昇→下降アルペジオの独自パターン)。
    const NOTE: f32 = 0.14;
    let notes: &[(f32, f32)] = &[
        (523.25, NOTE), // C5
        (659.25, NOTE), // E5
        (783.99, NOTE), // G5
        (1046.50, NOTE * 1.5), // C6
        (783.99, NOTE), // G5
        (659.25, NOTE), // E5
        (587.33, NOTE), // D5
        (523.25, NOTE * 1.5), // C5
        (440.00, NOTE), // A4
        (523.25, NOTE), // C5
        (587.33, NOTE), // D5
        (698.46, NOTE * 1.5), // F5
        (659.25, NOTE), // E5
        (523.25, NOTE), // C5
        (440.00, NOTE), // A4
        (392.00, NOTE * 2.0), // G4(フレーズの終止)
    ];
    let mut samples = Vec::new();
    for &(freq, dur) in notes {
        square_wave_note(freq, dur, 0.12, &mut samples);
    }
    samples
}

/// BGMを再生開始する。返り値の`(OutputStream, OutputStreamHandle, Sink)`を
/// 呼び出し側で保持し続けている間だけ再生される(dropすると停止するため、
/// `main()`側でイベントループが動いている間ずっとスコープに残す)。
pub fn play_bgm_loop() -> Option<(OutputStream, OutputStreamHandle, Sink)> {
    let (stream, handle) = match OutputStream::try_default() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("BGM再生用のオーディオ出力デバイスを開けませんでした(BGM無しで続行します): {e}");
            return None;
        }
    };
    let sink = match Sink::try_new(&handle) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("BGM再生用のSinkを作成できませんでした(BGM無しで続行します): {e}");
            return None;
        }
    };
    let melody = compose_original_melody();
    let source = rodio::buffer::SamplesBuffer::new(1, SAMPLE_RATE, melody).repeat_infinite();
    sink.append(source);
    Some((stream, handle, sink))
}
