use std::{
    env,
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const VERSION: &str = "0.2.0";
const TICK_SECS: u64 = 30;

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const TITLE: &str = "\x1b[38;5;180m";
const TRUNK: &str = "\x1b[38;5;137m";
const LEAF: &str = "\x1b[38;5;108m";
const LEAF_BRIGHT: &str = "\x1b[38;5;151m";
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
            self.drought_stress = (self.drought_stress + (25.0 - self.water) / 25.0 * dt_hours).min(120.0);
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
            let Some((k, v)) = line.split_once('=') else { continue };
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
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
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

fn state_path() -> PathBuf { data_dir().join("state.txt") }
fn socket_path() -> PathBuf { runtime_dir().join("bonzai.sock") }
fn pid_path() -> PathBuf { runtime_dir().join("bonzai.pid") }

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

fn daemon_running() -> bool { UnixStream::connect(socket_path()).is_ok() }

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
                stream.write_all(response.as_bytes())?;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        if now_secs().saturating_sub(last_save) >= TICK_SECS {
            let _ = save_state(&st);
            last_save = now_secs();
        }
        thread::sleep(Duration::from_millis(200));
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
            st.water = (st.water + 24.0).min(100.0);
            if st.water > 94.0 {
                st.wet_stress = (st.wet_stress + 0.8).min(120.0);
            }
            format!("💧 Water: {:.0}%\n", st.water)
        }
        "light" => {
            let d = parts.next().unwrap_or("center");
            st.light_dir = match d {
                "left" => -1,
                "right" => 1,
                _ => 0,
            };
            st.light = (st.light + 16.0).min(100.0);
            format!("☀ Light moved {d}. Light: {:.0}%\n", st.light)
        }
        "prune" => {
            let side = parts.next().unwrap_or("top");
            match side {
                "left" => st.prune_left += 1,
                "right" => st.prune_right += 1,
                _ => st.prune_top += 1,
            }
            st.health = (st.health - 0.6).max(0.0);
            format!("✂ Pruned {side}.\n")
        }
        "reset" => {
            *st = State::new();
            "new bonsai planted 🌱\n".into()
        }
        "stop" => {
            *stop = true;
            "stopping\n".into()
        }
        _ => "unknown command\n".into(),
    }
}

#[derive(Copy, Clone)]
struct Cell { ch: char, kind: u8 }

struct Canvas { w: i32, h: i32, cells: Vec<Cell> }
impl Canvas {
    fn new(w: i32, h: i32) -> Self {
        Self { w, h, cells: vec![Cell { ch: ' ', kind: 0 }; (w * h) as usize] }
    }
    fn set(&mut self, x: i32, y: i32, ch: char, kind: u8) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.cells[(y * self.w + x) as usize] = Cell { ch, kind };
        }
    }
    fn get(&self, x: i32, y: i32) -> Cell { self.cells[(y * self.w + x) as usize] }
}

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Self(seed.max(1)) }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo { return lo; }
        lo + (self.next() % (hi - lo) as u64) as i32
    }
    fn chance(&mut self, n: u64, d: u64) -> bool { self.next() % d < n }
}

