use crate::audio::{AudioStateChangedEventArgs, SpectrumAnalyzer};
use std::f64::consts::PI;

const RATE: i32 = 48_000;
const FFT_SIZE: usize = SpectrumAnalyzer::FFT_SIZE as usize;

fn sine(hz: f64, amplitude: f64, count: usize) -> Vec<f32> {
    let mut samples = vec![0.0f32; count];
    for (i, s) in samples.iter_mut().enumerate() {
        *s = (amplitude * (2.0 * PI * hz * i as f64 / RATE as f64).sin()) as f32;
    }
    samples
}

fn sine_default(hz: f64) -> Vec<f32> {
    sine(hz, 0.5, FFT_SIZE)
}

fn loudest_band(analyzer: &SpectrumAnalyzer) -> usize {
    let levels = analyzer.levels();
    let mut best = 0;
    for i in 1..levels.len() {
        if levels[i] > levels[best] {
            best = i;
        }
    }
    best
}

fn run(samples: &[f32]) -> SpectrumAnalyzer {
    let mut analyzer = SpectrumAnalyzer::new(24);
    for _ in 0..8 {
        analyzer.add_samples(samples);
        analyzer.update(RATE);
    }
    analyzer
}

#[test]
fn bas_sol_tiz_sag_bantta_cikar() {
    let bass = loudest_band(&run(&sine_default(80.0)));
    let mid = loudest_band(&run(&sine_default(1000.0)));
    let treble = loudest_band(&run(&sine_default(8000.0)));

    assert!(bass < mid, "bas {bass}, orta {mid}");
    assert!(mid < treble, "orta {mid}, tiz {treble}");
    assert!(bass <= 3);
    assert!(treble >= 18);
}

#[test]
fn sessizlikte_cubuklar_iner() {
    let mut analyzer = run(&sine_default(1000.0));
    assert!(!analyzer.is_silent());

    for _ in 0..60 {
        analyzer.add_silence(SpectrumAnalyzer::FFT_SIZE);
        analyzer.update(RATE);
    }

    assert!(analyzer.is_silent());
}

#[test]
fn duzeyler_sifir_ile_bir_arasinda() {
    let analyzer = run(&sine(440.0, 1.0, FFT_SIZE));
    for &level in analyzer.levels() {
        assert!((0.0f32..=1.0f32).contains(&level), "{level}");
    }
}

#[test]
fn yuksek_ses_kisik_sesten_uzun_cubuk_verir() {
    let loud = run(&sine(1000.0, 0.8, FFT_SIZE));
    let quiet = run(&sine(1000.0, 0.01, FFT_SIZE));
    assert!(loud.levels()[loudest_band(&loud)] > quiet.levels()[loudest_band(&quiet)]);
}

#[test]
fn bant_sinirlari_artan_ve_sinirda() {
    for rate in [44_100, 48_000, 96_000, 8_000] {
        let edges = SpectrumAnalyzer::band_edges(24, rate);
        assert_eq!(25, edges.len(), "{rate}");
        for i in 1..edges.len() {
            assert!(edges[i] > edges[i - 1] || edges[i] == SpectrumAnalyzer::FFT_SIZE / 2, "{rate} @ {i}");
        }
        assert!(edges[0] >= 1, "{rate}");
        assert!(*edges.last().unwrap() <= SpectrumAnalyzer::FFT_SIZE / 2, "{rate}");
    }
}

#[test]
fn kisa_parcalar_halka_bellekte_birikir() {
    let mut analyzer = SpectrumAnalyzer::new(24);
    let sine = sine_default(1000.0);
    for _ in 0..8 {
        for chunk in sine.chunks(128) {
            analyzer.add_samples(chunk);
        }
        analyzer.update(RATE);
    }

    assert!(!analyzer.is_silent());
}

#[test]
fn tiz_bantlar_egimle_yukseltilir() {
    let tilt = SpectrumAnalyzer::band_tilt(&SpectrumAnalyzer::band_edges(24, RATE), RATE);
    for i in 1..tilt.len() {
        assert!(tilt[i] >= tilt[i - 1]);
    }
    assert!(tilt[0] < 0.0);
    assert!(*tilt.last().unwrap() > 10.0);
}

#[test]
#[should_panic]
fn gecersiz_bant_sayisi_reddedilir() {
    let _ = SpectrumAnalyzer::new(0);
}

// ModelContractTests.AudioStateChangedEventArgs_InitializesProperly
#[test]
fn audio_state_changed_event_args_initializes_properly() {
    let args = AudioStateChangedEventArgs::new(0.75, true);
    assert_eq!(0.75f32, args.volume);
    assert!(args.is_muted);
}
