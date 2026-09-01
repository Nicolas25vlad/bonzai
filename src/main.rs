use std::{
    env,
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const VERSION: &str = "0.2.3";
const TICK_SECS: u64 = 30;
const ACTION_COOLDOWN_MS: u64 = 850;

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const TITLE: &str = "\x1b[38;5;180m";
const TRUNK: &str = "\x1b[38;5;137m";
const TRUNK_DARK: &str = "\x1b[38;5;94m";
const LEAF: &str = "\x1b[38;5;108m";
const LEAF_BRIGHT: &str = "\x1b[38;5;151m";
const LEAF_DARK: &str = "\x1b[38;5;65m";
const POT: &str = "\x1b[38;5;173m";
const WATER: &str = "\x1b[38;5;110m";
const SUN: &str = "\x1b[38;5;221m";
const HEALTH: &str = "\x1b[38;5;151m";
const MUTED: &str = "\x1b[38;5;245m";
const SOIL: &str = "\x1b[38;5;130m";

#[derive(Clone, Debug)]
struct State {
    seed: u64,
    born_at: u64,
    last_tick: u64,
    water: f32,
    light: f32,
    health: f32,
    growth: f32,
    light_dir: i8,
    prune_left: u32,
    prune_right: u32,
    prune_top: u32,
    light_left_hours: f32,
    light_center_hours: f32,
    light_right_hours: f32,
    drought_stress: f32,
    wet_stress: f32,
}

impl State {
    fn new() -> Self {
        let now = now_secs();
        Self {
            seed: now ^ (std::process::id() as u64).rotate_left(17),
            born_at: now,
            last_tick: now,
            water: 72.0,
            light: 68.0,
            health: 100.0,
            growth: 5.0,
            light_dir: 0,
            prune_left: 0,
            prune_right: 0,
            prune_top: 0,
            light_left_hours: 0.0,
            light_center_hours: 2.0,
            light_right_hours: 0.0,
            drought_stress: 0.0,
            wet_stress: 0.0,
        }
    }

    fn advance_to(&mut self, now: u64) {
        if now <= self.last_tick {
            return;
        }
        let dt_hours = (now - self.last_tick) as f32 / 3600.0;
        self.last_tick = now;

        self.water = (self.water - 1.45 * dt_hours).clamp(0.0, 100.0);
        self.light = (self.light - 0.38 * dt_hours).clamp(0.0, 100.0);

        let effective_light = self.light / 100.0 * dt_hours;
        match self.light_dir {
            -1 => self.light_left_hours += effective_light,
            1 => self.light_right_hours += effective_light,
            _ => self.light_center_hours += effective_light,
        }

        if self.water < 25.0 {
            self.drought_stress =
                (self.drought_stress + (25.0 - self.water) / 25.0 * dt_hours).min(120.0);
        } else {
            self.drought_stress = (self.drought_stress - 0.18 * dt_hours).max(0.0);
        }

        if self.water > 88.0 {
            self.wet_stress = (self.wet_stress + (self.water - 88.0) / 12.0 * dt_hours).min(120.0);
        } else {
            self.wet_stress = (self.wet_stress - 0.25 * dt_hours).max(0.0);
        }

        let water_score = triangular(self.water, 58.0, 55.0);
        let light_score = triangular(self.light, 70.0, 65.0);
        let stress_penalty = ((self.drought_stress + self.wet_stress) / 90.0).min(0.45);
        let comfort = (water_score * 0.58 + light_score * 0.42 - stress_penalty).clamp(0.0, 1.0);
        let target_health = 30.0 + comfort * 70.0;
        self.health += (target_health - self.health) * (0.07 * dt_hours).min(0.6);
        self.health = self.health.clamp(0.0, 100.0);

        if self.health > 32.0 && self.water > 10.0 {
            let rate = 0.86 * comfort * (self.health / 100.0);
            self.growth = (self.growth + rate * dt_hours).clamp(0.0, 100.0);
        }
    }

    fn age_secs(&self) -> u64 {
        now_secs().saturating_sub(self.born_at)
    }

    fn photo_bias(&self) -> f32 {
        let total = self.light_left_hours + self.light_center_hours + self.light_right_hours + 0.01;
        ((self.light_right_hours - self.light_left_hours) / total).clamp(-1.0, 1.0)
    }

    fn total_light_hours(&self) -> f32 {
        self.light_left_hours + self.light_center_hours + self.light_right_hours
    }

    fn serialize(&self) -> String {
        format!(
            "seed={}\nborn_at={}\nlast_tick={}\nwater={:.4}\nlight={:.4}\nhealth={:.4}\ngrowth={:.4}\nlight_dir={}\nprune_left={}\nprune_right={}\nprune_top={}\nlight_left_hours={:.4}\nlight_center_hours={:.4}\nlight_right_hours={:.4}\ndrought_stress={:.4}\nwet_stress={:.4}\n",
            self.seed,
            self.born_at,
            self.last_tick,
            self.water,
            self.light,
            self.health,
            self.growth,
            self.light_dir,
            self.prune_left,
            self.prune_right,
            self.prune_top,
            self.light_left_hours,
            self.light_center_hours,
            self.light_right_hours,
            self.drought_stress,
            self.wet_stress,
        )
    }

    fn parse(s: &str) -> Option<Self> {
        let mut st = State::new();
        for line in s.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k {
                "seed" => st.seed = v.parse().ok()?,
                "born_at" => st.born_at = v.parse().ok()?,
                "last_tick" => st.last_tick = v.parse().ok()?,
                "water" => st.water = v.parse().ok()?,
                "light" => st.light = v.parse().ok()?,
                "health" => st.health = v.parse().ok()?,
                "growth" => st.growth = v.parse().ok()?,
                "light_dir" => st.light_dir = v.parse().ok()?,
                "prune_left" => st.prune_left = v.parse().ok()?,
                "prune_right" => st.prune_right = v.parse().ok()?,
                "prune_top" => st.prune_top = v.parse().ok()?,
                "light_left_hours" => st.light_left_hours = v.parse().ok()?,
                "light_center_hours" => st.light_center_hours = v.parse().ok()?,
                "light_right_hours" => st.light_right_hours = v.parse().ok()?,
                "drought_stress" => st.drought_stress = v.parse().ok()?,
                "wet_stress" => st.wet_stress = v.parse().ok()?,
                _ => {}
            }
        }
        Some(st)
    }
}