fn grow_tree(st: &State, w: i32, h: i32) -> Canvas {
    let mut c = Canvas::new(w, h);
    let base_y = h - 5;
    let base_x = w / 2;
    let mut rng = Rng::new(st.seed);

    // A tiny biological model, intentionally not a full plant simulator:
    // - accumulated directional light drives phototropism
    // - low light causes longer internodes (etiolation)
    // - drought/wet stress suppresses foliage and branching
    // - pruning suppresses future walkers on that side
    let photo = st.photo_bias();
    let current_light_bias = st.light_dir as f32 * (st.light / 100.0) * 0.35;
    let tropism = (photo * 1.5 + current_light_bias).clamp(-1.6, 1.6);
    let light_quality = (st.light / 100.0).clamp(0.0, 1.0);
    let etiolation = (1.0 - light_quality) * 0.75;
    let stress = ((st.drought_stress + st.wet_stress) / 70.0).clamp(0.0, 0.75);
    let vigor = ((st.health / 100.0) * (1.0 - stress)).clamp(0.1, 1.0);

    let max_steps = 8 + (st.growth / 100.0 * 21.0) as i32 + (etiolation * 5.0) as i32;
    let top_penalty = (st.prune_top.min(8) as i32) * 2;
    let trunk_steps = (max_steps - top_penalty).max(8);
    let prune_balance = st.prune_right as i32 - st.prune_left as i32;
    let mut walkers = vec![(base_x, base_y, 0i32, trunk_steps, 0u8)];

    while let Some((mut x, mut y, mut dx, mut life, kind)) = walkers.pop() {
        let start_life = life;
        while life > 0 && y > 1 {
            let age = start_life - life;
            let photo_step = if tropism > 0.30 { 1 } else if tropism < -0.30 { -1 } else { 0 };
            let shape_bias = if kind == 0 {
                photo_step + prune_balance.signum()
            } else {
                dx.signum() + if rng.chance((tropism.abs() * 20.0) as u64, 100) { photo_step } else { 0 }
            };
            let jitter = rng.range(-1, 2);
            dx = (dx + jitter + shape_bias).clamp(-2, 2);

            // Low-light shoots stretch: they climb more often before branching.
            let climb_chance = (66.0 + etiolation * 24.0) as u64;
            if rng.chance(climb_chance.min(94), 100) { y -= 1; }
            if rng.chance((35.0 + etiolation * 15.0) as u64, 100) { x += dx.signum(); }

            let glyph = if dx < 0 { '/' } else if dx > 0 { '\\' } else { '|' };
            c.set(x, y, glyph, 1);

            let branch_base = (7.0 + st.growth * 0.12) * vigor * (1.0 - etiolation * 0.25);
            let can_branch = life > 6 && age > 3;
            if can_branch && rng.chance(branch_base.max(2.0) as u64, 100) {
                let light_favored_right = tropism > 0.15;
                let dir = if rng.chance(58, 100) {
                    if light_favored_right { 1 } else if tropism < -0.15 { -1 } else if rng.chance(1, 2) { -1 } else { 1 }
                } else if rng.chance(1, 2) { -1 } else { 1 };

                let pruned = (dir < 0 && st.prune_left > 0 && rng.chance(st.prune_left.min(8) as u64, 10))
                    || (dir > 0 && st.prune_right > 0 && rng.chance(st.prune_right.min(8) as u64, 10));
                if !pruned {
                    walkers.push((x, y, dir, (life / 2 + rng.range(2, 7)).max(4), 1));
                }
            }
            life -= 1;
        }

        if st.health > 16.0 {
            let base_density = 2 + (st.health / 19.0) as i32;
            let density = ((base_density as f32) * vigor * (0.55 + light_quality * 0.55)).round().clamp(1.0, 8.0) as i32;
            for _ in 0..density * 3 {
                let mut fx = x;
                let mut fy = y;
                let steps = rng.range(2, 6);
                for _ in 0..steps {
                    // Leaves preferentially occupy the illuminated side.
                    let toward_light = if tropism > 0.12 { 1 } else if tropism < -0.12 { -1 } else { 0 };
                    let leaf_pull = if toward_light != 0 && rng.chance(62, 100) { toward_light } else { rng.range(-1, 2) };
                    fx += leaf_pull;
                    fy += rng.range(-1, 2);
                    let leaf = match rng.next() % 6 { 0 => '&', 1 => '*', 2 => '+', 3 => '%', _ => '·' };
                    c.set(fx, fy, leaf, if rng.chance(1, 4) { 4 } else { 2 });
                }
            }
        }
    }

    let pot = ["   .-~~~~-.   ", "  /        \\  ", " /__________\\ ", "   \\______/   "];
    for (i, row) in pot.iter().enumerate() {
        let sx = base_x - row.chars().count() as i32 / 2;
        for (j, ch) in row.chars().enumerate() {
            if ch != ' ' { c.set(sx + j as i32, base_y + 1 + i as i32, ch, 3); }
        }
    }
    c
}

fn bar(v: f32, n: usize) -> String {
    let filled = ((v.clamp(0.0, 100.0) / 100.0) * n as f32).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(n - filled))
}

