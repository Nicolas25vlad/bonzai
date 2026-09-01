use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TestEnv {
    root: PathBuf,
    data_home: PathBuf,
    runtime_dir: PathBuf,
    home: PathBuf,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "bonzai-test-{name}-{}-{stamp}-{id}",
            std::process::id()
        ));
        let data_home = root.join("data");
        let runtime_dir = root.join("run");
        let home = root.join("home");

        fs::create_dir_all(&data_home).unwrap();
        fs::create_dir_all(&runtime_dir).unwrap();
        fs::create_dir_all(&home).unwrap();

        Self {
            root,
            data_home,
            runtime_dir,
            home,
        }
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_bonzai"));
        cmd.env("XDG_DATA_HOME", &self.data_home)
            .env("XDG_RUNTIME_DIR", &self.runtime_dir)
            .env("HOME", &self.home)
            .env("TERM", "xterm-256color");
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command()
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("failed to run bonzai {args:?}: {e}"))
    }

    fn assert_ok(&self, args: &[&str]) -> Output {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "bonzai {args:?} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn state_path(&self) -> PathBuf {
        self.data_home.join("bonzai/state.txt")
    }

    fn state(&self) -> HashMap<String, String> {
        parse_state(&self.state_path())
    }

    fn stop_daemon(&self) {
        let _ = self.run(&["stop"]);
        thread::sleep(Duration::from_millis(180));
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        self.stop_daemon();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn parse_state(path: &Path) -> HashMap<String, String> {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("could not read state file {}: {e}", path.display()));
    raw.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

fn state_f32(state: &HashMap<String, String>, key: &str) -> f32 {
    state
        .get(key)
        .unwrap_or_else(|| panic!("missing state key {key}"))
        .parse::<f32>()
        .unwrap_or_else(|e| panic!("invalid float for {key}: {e}"))
}

#[test]
fn version_and_help_are_available() {
    let env = TestEnv::new("version-help");

    let version = env.assert_ok(&["--version"]);
    let version = String::from_utf8_lossy(&version.stdout);
    assert!(version.starts_with("bonzai "), "unexpected version output: {version}");

    let help = env.assert_ok(&["help"]);
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("bonzai <command>"));
    assert!(help.contains("water"));
    assert!(help.contains("watch"));
}

#[test]
fn init_creates_a_complete_bounded_state_file() {
    let env = TestEnv::new("init-state");
    env.assert_ok(&["init"]);

    let state = env.state();
    for key in [
        "seed",
        "born_at",
        "last_tick",
        "water",
        "light",
        "health",
        "growth",
        "light_dir",
        "prune_left",
        "prune_right",
        "prune_top",
        "light_left_hours",
        "light_center_hours",
        "light_right_hours",
        "drought_stress",
        "wet_stress",
    ] {
        assert!(state.contains_key(key), "missing state key: {key}");
    }

    for key in ["water", "light", "health", "growth"] {
        let value = state_f32(&state, key);
        assert!((0.0..=100.0).contains(&value), "{key} out of bounds: {value}");
    }
}

#[test]
fn daemon_round_trip_updates_and_persists_water() {
    let env = TestEnv::new("daemon-water");
    env.assert_ok(&["init"]);
    env.assert_ok(&["start"]);

    let before = state_f32(&env.state(), "water");
    env.assert_ok(&["water"]);
    let after = state_f32(&env.state(), "water");
    assert!(after > before, "watering did not increase water: {before} -> {after}");
    assert!(after <= 100.0);

    let status = env.assert_ok(&["status"]);
    assert!(String::from_utf8_lossy(&status.stdout).contains("water"));

    env.assert_ok(&["stop"]);
    thread::sleep(Duration::from_millis(220));

    let persisted = state_f32(&env.state(), "water");
    assert!((persisted - after).abs() < 0.2, "state changed across stop: {after} -> {persisted}");
}

#[test]
fn repeated_watering_is_bounded_and_does_not_kill_the_daemon() {
    let env = TestEnv::new("water-bounds");
    env.assert_ok(&["init"]);
    env.assert_ok(&["start"]);

    for _ in 0..10 {
        env.assert_ok(&["water"]);
    }

    let water = state_f32(&env.state(), "water");
    assert!(water <= 100.0, "water exceeded 100%: {water}");
    assert!(water >= 90.0, "water unexpectedly low after repeated watering: {water}");

    let status = env.assert_ok(&["status"]);
    assert!(status.status.success(), "daemon stopped responding after repeated actions");
}

#[test]
fn light_and_prune_commands_persist_their_directional_state() {
    let env = TestEnv::new("light-prune");
    env.assert_ok(&["init"]);
    env.assert_ok(&["start"]);

    env.assert_ok(&["light", "right"]);
    env.assert_ok(&["prune", "left"]);

    let state = env.state();
    assert_eq!(state.get("light_dir").map(String::as_str), Some("1"));
    assert_eq!(state.get("prune_left").map(String::as_str), Some("1"));
}