fn triangular(x: f32, center: f32, width: f32) -> f32 {
    (1.0 - ((x - center).abs() / width.max(1.0))).clamp(0.0, 1.0)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn data_dir() -> PathBuf {
    if let Ok(x) = env::var("XDG_DATA_HOME") {
        PathBuf::from(x).join("bonzai")
    } else {
        PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".into())).join(".local/share/bonzai")
    }
}

fn runtime_dir() -> PathBuf {
    if let Ok(x) = env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(x).join("bonzai")
    } else {
        data_dir().join("run")
    }
}

fn state_path() -> PathBuf {
    data_dir().join("state.txt")
}
fn socket_path() -> PathBuf {
    runtime_dir().join("bonzai.sock")
}
fn pid_path() -> PathBuf {
    runtime_dir().join("bonzai.pid")
}

fn ensure_dirs() -> io::Result<()> {
    fs::create_dir_all(data_dir())?;
    fs::create_dir_all(runtime_dir())?;
    Ok(())
}

fn load_state() -> State {
    if let Ok(mut f) = File::open(state_path()) {
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            if let Some(mut st) = State::parse(&s) {
                st.advance_to(now_secs());
                return st;
            }
        }
    }
    State::new()
}

fn save_state(st: &State) -> io::Result<()> {
    ensure_dirs()?;
    let tmp = data_dir().join("state.tmp");
    fs::write(&tmp, st.serialize())?;
    fs::rename(tmp, state_path())
}

fn daemon_running() -> bool {
    send("ping")
        .map(|reply| reply.trim() == "pong")
        .unwrap_or(false)
}

fn send(cmd: &str) -> io::Result<String> {
    let mut stream = UnixStream::connect(socket_path())?;
    stream.write_all(cmd.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut out = String::new();
    stream.read_to_string(&mut out)?;
    Ok(out)
}

fn daemon_loop() -> io::Result<()> {
    ensure_dirs()?;
    let sock = socket_path();
    let _ = fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock)?;
    listener.set_nonblocking(true)?;
    fs::write(pid_path(), std::process::id().to_string())?;

    let mut st = load_state();
    save_state(&st)?;
    let mut should_stop = false;
    let mut last_save = now_secs();

    while !should_stop {
        st.advance_to(now_secs());
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut line = String::new();
                BufReader::new(stream.try_clone()?).read_line(&mut line)?;
                let response = handle_command(&mut st, line.trim(), &mut should_stop);
                save_state(&st)?;
                // A health probe or client may disconnect before reading the reply.
                // That must never terminate the authoritative daemon.
                let _ = stream.write_all(response.as_bytes());
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        if now_secs().saturating_sub(last_save) >= TICK_SECS {
            let _ = save_state(&st);
            last_save = now_secs();
        }
        thread::sleep(Duration::from_millis(120));
    }

    let _ = save_state(&st);
    let _ = fs::remove_file(sock);
    let _ = fs::remove_file(pid_path());
    Ok(())
}

