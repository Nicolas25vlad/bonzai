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

const VERSION: &str = "0.2.1";
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
            self.wet_stress =
                (self.wet_stress + (self.water - 88.0) / 12.0 * dt_hours).min(120.0);
        } else {
            self.wet_stress = (self.wet_stress - 0.25 * dt_hours).max(0.0);
        }

        let water_score = triangular(self.water, 58.0, 55.0);
        let light_score = triangular(self.light, 70.0, 65.0);
        let stress_penalty = ((self.drought_stress + self.wet_stress) / 90.0).min(0.45);
        let comfort =
            (water_score * 0.58 + light_score * 0.42 - stress_penalty).clamp(0.0, 1.0);
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
        PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join(".local/share/bonzai")
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
    UnixStream::connect(socket_path()).is_ok()
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
                stream.write_all(response.as_bytes())?;
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
    mut dx: i32,
    length: i32,
    depth: u8,
    tropism: f32,
    vigor: f32,
    prune_left: u32,
    prune_right: u32,
) {
    if depth > 4 || length < 2 {
        return;
    }

    let light_step = if tropism > 0.22 {
        1
    } else if tropism < -0.22 {
        -1
    } else {
        0
    };

    for step in 0..length {
        y -= 1;
        let drift_chance = if depth == 0 { 28 } else { 44 };
        if rng.chance(drift_chance, 100) {
            let jitter = rng.range(-1, 2);
            dx = (dx + jitter + if rng.chance(26, 100) { light_step } else { 0 }).clamp(-1, 1);
        }
        if dx != 0 && rng.chance(54, 100) {
            x += dx;
        }

        let glyph = match dx {
            -1 => '/',
            1 => '\\',
            _ => if depth == 0 && step < 4 { '┃' } else { '│' },
        };
        c.set(x, y, glyph, if depth == 0 && step < 4 { 5 } else { 1 });

        let branch_window = step > 2 && step < length - 2;
        let branch_chance = ((9.0 + depth as f32 * 2.0) * vigor).clamp(3.0, 18.0) as u64;
        if branch_window && depth < 3 && rng.chance(branch_chance, 100) {
            let preferred = if light_step != 0 && rng.chance(60, 100) {
                light_step
            } else if rng.chance(1, 2) {
                -1
            } else {
                1
            };
            let suppressed = (preferred < 0 && prune_left > 0 && rng.chance(prune_left.min(8) as u64, 11))
                || (preferred > 0 && prune_right > 0 && rng.chance(prune_right.min(8) as u64, 11));
            if !suppressed {
                let child_len = ((length - step) / 2 + rng.range(2, 6)).max(3);
                draw_branch(
                    c,
                    rng,
                    x,
                    y,
                    preferred,
                    child_len,
                    depth + 1,
                    tropism,
                    vigor,
                    prune_left,
                    prune_right,
                );
            }
        }
    }

    if depth > 0 {
        draw_leaf_cluster(c, rng, x, y, tropism, vigor, depth);
    }
}

fn draw_leaf_cluster(
    c: &mut Canvas,
    rng: &mut Rng,
    x: i32,
    y: i32,
    tropism: f32,
    vigor: f32,
    depth: u8,
) {
    let radius_x = (2 + (vigor * 3.0) as i32 - (depth as i32 / 2)).clamp(2, 5);
    let radius_y = (1 + (vigor * 2.0) as i32).clamp(1, 3);
    let shift = if tropism > 0.18 {
        1
    } else if tropism < -0.18 {
        -1
    } else {
        0
    };

    for yy in -radius_y..=radius_y {
        for xx in -radius_x..=radius_x {
            let ellipse = (xx * xx * 4) + (yy * yy * 9);
            let limit = radius_x * radius_x * 4;
            if ellipse > limit || !rng.chance((55.0 + vigor * 25.0) as u64, 100) {
                continue;
            }
            let px = x + xx + if rng.chance(58, 100) { shift } else { 0 };
            let py = y + yy;
            let glyph = match rng.next() % 7 {
                0 => '❧',
                1 => '•',
                2 => '·',
                3 => '✦',
                _ => '●',
            };
            let kind = match rng.next() % 6 {
                0 => 4,
                1 => 6,
                _ => 2,
            };
            c.set(px, py, glyph, kind);
        }
    }
}

