use std::time::Duration;
use ui_todo::mathalley::game::{Action, Game, LevelState, Phase};

#[test]
fn a_seeded_run_visits_both_games_and_restarts() {
    let mut game = Game::new(13);
    game.apply(Action::Start);
    let mut train = false;
    let mut gumball = false;
    for _ in 0..200 {
        match game.phase {
            Phase::Intro => game.apply(Action::SkipIntro),
            Phase::Playing => match &game.level {
                LevelState::Train(level) => {
                    train = true;
                    game.apply(Action::Choose(level.correct));
                }
                LevelState::Gumball(level) => {
                    gumball = true;
                    game.apply(Action::Choose(level.correct));
                }
                LevelState::Empty => panic!("playing requires a level"),
            },
            Phase::RoundTally => game.apply(Action::Start),
            Phase::GameOver => break,
            Phase::Title => panic!("start must leave the title"),
        }
        game.update(game.now + Duration::from_secs(2));
    }
    assert!(train && gumball);
    assert_eq!(game.phase, Phase::GameOver);
    assert!(game.banked_score > 0);
    game.apply(Action::Start);
    assert_eq!(game.phase, Phase::Title);
    game.apply(Action::Start);
    assert_eq!(game.phase, Phase::Intro);
}

#[test]
fn pointer_input_excludes_bars_and_background_time() {
    use ui_todo::mathalley::Session;
    let mut session = Session::new(5);
    assert_eq!(session.tap(25., 422., 1000., 844.), None);
    assert_eq!(session.tap(500., 422., 1000., 844.), Some(Action::Start));
    session.tick(Duration::from_secs(1));
    session.tick(Duration::from_millis(1400));
    session.set_active(false);
    assert_eq!(session.tap(500., 422., 1000., 844.), None);
    session.tick(Duration::from_secs(80));
    session.set_active(true);
    session.tick(Duration::from_secs(100));
    assert_eq!(session.game.now, Duration::from_millis(400));
    assert_eq!(session.game.phase, Phase::Intro);
    session.tick(Duration::from_secs(101));
    assert_eq!(session.game.phase, Phase::Playing);
}

#[test]
fn shared_audio_is_seeded_and_mutes_all_voices() {
    use ui_todo::mathalley::audio::{Cue, Mixer};
    let mut first = Mixer::new(5, 16000);
    let mut second = Mixer::new(5, 16000);
    let mut a = vec![0.; 1200];
    let mut b = a.clone();
    first.cue(Cue::Wrong);
    second.cue(Cue::Wrong);
    first.fill(16000, &mut a, true);
    second.fill(16000, &mut b, true);
    assert_eq!(a, b);
    assert!(a.iter().any(|v| v.abs() > 0.01));
    first.fill(16000, &mut a, false);
    assert!(a.iter().all(|v| *v == 0.0));
}

#[test]
fn recording_host_runs_two_views_and_discards_closed_output() {
    use foundation_context::Ctx;
    use foundation_ui_runtime::sound::{
        PullOutput, SoundError, SoundHost, SoundOutput, SoundSession, SoundSource,
    };
    use std::sync::{Arc, Mutex};
    use ui_todo::mathalley::audio::Track;
    #[derive(Default)]
    struct RecordingHost(Mutex<usize>);
    impl SoundOutput for RecordingHost {
        fn open(&self, source: Arc<dyn SoundSource>) -> Result<SoundSession, SoundError> {
            *self.0.lock().unwrap() += 1;
            PullOutput.open(source)
        }
    }
    let ctx = Ctx::host();
    let host = Arc::new(RecordingHost::default());
    assert!(matches!(
        Track::new(&ctx, 1, 16000),
        Err(SoundError::MissingHost)
    ));
    ctx.store(SoundHost(host.clone()));
    let mut game_sound = Track::new(&ctx, 8, 16000).unwrap();
    let other = Track::new(&ctx, 8, 16000).unwrap();
    assert_eq!(*host.0.lock().unwrap(), 2);
    let mut game = Game::new(8);
    game.apply(Action::Start);
    game_sound.observe(&game).unwrap();
    let mut a = vec![0.; 1000];
    let mut b = a.clone();
    game_sound
        .output
        .render(Duration::ZERO, 16000, &mut a)
        .unwrap();
    other.output.render(Duration::ZERO, 16000, &mut b).unwrap();
    assert_ne!(a, b, "the intro cue reaches only its view");
    game_sound.output.volume(0.).unwrap();
    game_sound.output.active(false).unwrap();
    game_sound
        .output
        .render(Duration::from_secs(200), 16000, &mut a)
        .unwrap();
    assert!(a.iter().all(|v| *v == 0.));
    game_sound.output.active(true).unwrap();
    game_sound
        .output
        .render(Duration::from_secs(201), 16000, &mut a)
        .unwrap();
    assert!(a.iter().all(|v| *v == 0.));
    game_sound.output.close().unwrap();
    assert_eq!(
        game_sound
            .output
            .render(Duration::from_secs(202), 16000, &mut a),
        Err(SoundError::Closed)
    );
    other
        .output
        .render(Duration::from_secs(202), 16000, &mut b)
        .unwrap();
    assert!(b.iter().any(|v| v.abs() > 0.001));
}

#[test]
fn a_wrong_train_answer_costs_one_life_and_the_last_miss_ends_the_run() {
    let mut game = Game::new(17);
    game.apply(Action::Start);
    game.apply(Action::SkipIntro);
    while game.lives > 0 {
        let LevelState::Train(train) = &game.level else {
            panic!("The first level must be Train")
        };
        let wrong = (train.correct + 1) % train.options.len();
        let lives = game.lives;
        game.apply(Action::Choose(wrong));
        assert_eq!(game.lives, lives - 1);
        game.update(game.now + Duration::from_secs(1));
    }
    assert_eq!(game.phase, Phase::GameOver);
}

#[test]
fn all_hosts_classify_the_actual_game_command() {
    use ui_todo::mathalley::route::{is_mathalley_open, open_command};
    let bytes = open_command().unwrap();
    assert_eq!(bytes, rmp_serde::to_vec("open:mathalley").unwrap());
    assert!(is_mathalley_open(bytes.clone()));
    assert!(!is_mathalley_open(bytes[..bytes.len()-1].to_vec()));
    assert!(!is_mathalley_open([bytes, vec![0]].concat()));
    assert!(!is_mathalley_open(rmp_serde::to_vec("open:other").unwrap()));
    assert!(!is_mathalley_open(Vec::new()));
    assert!(!is_mathalley_open(vec![0]));
    assert!(!is_mathalley_open(vec![0x81, 0, 0xc0]));
    assert!(!is_mathalley_open([vec![0x81], open_command().unwrap(), vec![0xc0]].concat()));
    assert!(!is_mathalley_open([vec![0xc4, 14], b"open:mathalley".to_vec()].concat()));
}