fn handle_command(st: &mut State, cmd: &str, stop: &mut bool) -> String {
    st.advance_to(now_secs());
    let mut parts = cmd.split_whitespace();
    match parts.next().unwrap_or("") {
        "ping" => "pong\n".into(),
        "snapshot" => st.serialize(),
        "water" => {
            if st.water >= 96.0 {
                return format!("Water already high: {:.0}%\n", st.water);
            }
            st.water = (st.water + 22.0).min(100.0);
            if st.water > 94.0 {
                st.wet_stress = (st.wet_stress + 0.35).min(120.0);
            }
            format!("Water: {:.0}%\n", st.water)
        }
        "light" => {
            let d = parts.next().unwrap_or("center");
            st.light_dir = match d {
                "left" => -1,
                "right" => 1,
                _ => 0,
            };
            st.light = (st.light + 10.0).min(100.0);
            format!("Light moved {d}. Light: {:.0}%\n", st.light)
        }
        "prune" => {
            let side = parts.next().unwrap_or("top");
            match side {
                "left" => st.prune_left = st.prune_left.saturating_add(1),
                "right" => st.prune_right = st.prune_right.saturating_add(1),
                _ => st.prune_top = st.prune_top.saturating_add(1),
            }
            st.health = (st.health - 0.4).max(0.0);
            format!("Pruned {side}.\n")
        }
        "reset" => {
            *st = State::new();
            "new bonsai planted\n".into()
        }
        "stop" => {
            *stop = true;
            "stopping\n".into()
        }
        _ => "unknown command\n".into(),
    }
}

#[derive(Copy, Clone)]
struct Cell {
    ch: char,
    kind: u8,
}

struct Canvas {
    w: i32,
    h: i32,
    cells: Vec<Cell>,
}

impl Canvas {
    fn new(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            cells: vec![Cell { ch: ' ', kind: 0 }; (w * h) as usize],
        }
    }

    fn set(&mut self, x: i32, y: i32, ch: char, kind: u8) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.cells[(y * self.w + x) as usize] = Cell { ch, kind };
        }
    }

    fn get(&self, x: i32, y: i32) -> Cell {
        self.cells[(y * self.w + x) as usize]
    }
}

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() % (hi - lo) as u64) as i32
    }
    fn chance(&mut self, n: u64, d: u64) -> bool {
        self.next() % d < n
    }
}

fn draw_branch(
    c: &mut Canvas,
    rng: &mut Rng,
    mut x: i32,
    mut y: i32,
    mut dir: i32,
    mut life: i32,
    depth: u8,
    tropism: f32,
    vigor: f32,
    prune_left: u32,
    prune_right: u32,
) {
    let mut previous_x = x;
    while life > 0 && y > 2 {
        let photo_step = if tropism > 0.25 {
            1
        } else if tropism < -0.25 {
            -1
        } else {
            0
        };

        if rng.chance(if depth == 0 { 28 } else { 20 }, 100) {
            dir = (dir + photo_step).clamp(-2, 2);
        }
        if rng.chance(24, 100) {
            dir = (dir + rng.range(-1, 2)).clamp(-2, 2);
        }

        previous_x = x;
        if rng.chance(if depth == 0 { 34 } else { 62 }, 100) {
            x += dir.signum();
        }
        y -= 1;

        let glyph = if x < previous_x {
            '/'
        } else if x > previous_x {
            '\\'
        } else if depth == 0 && life > 8 {
            '|'
        } else if depth == 0 {
            'Y'
        } else {
            '|'
        };
        c.set(x, y, glyph, if depth == 0 && life > 8 { 5 } else { 1 });

        let branch_chance = ((8.0 + life as f32 * 0.55) * vigor).clamp(2.0, 20.0) as u64;
        if life > 5 && depth < 3 && rng.chance(branch_chance, 100) {
            let mut branch_dir = if rng.chance(1, 2) { -1 } else { 1 };
            if photo_step != 0 && rng.chance(58, 100) {
                branch_dir = photo_step;
            }
            let pruned = (branch_dir < 0
                && prune_left > 0
                && rng.chance(prune_left.min(8) as u64, 10))
                || (branch_dir > 0 && prune_right > 0 && rng.chance(prune_right.min(8) as u64, 10));

            if !pruned {
                let child_life = (life / 2 + rng.range(1, 5)).max(3);
                draw_branch(
                    c,
                    rng,
                    x,
                    y,
                    branch_dir,
                    child_life,
                    depth + 1,
                    tropism,
                    vigor,
                    prune_left,
                    prune_right,
                );
            }
        }

        life -= 1;
    }

    if depth > 0 {
        draw_leaf_cluster(c, rng, x, y, tropism, vigor, depth);
    }
}