fn mood(st: &State) -> &'static str {
    if st.health > 85.0 && st.water > 35.0 { "settled" }
    else if st.water < 20.0 { "thirsty" }
    else if st.light < 22.0 { "searching for light" }
    else if st.health < 45.0 { "recovering" }
    else { "quietly growing" }
}

fn render(st: &State) -> String {
    let w = 64;
    let h = 28;
    let c = grow_tree(st, w, h);
    let mut out = String::new();
    out.push_str("\x1b[?25l\x1b[2J\x1b[H");
    out.push_str(&format!("{TITLE}                 bonzai  ·  a quiet place to grow{RESET}\n"));
    out.push_str(&format!("{MUTED}                         {}{RESET}\n\n", mood(st)));

    for y in 0..h {
        for x in 0..w {
            let cell = c.get(x, y);
            let color = match cell.kind {
                1 => TRUNK,
                2 => LEAF,
                3 => POT,
                4 => LEAF_BRIGHT,
                _ => "",
            };
            if cell.kind == 0 { out.push(' '); }
            else {
                out.push_str(color);
                out.push(cell.ch);
                out.push_str(RESET);
            }
        }
        out.push('\n');
    }

    let dir = match st.light_dir { -1 => "←", 1 => "→", _ => "↑" };
    out.push_str(&format!(
        "\n  {TITLE}age{RESET} {}   {TITLE}growth{RESET} {:>5.1}%   {TITLE}light memory{RESET} {:>5.1}h\n  {WATER}💧 water{RESET}  {} {:>5.1}%   {SUN}☀ light{RESET}  {} {:>5.1}% {dir}   {HEALTH}♥ health{RESET} {} {:>5.1}%\n\n  {DIM}[w/r]{RESET} water   {DIM}[a/s/d]{RESET} move light   {DIM}[j/k/l]{RESET} prune   {DIM}[h/?]{RESET} help   {DIM}[q]{RESET} leave\n  {MUTED}Your tree keeps growing while you code. No streaks. No punishment.{RESET}\n",
        human_age(st.age_secs()), st.growth, st.total_light_hours(),
        bar(st.water, 10), st.water,
        bar(st.light, 10), st.light,
        bar(st.health, 10), st.health,
    ));
    out
}

fn human_age(s: u64) -> String {
    if s < 3600 { format!("{}m", s / 60) }
    else if s < 86400 { format!("{}h", s / 3600) }
    else { format!("{}d", s / 86400) }
}

fn get_snapshot() -> State {
    if let Ok(s) = send("snapshot") { State::parse(&s).unwrap_or_else(load_state) }
    else { load_state() }
}

fn frame_with_overlay(st: &State, overlay: &[&str]) -> String {
    let mut s = render(st);
    s.push_str("\n");
    for line in overlay { s.push_str(line); s.push('\n'); }
    s
}

fn animate_water() -> io::Result<()> {
    let frames: &[&[&str]] = &[
        &["                         💧", "                         │"],
        &["                       💧 💧", "                        │ │"],
        &["                     💧  💧  💧", "                      │   │"],
        &["                    ·  ·  ·  ·", "                     moistening soil..."],
    ];
    for f in frames {
        let st = get_snapshot();
        print!("{}", frame_with_overlay(&st, f));
        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(180));
    }
    let _ = send("water");
    Ok(())
}

fn animate_light(direction: &str) -> io::Result<()> {
    let arrow = match direction { "left" => "☀  → → →", "right" => "← ← ←  ☀", _ => "      ☀\n      ↓" };
    let phases = ["soft morning light", "warming the leaves", "the crown turns, almost imperceptibly"];
    for phase in phases {
        let st = get_snapshot();
        let lines = [arrow, phase];
        print!("{}", frame_with_overlay(&st, &lines));
        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(190));
    }
    let _ = send(&format!("light {direction}"));
    Ok(())
}

fn animate_prune(side: &str) -> io::Result<()> {
    let frames = [
        "                         ✂",
        "                       ✂  ·",
        "                    a clean cut",
    ];
    for frame in frames {
        let st = get_snapshot();
        let lines = [frame];
        print!("{}", frame_with_overlay(&st, &lines));
        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(170));
    }
    let _ = send(&format!("prune {side}"));
    Ok(())
}

