use transcriber_audio::TestToneSources;

use super::run;
use crate::language::Language;

#[test]
fn lists_the_test_factorys_one_subject_with_its_root_pid() {
    let Ok(factory) = TestToneSources::new() else {
        panic!("fixed 48 kHz stereo format is always valid");
    };
    let mut output = Vec::new();

    let Ok(()) = run(&factory, Language::En, &mut output) else {
        panic!("listing a test factory's subjects must not fail");
    };

    let rendered = String::from_utf8_lossy(&output);
    assert!(rendered.contains("test-tone"));
    assert!(rendered.contains("PID"));
}