fn draw_leaf_cluster(
    c: &mut Canvas,
    rng: &mut Rng,
    cx: i32,
    cy: i32,
    tropism: f32,
    vigor: f32,
    depth: u8,
) {
    let rx = (2 + depth as i32 + (vigor * 1.5) as i32).clamp(2, 5);
    let ry = (1 + (vigor * 1.5) as i32).clamp(1, 3);
    let light_shift = if tropism > 0.2 {
        1
    } else if tropism < -0.2 {
        -1
    } else {
        0
    };

    for yy in -ry..=ry {
        for xx in -rx..=rx {
            let nx = xx as f32 / rx as f32;
            let ny = yy as f32 / ry.max(1) as f32;
            if nx * nx + ny * ny > 1.0 {
                continue;
            }
            if !rng.chance((48.0 + vigor * 34.0) as u64, 100) {
                continue;
            }

            let pull = if light_shift != 0 && rng.chance(45, 100) {
                light_shift
            } else {
                0
            };
            let roll = rng.next() % 10;
            let (ch, kind) = match roll {
                0 => ('@', 3),
                1 | 2 => ('*', 4),
                _ => ('&', 2),
            };
            c.set(cx + xx + pull, cy + yy, ch, kind);
        }
    }
}

fn grow_tree(st: &State, w: i32, h: i32) -> Canvas {
    let mut c = Canvas::new(w, h);
    let base_y = h - 5;
    let base_x = w / 2;
    let mut rng = Rng::new(st.seed);

    let photo = st.photo_bias();
    let current_light_bias = st.light_dir as f32 * (st.light / 100.0) * 0.24;
    let tropism = (photo * 1.15 + current_light_bias).clamp(-1.25, 1.25);
    let stress = ((st.drought_stress + st.wet_stress) / 75.0).clamp(0.0, 0.72);
    let vigor = ((st.health / 100.0) * (1.0 - stress)).clamp(0.12, 1.0);
    let light_quality = (st.light / 100.0).clamp(0.0, 1.0);
    let etiolation = (1.0 - light_quality) * 0.55;

    let max_steps = 10 + (st.growth / 100.0 * 17.0) as i32 + (etiolation * 4.0) as i32;
    let top_penalty = (st.prune_top.min(7) as i32) * 2;
    let trunk_steps = (max_steps - top_penalty).max(8);

    draw_branch(
        &mut c,
        &mut rng,
        base_x,
        base_y,
        0,
        trunk_steps,
        0,
        tropism,
        vigor,
        st.prune_left,
        st.prune_right,
    );

    let crown_y = (base_y - trunk_steps + 2).max(3);
    let crown_shift = (tropism * 2.0).round() as i32;
    for offset in [-4, 0, 4] {
        if rng.chance((48.0 * vigor + 32.0) as u64, 100) {
            let jitter = rng.range(-1, 2);
            draw_leaf_cluster(
                &mut c,
                &mut rng,
                base_x + offset + crown_shift,
                crown_y + jitter,
                tropism,
                vigor,
                1,
            );
        }
    }

    let pot = [
        "       _________       ",
        "      /_________\\      ",
        "       \\       /       ",
        "        \\_____/        ",
    ];
    for (i, row) in pot.iter().enumerate() {
        let sx = base_x - row.chars().count() as i32 / 2;
        for (j, ch) in row.chars().enumerate() {
            if ch != ' ' {
                c.set(sx + j as i32, base_y + i as i32, ch, 7);
            }
        }
    }

    c
}

#[derive(Copy, Clone)]
enum SceneEffect {
    Water(u8),
    Light(i8, u8),
    Prune(i8, u8),
}