fn watch_help() -> String {
    format!(
        "\x1b[2J\x1b[H{TITLE}                         bonzai controls{RESET}\n\n\
  {WATER}w / r{RESET}   water your bonsai  {DIM}(r = a little rain break){RESET}\n\n\
  {SUN}a / s / d{RESET}\n\
          move the light left / center / right\n\n\
  {HEALTH}j / k / l{RESET}\n\
          prune left / top / right\n\n\
  {TITLE}h or ?{RESET}  show this guide\n\
  {TITLE}q{RESET}       return to your shell\n\n\
  {DIM}Light has memory. Keep it on one side and new growth gradually leans toward it.\n\
  Low light stretches shoots. Drought reduces foliage. Pruning changes future branching.\n\n\
  Nothing here has a streak. The tree waits for you.{RESET}\n\n\
  press any key to return\n"
    )
}

fn watch() -> io::Result<()> {
    let _ = Command::new("stty").arg("-echo").arg("-icanon").status();
    let result = (|| {
        loop {
            let st = get_snapshot();
            print!("{}", render(&st));
            io::stdout().flush()?;
            let mut buf = [0u8; 1];
            let _ = Command::new("stty").args(["min", "0", "time", "1"]).status();
            if io::stdin().read(&mut buf).unwrap_or(0) > 0 {
                match buf[0] as char {
                    'q' => break,
                    'w' | 'r' => animate_water()?,
                    'a' => animate_light("left")?,
                    's' => animate_light("center")?,
                    'd' => animate_light("right")?,
                    'j' => animate_prune("left")?,
                    'k' => animate_prune("top")?,
                    'l' => animate_prune("right")?,
                    'h' | '?' => {
                        print!("{}", watch_help());
                        io::stdout().flush()?;
                        let _ = Command::new("stty").args(["min", "1", "time", "0"]).status();
                        let _ = io::stdin().read(&mut buf);
                    }
                    _ => {}
                }
            }
            thread::sleep(Duration::from_millis(130));
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
        println!("bonzai is already growing 🌱");
        return Ok(());
    }
    if !state_path().exists() { save_state(&State::new())?; }
    let exe = env::current_exe()?;
    Command::new(exe)
        .arg("daemon-run")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(50));
        if daemon_running() {
            println!("bonzai started 🌱");
            return Ok(());
        }
    }
    Err(io::Error::other("daemon failed to start"))
}

fn print_status(st: &State) {
    println!("{TITLE}bonzai 🌱{RESET}  {}  ·  {}", human_age(st.age_secs()), mood(st));
    println!("{TITLE}growth{RESET} {:>5.1}%", st.growth);
    println!("{WATER}water {RESET} {} {:>5.1}%", bar(st.water, 12), st.water);
    println!("{SUN}light {RESET} {} {:>5.1}%", bar(st.light, 12), st.light);
    println!("{HEALTH}health{RESET} {} {:>5.1}%", bar(st.health, 12), st.health);
    println!("{SOIL}memory{RESET} L {:.1}h  C {:.1}h  R {:.1}h", st.light_left_hours, st.light_center_hours, st.light_right_hours);
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
            if daemon_running() { print!("{}", send("reset")?); }
            else {
                let st = State::new();
                save_state(&st)?;
                println!("new bonsai planted 🌱");
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
            if !daemon_running() { let _ = start_daemon(); }
            watch()
        }
        "water" => {
            if !daemon_running() { start_daemon()?; }
            print!("{}", send("water")?);
            Ok(())
        }
        "light" => {
            if !daemon_running() { start_daemon()?; }
            let d = args.get(2).map(String::as_str).unwrap_or("center");
            print!("{}", send(&format!("light {d}"))?);
            Ok(())
        }
        "prune" => {
            if !daemon_running() { start_daemon()?; }
            let d = args.get(2).map(String::as_str).unwrap_or("top");
            print!("{}", send(&format!("prune {d}"))?);
            Ok(())
        }
        "help" | "--help" | "-h" => {
            usage();
            Ok(())
        }
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