fn grow_tree(st: &State, w: i32, h: i32) -> Canvas {
    let mut c = Canvas::new(w, h);
    let base_y = h - 6;
    let base_x = w / 2;
    let mut rng = Rng::new(st.seed);

    let photo = st.photo_bias();
    let current_light_bias = st.light_dir as f32 * (st.light / 100.0) * 0.28;
    let tropism = (photo * 1.35 + current_light_bias).clamp(-1.4, 1.4);
    let light_quality = (st.light / 100.0).clamp(0.0, 1.0);
    let stress = ((st.drought_stress + st.wet_stress) / 80.0).clamp(0.0, 0.72);
    let vigor = ((st.health / 100.0) * (1.0 - stress) * (0.70 + light_quality * 0.30))
        .clamp(0.15, 1.0);
    let etiolation = (1.0 - light_quality) * 0.55;
    let max_steps = 9 + (st.growth / 100.0 * 18.0) as i32 + (etiolation * 5.0) as i32;
    let top_penalty = st.prune_top.min(7) as i32;
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
    for offset in [-5, 0, 5] {
        if rng.chance((62.0 * vigor + 22.0) as u64, 100) {
            draw_leaf_cluster(
                &mut c,
                &mut rng,
                base_x + offset + crown_shift,
                crown_y + rng.range(-1, 3),
                tropism,
                vigor,
                1,
            );
        }
    }

    let pot = [
        "      ╭──────╮      ",
        "     ╱        ╲     ",
        "    ╱  ░░░░░░  ╲    ",
        "    ╰──────────╯    ",
        "      ╰──────╯      ",
    ];
    for (i, row) in pot.iter().enumerate() {
        let sx = base_x - row.chars().count() as i32 / 2;
        for (j, ch) in row.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            c.set(
                sx + j as i32,
                base_y + 1 + i as i32,
                ch,
                if ch == '░' { 7 } else { 3 },
            );
        }
    }

    c
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
        "reaching for light"
    } else if st.health < 45.0 {
        "recovering"
    } else {
        "quietly growing"
    }
}

fn terminal_size() -> (usize, usize) {
    if let Ok(output) = Command::new("stty").arg("size").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut parts = text.split_whitespace();
            if let (Some(rows), Some(cols)) = (parts.next(), parts.next()) {
                if let (Ok(r), Ok(c)) = (rows.parse::<usize>(), cols.parse::<usize>()) {
                    return (c.max(40), r.max(20));
                }
            }
        }
    }
    (80, 40)
}

fn ansi_visible_width(s: &str) -> usize {
    let mut width = 0usize;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            i += 1;
            if i < bytes.len() && bytes[i] == b'[' {
                i += 1;
                while i < bytes.len() && !(bytes[i] >= 0x40 && bytes[i] <= 0x7e) {
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                }
            }
            continue;
        }
        if let Some(ch) = s[i..].chars().next() {
            width += 1;
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    width
}

fn centered_line(s: &str, cols: usize) -> String {
    let width = ansi_visible_width(s);
    let pad = cols.saturating_sub(width) / 2;
    format!("{}{}", " ".repeat(pad), s)
}

fn tree_lines(st: &State) -> Vec<String> {
    let c = grow_tree(st, 54, 26);
    let mut lines = Vec::new();
    for y in 0..c.h {
        let mut line = String::new();
        for x in 0..c.w {
            let cell = c.get(x, y);
            if cell.kind == 0 {
                line.push(' ');
                continue;
            }
            let color = match cell.kind {
                1 => TRUNK,
                2 => LEAF,
                3 => POT,
                4 => LEAF_BRIGHT,
                5 => TRUNK_DARK,
                6 => LEAF_DARK,
                7 => SOIL,
                _ => "",
            };
            line.push_str(color);
            line.push(cell.ch);
            line.push_str(RESET);
        }
        lines.push(line.trim_end().to_string());
    }
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    lines
}