fn apply_effect(c: &mut Canvas, effect: SceneEffect) {
    let cx = c.w / 2;
    let base_y = c.h - 5;
    match effect {
        SceneEffect::Water(frame) => {
            let fall = frame as i32;
            for (dx, offset) in [(-6, 0), (-2, 2), (2, 1), (6, 3)] {
                let y = (3 + offset + fall * 3).min(base_y - 2);
                c.set(cx + dx, y, if frame < 2 { '.' } else { '|' }, 8);
            }
        }
        SceneEffect::Light(dir, frame) => {
            let x = match dir {
                -1 => 4,
                1 => c.w - 5,
                _ => cx,
            };
            c.set(x, 2, '*', 9);
            if frame > 0 {
                let step = if dir < 0 {
                    1
                } else if dir > 0 {
                    -1
                } else {
                    0
                };
                for i in 1..=4 {
                    c.set(x + step * i, 2 + i, '.', 9);
                }
            }
        }
        SceneEffect::Prune(side, frame) => {
            let x = cx + side as i32 * 9;
            let y = 9 + frame as i32;
            c.set(x, y, if frame == 0 { 'x' } else { '+' }, 10);
        }
    }
}

fn bar(v: f32, n: usize) -> String {
    let filled = ((v.clamp(0.0, 100.0) / 100.0) * n as f32).round() as usize;
    format!("{}{}", "━".repeat(filled), "─".repeat(n - filled))
}

fn mood(st: &State) -> &'static str {
    if st.health > 85.0 && st.water > 35.0 {
        "settled"
    } else if st.water < 20.0 {
        "thirsty"
    } else if st.light < 22.0 {
        "searching for light"
    } else if st.health < 45.0 {
        "recovering"
    } else {
        "quietly growing"
    }
}

fn terminal_size() -> (usize, usize) {
    let env_rows = env::var("LINES").ok().and_then(|v| v.parse::<usize>().ok());
    let env_cols = env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok());
    if let (Some(rows), Some(cols)) = (env_rows, env_cols) {
        if rows > 0 && cols > 0 {
            return (rows, cols);
        }
    }

    let tput_lines = Command::new("tput")
        .arg("lines")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<usize>().ok());
    let tput_cols = Command::new("tput")
        .arg("cols")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<usize>().ok());
    if let (Some(rows), Some(cols)) = (tput_lines, tput_cols) {
        if rows > 0 && cols > 0 {
            return (rows, cols);
        }
    }

    if let Ok(out) = Command::new("stty")
        .args(["-F", "/dev/tty", "size"])
        .output()
    {
        if out.status.success() {
            if let Ok(s) = String::from_utf8(out.stdout) {
                let mut parts = s.split_whitespace();
                if let (Some(rows), Some(cols)) = (parts.next(), parts.next()) {
                    if let (Ok(rows), Ok(cols)) = (rows.parse::<usize>(), cols.parse::<usize>()) {
                        if rows > 0 && cols > 0 {
                            return (rows, cols);
                        }
                    }
                }
            }
        }
    }

    if let Ok(out) = Command::new("stty").arg("size").output() {
        if out.status.success() {
            if let Ok(s) = String::from_utf8(out.stdout) {
                let mut parts = s.split_whitespace();
                if let (Some(rows), Some(cols)) = (parts.next(), parts.next()) {
                    if let (Ok(rows), Ok(cols)) = (rows.parse::<usize>(), cols.parse::<usize>()) {
                        if rows > 0 && cols > 0 {
                            return (rows, cols);
                        }
                    }
                }
            }
        }
    }

    (40, 100)
}

fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
            continue;
        }
        if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        width += 1;
    }
    width
}

fn center_line(line: &str, cols: usize) -> String {
    let width = visible_width(line);
    let pad = cols.saturating_sub(width) / 2;
    format!("{}{}", " ".repeat(pad), line)
}

fn render_scene(st: &State, effect: Option<SceneEffect>) -> String {
    let w = 58;
    let h = 25;
    let mut c = grow_tree(st, w, h);
    if let Some(effect) = effect {
        apply_effect(&mut c, effect);
    }
    let (rows, cols) = terminal_size();

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("{TITLE}bonzai{RESET}  {MUTED}{}{RESET}", mood(st)));
    lines.push(String::new());

    for y in 0..h {
        let mut line = String::new();
        for x in 0..w {
            let cell = c.get(x, y);
            let color = match cell.kind {
                1 => TRUNK,
                2 => LEAF,
                3 => LEAF_BRIGHT,
                4 => LEAF_DARK,
                5 => TRUNK_DARK,
                6 => SOIL,
                7 => POT,
                8 => WATER,
                9 => SUN,
                10 => HEALTH,
                _ => "",
            };
            if cell.kind == 0 {
                line.push(' ');
            } else {
                line.push_str(color);
                line.push(cell.ch);
                line.push_str(RESET);
            }
        }
        lines.push(line);
    }

    let dir = match st.light_dir {
        -1 => "←",
        1 => "→",
        _ => "↑",
    };
    lines.push(format!(
        "{WATER}water{RESET} {} {:>3.0}%   {SUN}light{RESET} {} {:>3.0}% {dir}   {HEALTH}health{RESET} {} {:>3.0}%",
        bar(st.water, 10), st.water,
        bar(st.light, 10), st.light,
        bar(st.health, 10), st.health,
    ));
    lines.push(format!(
        "{DIM}[w/r] water   [a/s/d] light   [j/k/l] prune   [h/?] help   [q] leave{RESET}"
    ));

    let top_pad = rows.saturating_sub(lines.len()) / 2;
    let mut out = String::new();
    out.push_str("\x1b[?25l\x1b[2J\x1b[H");
    out.push_str(&"\n".repeat(top_pad));
    for line in lines {
        out.push_str(&center_line(&line, cols));
        out.push('\n');
    }
    out
}

fn render(st: &State) -> String {
    render_scene(st, None)
}

fn human_age(s: u64) -> String {
    if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86400 {
        format!("{}h", s / 3600)
    } else {
        format!("{}d", s / 86400)
    }
}

fn get_snapshot() -> State {
    if let Ok(s) = send("snapshot") {
        State::parse(&s).unwrap_or_else(load_state)
    } else {
        load_state()
    }
}

fn animate_water(st: &State) -> io::Result<()> {
    for frame in 0..3 {
        print!("{}", render_scene(st, Some(SceneEffect::Water(frame))));
        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(90));
    }
    print!("{}", render(st));
    io::stdout().flush()?;
    Ok(())
}

fn animate_light(direction: &str, st: &State) -> io::Result<()> {
    let dir = match direction {
        "left" => -1,
        "right" => 1,
        _ => 0,
    };
    for frame in 0..2 {
        print!("{}", render_scene(st, Some(SceneEffect::Light(dir, frame))));
        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(100));
    }
    print!("{}", render(st));
    io::stdout().flush()?;
    Ok(())
}

fn animate_prune(side: &str, st: &State) -> io::Result<()> {
    let side = match side {
        "left" => -1,
        "right" => 1,
        _ => 0,
    };
    for frame in 0..2 {
        print!(
            "{}",
            render_scene(st, Some(SceneEffect::Prune(side, frame)))
        );
        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(90));
    }
    print!("{}", render(st));
    io::stdout().flush()?;
    Ok(())
}

fn watch_help() -> String {
    let (_, cols) = terminal_size();
    let lines = [
        format!("{TITLE}bonzai controls{RESET}"),
        String::new(),
        format!("{WATER}w / r{RESET}   water"),
        format!("{SUN}a / s / d{RESET}   light left / center / right"),
        format!("{HEALTH}j / k / l{RESET}   prune left / top / right"),
        format!("{TITLE}h / ?{RESET}   help"),
        format!("{TITLE}q{RESET}   leave"),
        String::new(),
        format!("{MUTED}Actions have a short cooldown so one held key cannot flood the tree.{RESET}"),
        format!("{MUTED}Light history affects future growth. Water and pruning affect vigor and structure.{RESET}"),
        String::new(),
        format!("{DIM}press any key to return{RESET}"),
    ];

    let mut out = String::new();
    out.push_str("\x1b[2J\x1b[H");
    let (rows, _) = terminal_size();
    out.push_str(&"\n".repeat(rows.saturating_sub(lines.len()) / 2));
    for line in lines {
        out.push_str(&center_line(&line, cols));
        out.push('\n');
    }
    out
}

fn drain_input() {
    let _ = Command::new("stty")
        .args(["min", "0", "time", "0"])
        .status();
    let mut buf = [0u8; 64];
    loop {
        match io::stdin().read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
    }
    let _ = Command::new("stty")
        .args(["min", "0", "time", "1"])
        .status();
}