fn render(st: &State, notice: Option<&str>) -> String {
    let (cols, rows) = terminal_size();
    let mut content = Vec::<String>::new();
    content.push(format!("{TITLE}bonzai{RESET}  {DIM}·  a quiet place to grow{RESET}"));
    content.push(format!("{MUTED}{}{RESET}", mood(st)));
    content.push(String::new());
    content.extend(tree_lines(st));
    content.push(String::new());

    let dir = match st.light_dir {
        -1 => "←",
        1 => "→",
        _ => "↑",
    };
    content.push(format!(
        "{TITLE}age{RESET} {}    {TITLE}growth{RESET} {:>5.1}%    {TITLE}light memory{RESET} {:>5.1}h",
        human_age(st.age_secs()),
        st.growth,
        st.total_light_hours()
    ));
    content.push(format!(
        "{WATER}water{RESET}  {} {:>5.1}%    {SUN}light{RESET} {} {:>5.1}% {dir}",
        bar(st.water, 12),
        st.water,
        bar(st.light, 12),
        st.light,
    ));
    content.push(format!(
        "{HEALTH}health{RESET} {} {:>5.1}%",
        bar(st.health, 12),
        st.health
    ));
    content.push(String::new());
    content.push(format!(
        "{DIM}[w/r]{RESET} water   {DIM}[a/s/d]{RESET} light   {DIM}[j/k/l]{RESET} prune   {DIM}[h/?]{RESET} help   {DIM}[q]{RESET} leave"
    ));
    if let Some(msg) = notice {
        content.push(format!("{MUTED}{msg}{RESET}"));
    } else {
        content.push(format!("{MUTED}Your tree keeps growing while you code.{RESET}"));
    }

    let top_pad = rows.saturating_sub(content.len()) / 2;
    let mut out = String::from("\x1b[?25l\x1b[2J\x1b[H");
    out.push_str(&"\n".repeat(top_pad));
    for line in content {
        out.push_str(&centered_line(&line, cols));
        out.push('\n');
    }
    out
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

fn show_frame(st: &State, notice: Option<&str>, delay_ms: u64) -> io::Result<()> {
    print!("{}", render(st, notice));
    io::stdout().flush()?;
    thread::sleep(Duration::from_millis(delay_ms));
    Ok(())
}

fn animate_water(st: &State) -> io::Result<()> {
    for (msg, delay) in [
        ("       ·     ·        ", 90),
        ("     ·   · ·   ·      ", 100),
        ("   ˙ · ˙ · ˙ · ˙     ", 110),
        ("the soil darkens gently", 140),
    ] {
        show_frame(st, Some(msg), delay)?;
    }
    Ok(())
}

fn animate_light(st: &State, direction: &str) -> io::Result<()> {
    let frames: [&str; 3] = match direction {
        "left" => ["☀  ·", "☀  · ·", "soft light settles from the left"],
        "right" => ["·  ☀", "· ·  ☀", "soft light settles from the right"],
        _ => ["   ☀", "   ↓", "soft light settles above"],
    };
    for msg in frames {
        show_frame(st, Some(msg), 125)?;
    }
    Ok(())
}

fn animate_prune(st: &State, side: &str) -> io::Result<()> {
    let target = match side {
        "left" => "left branch",
        "right" => "right branch",
        _ => "crown",
    };
    for msg in ["      ✂", "     ✂ ·", target] {
        show_frame(st, Some(msg), 120)?;
    }
    Ok(())
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

fn watch_help() -> String {
    let (cols, rows) = terminal_size();
    let lines = [
        format!("{TITLE}bonzai controls{RESET}"),
        String::new(),
        format!("{WATER}w / r{RESET}     water"),
        format!("{SUN}a / s / d{RESET} move light left / center / right"),
        format!("{HEALTH}j / k / l{RESET} prune left / crown / right"),
        format!("{TITLE}h or ?{RESET}    help"),
        format!("{TITLE}q{RESET}         leave viewer"),
        String::new(),
        format!("{DIM}Actions have a short cooldown so key repeat cannot queue an endless loop.{RESET}"),
        format!("{DIM}Light history, water stress and pruning affect future growth.{RESET}"),
        String::new(),
        format!("{MUTED}press any key to return{RESET}"),
    ];
    let top_pad = rows.saturating_sub(lines.len()) / 2;
    let mut out = String::from("\x1b[2J\x1b[H");
    out.push_str(&"\n".repeat(top_pad));
    for line in lines {
        out.push_str(&centered_line(&line, cols));
        out.push('\n');
    }
    out
}

fn watch() -> io::Result<()> {
    let _ = Command::new("stty")
        .args(["-echo", "-icanon", "min", "0", "time", "1"])
        .status();

    let result = (|| {
        let mut last_action = Instant::now()
            .checked_sub(Duration::from_millis(ACTION_COOLDOWN_MS))
            .unwrap_or_else(Instant::now);
        let mut notice: Option<String> = None;

        loop {
            let st = get_snapshot();
            print!("{}", render(&st, notice.as_deref()));
            io::stdout().flush()?;

            let mut buf = [0u8; 1];
            if io::stdin().read(&mut buf).unwrap_or(0) == 0 {
                thread::sleep(Duration::from_millis(80));
                continue;
            }

            let key = buf[0] as char;
            if key == 'q' {
                break;
            }

            if matches!(key, 'h' | '?') {
                print!("{}", watch_help());
                io::stdout().flush()?;
                let _ = Command::new("stty").args(["min", "1", "time", "0"]).status();
                let _ = io::stdin().read(&mut buf);
                drain_input();
                notice = None;
                continue;
            }

            let actionable = matches!(key, 'w' | 'r' | 'a' | 's' | 'd' | 'j' | 'k' | 'l');
            if !actionable {
                continue;
            }

            if last_action.elapsed() < Duration::from_millis(ACTION_COOLDOWN_MS) {
                notice = Some("easy there · let the tree settle for a moment".into());
                drain_input();
                continue;
            }

            last_action = Instant::now();
            notice = None;

            match key {
                'w' | 'r' => {
                    let before = get_snapshot();
                    if before.water >= 96.0 {
                        notice = Some("the soil is already well hydrated".into());
                    } else {
                        send("water")?;
                        let after = get_snapshot();
                        animate_water(&after)?;
                        notice = Some(format!("watered · {:.0}%", after.water));
                    }
                }
                'a' => {
                    send("light left")?;
                    let after = get_snapshot();
                    animate_light(&after, "left")?;
                    notice = Some("light moved left".into());
                }
                's' => {
                    send("light center")?;
                    let after = get_snapshot();
                    animate_light(&after, "center")?;
                    notice = Some("light centered".into());
                }
                'd' => {
                    send("light right")?;
                    let after = get_snapshot();
                    animate_light(&after, "right")?;
                    notice = Some("light moved right".into());
                }
                'j' => {
                    send("prune left")?;
                    let after = get_snapshot();
                    animate_prune(&after, "left")?;
                    notice = Some("left side pruned".into());
                }
                'k' => {
                    send("prune top")?;
                    let after = get_snapshot();
                    animate_prune(&after, "top")?;
                    notice = Some("crown pruned".into());
                }
                'l' => {
                    send("prune right")?;
                    let after = get_snapshot();
                    animate_prune(&after, "right")?;
                    notice = Some("right side pruned".into());
                }
                _ => {}
            }

            drain_input();
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
    for _ in 0..30 {
        thread::sleep(Duration::from_millis(50));
        if daemon_running() {
            println!("bonzai started");
            return Ok(());
        }
    }
    Err(io::Error::other("daemon failed to start"))
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
{DIM}Actions are rate-limited in watch mode to prevent terminal key repeat from queuing them.{RESET}\n\n\
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
            print!("{}", render(&st, None));
            print!("\x1b[?25h");
            Ok(())
        }
        "watch" => {
            if !daemon_running() {
                let _ = start_daemon();
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