fn watch() -> io::Result<()> {
    let _ = Command::new("stty")
        .args(["-echo", "-icanon", "min", "0", "time", "1"])
        .status();

    let result = (|| {
        let mut last_action = Instant::now() - Duration::from_millis(ACTION_COOLDOWN_MS);
        loop {
            let st = get_snapshot();
            print!("{}", render(&st));
            io::stdout().flush()?;

            let mut buf = [0u8; 1];
            if io::stdin().read(&mut buf).unwrap_or(0) > 0 {
                let key = buf[0] as char;
                if key == 'q' {
                    break;
                }

                if matches!(key, 'w' | 'r' | 'a' | 's' | 'd' | 'j' | 'k' | 'l')
                    && last_action.elapsed() < Duration::from_millis(ACTION_COOLDOWN_MS)
                {
                    let (_, cols) = terminal_size();
                    let msg = format!("{MUTED}give the tree a second{RESET}");
                    print!("\x1b[2J\x1b[H\n{}\n", center_line(&msg, cols));
                    io::stdout().flush()?;
                    drain_input();
                    thread::sleep(Duration::from_millis(180));
                    continue;
                }

                match key {
                    'w' | 'r' => {
                        let _ = send("water")?;
                        let after = get_snapshot();
                        animate_water(&after)?;
                        last_action = Instant::now();
                        drain_input();
                    }
                    'a' => {
                        let _ = send("light left")?;
                        let after = get_snapshot();
                        animate_light("left", &after)?;
                        last_action = Instant::now();
                        drain_input();
                    }
                    's' => {
                        let _ = send("light center")?;
                        let after = get_snapshot();
                        animate_light("center", &after)?;
                        last_action = Instant::now();
                        drain_input();
                    }
                    'd' => {
                        let _ = send("light right")?;
                        let after = get_snapshot();
                        animate_light("right", &after)?;
                        last_action = Instant::now();
                        drain_input();
                    }
                    'j' => {
                        let _ = send("prune left")?;
                        let after = get_snapshot();
                        animate_prune("left", &after)?;
                        last_action = Instant::now();
                        drain_input();
                    }
                    'k' => {
                        let _ = send("prune top")?;
                        let after = get_snapshot();
                        animate_prune("top", &after)?;
                        last_action = Instant::now();
                        drain_input();
                    }
                    'l' => {
                        let _ = send("prune right")?;
                        let after = get_snapshot();
                        animate_prune("right", &after)?;
                        last_action = Instant::now();
                        drain_input();
                    }
                    'h' | '?' => {
                        print!("{}", watch_help());
                        io::stdout().flush()?;
                        let _ = Command::new("stty")
                            .args(["min", "1", "time", "0"])
                            .status();
                        let _ = io::stdin().read(&mut buf);
                        let _ = Command::new("stty")
                            .args(["min", "0", "time", "1"])
                            .status();
                        drain_input();
                    }
                    _ => {}
                }
            }

            thread::sleep(Duration::from_millis(90));
        }
        Ok(())
    })();

    let _ = Command::new("stty").arg("sane").status();
    print!("\x1b[?25h\x1b[0m\n");
    result
}

fn start_daemon() -> io::Result<()> {
    ensure_dirs()?;
    if daemon_running() {
        println!("bonzai is already growing");
        return Ok(());
    }

    // Clean up stale runtime files left by an interrupted or upgraded daemon.
    let _ = fs::remove_file(socket_path());
    let _ = fs::remove_file(pid_path());

    if !state_path().exists() {
        save_state(&State::new())?;
    }
    let exe = env::current_exe()?;
    Command::new(exe)
        .arg("daemon-run")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    for _ in 0..40 {
        thread::sleep(Duration::from_millis(50));
        if daemon_running() {
            println!("bonzai started");
            return Ok(());
        }
    }
    Err(io::Error::other(
        "daemon failed to start; try `bonzai daemon-run` for diagnostics",
    ))
}

fn self_update() -> io::Result<()> {
    const INSTALL_URL: &str =
        "https://raw.githubusercontent.com/Nicolas25vlad/bonzai/main/install.sh";
    let was_running = daemon_running();
    if was_running {
        let _ = send("stop");
        thread::sleep(Duration::from_millis(180));
    }

    println!("checking for the latest bonzai...");
    let download = Command::new("curl").args(["-fsSL", INSTALL_URL]).output()?;
    if !download.status.success() {
        return Err(io::Error::other("failed to download the installer"));
    }

    let mut child = Command::new("bash").stdin(Stdio::piped()).spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(&download.stdout)?;
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(io::Error::other(
            "update failed while building the new version",
        ));
    }

    if was_running {
        let exe = env::current_exe()?;
        let status = Command::new(exe).arg("start").status()?;
        if !status.success() {
            return Err(io::Error::other(
                "updated successfully, but the daemon could not restart",
            ));
        }
    }

    println!("bonzai is up to date");
    Ok(())
}

fn print_status(st: &State) {
    println!(
        "{TITLE}bonzai{RESET}  {}  ·  {}",
        human_age(st.age_secs()),
        mood(st)
    );
    println!("{TITLE}growth{RESET} {:>5.1}%", st.growth);
    println!(
        "{WATER}water {RESET} {} {:>5.1}%",
        bar(st.water, 12),
        st.water
    );
    println!(
        "{SUN}light {RESET} {} {:>5.1}%",
        bar(st.light, 12),
        st.light
    );
    println!(
        "{HEALTH}health{RESET} {} {:>5.1}%",
        bar(st.health, 12),
        st.health
    );
    println!(
        "{SOIL}memory{RESET} L {:.1}h  C {:.1}h  R {:.1}h",
        st.light_left_hours, st.light_center_hours, st.light_right_hours
    );
    println!("{DIM}Tip: `bonzai watch` for a quiet break.{RESET}");
}

fn usage() {
    println!(
        "{TITLE}bonzai {VERSION}{RESET}  ·  a living bonsai for your terminal\n\
{DIM}Grow it slowly. Shape it deliberately. Then get back to coding.{RESET}\n\n\
{TITLE}USAGE{RESET}\n  bonzai <command> [options]\n\n\
{TITLE}CARE{RESET}\n  {WATER}water{RESET}                    Water the tree\n  {SUN}light{RESET} left|center|right  Move its light source\n  {HEALTH}prune{RESET} left|top|right     Shape future growth\n\n\
{TITLE}OBSERVE{RESET}\n  watch                    Open the interactive cozy view\n  show                     Render the current tree once\n  status                   Show health, growth and light memory\n\n\
{TITLE}LIFECYCLE{RESET}\n  init                     Plant a new tree\n  start                    Start the background daemon\n  stop                     Stop the daemon\n  reset                    Plant a completely new tree\n\n\
{TITLE}INFO{RESET}\n  help, -h, --help         Show this help\n  version, -V, --version   Print the version\n\n\
{DIM}Interactive keys{RESET}\n  w/r water   a/s/d light   j/k/l prune   h/? help   q leave\n\n\
{DIM}Growth model: directional light memory → phototropism; low light → longer shoots;\nwater stress → fewer leaves and branches; pruning → persistent structural pressure.{RESET}\n\n\
{DIM}Data: ~/.local/share/bonzai or $XDG_DATA_HOME/bonzai{RESET}\n"
    );
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    match cmd {
        "daemon-run" => daemon_loop(),
        "init" | "reset" => {
            ensure_dirs()?;
            if daemon_running() {
                print!("{}", send("reset")?);
            } else {
                let st = State::new();
                save_state(&st)?;
                println!("new bonsai planted");
            }
            Ok(())
        }
        "start" => start_daemon(),
        "stop" => {
            match send("stop") {
                Ok(_) => println!("bonzai stopped"),
                Err(_) => println!("bonzai is not running"),
            }
            Ok(())
        }
        "status" => {
            let st = get_snapshot();
            print_status(&st);
            Ok(())
        }
        "show" => {
            let st = get_snapshot();
            print!("{}", render(&st));
            print!("\x1b[?25h");
            Ok(())
        }
        "watch" => {
            if !daemon_running() {
                start_daemon()?;
            }
            watch()
        }
        "water" => {
            if !daemon_running() {
                start_daemon()?;
            }
            print!("{}", send("water")?);
            Ok(())
        }
        "light" => {
            if !daemon_running() {
                start_daemon()?;
            }
            let d = args.get(2).map(String::as_str).unwrap_or("center");
            print!("{}", send(&format!("light {d}"))?);
            Ok(())
        }
        "prune" => {
            if !daemon_running() {
                start_daemon()?;
            }
            let d = args.get(2).map(String::as_str).unwrap_or("top");
            print!("{}", send(&format!("prune {d}"))?);
            Ok(())
        }
        "help" | "--help" | "-h" => {
            usage();
            Ok(())
        }
        "update" => self_update(),
        "version" | "--version" | "-V" => {
            println!("bonzai {VERSION}");
            Ok(())
        }
        _ => {
            eprintln!("unknown command: {cmd}\n");
            usage();
            Ok(())
        }
    }
}
