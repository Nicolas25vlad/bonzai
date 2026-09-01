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

const VERSION: &str = "0.3.0";
const TICK_SECS: u64 = 30;
const ACTION_COOLDOWN_MS: u64 = 420;

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const TITLE: &str = "\x1b[38;5;180m";
const TRUNK: &str = "\x1b[38;5;11m";
const TRUNK_DARK: &str = "\x1b[38;5;3m";
const LEAF: &str = "\x1b[38;5;2m";
const LEAF_BRIGHT: &str = "\x1b[1;38;5;10m";
const LEAF_DARK: &str = "\x1b[38;5;2m";
const POT: &str = "\x1b[38;5;245m";
const WATER: &str = "\x1b[38;5;110m";
const SUN: &str = "\x1b[38;5;221m";
const HEALTH: &str = "\x1b[38;5;151m";
const MUTED: &str = "\x1b[38;5;245m";
const SOIL: &str = "\x1b[38;5;130m";

#[derive(Clone, Debug, PartialEq, Eq)]
struct BranchNode {
    id: u32,
    parent: u32,
    side: i8,
    depth: u8,
    length: u8,
    age: u16,
    lean: i8,
    attach: u8,
    cut: bool,
    born_step: u32,
}

impl BranchNode {
    fn root() -> Self {
        Self {
            id: 0,
            parent: u32::MAX,
            side: 0,
            depth: 0,
            length: 4,
            age: 0,
            lean: 0,
            attach: 0,
            cut: false,
            born_step: 0,
        }
    }

    fn encode(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{}",
            self.id,
            self.parent,
            self.side,
            self.depth,
            self.length,
            self.age,
            self.lean,
            self.attach,
            u8::from(self.cut),
            self.born_step,
        )
    }

    fn parse_record(record: &str) -> Option<Self> {
        let mut p = record.split(',');
        Some(Self {
            id: p.next()?.parse().ok()?,
            parent: p.next()?.parse().ok()?,
            side: p.next()?.parse().ok()?,
            depth: p.next()?.parse().ok()?,
            length: p.next()?.parse().ok()?,
            age: p.next()?.parse().ok()?,
            lean: p.next()?.parse().ok()?,
            attach: p.next()?.parse().ok()?,
            cut: p.next()?.parse::<u8>().ok()? != 0,
            born_step: p.next()?.parse().ok()?,
        })
    }
}

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
    tree_step: u32,
    tree_next_id: u32,
    tree_nodes: Vec<BranchNode>,
}

impl State {
    fn new() -> Self {
        let now = now_secs();
        let mut st = Self {
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
            tree_step: 0,
            tree_next_id: 1,
            tree_nodes: vec![BranchNode::root()],
        };
        st.sync_tree_memory();
        st
    }

    fn reset_tree_memory(&mut self) {
        self.tree_step = 0;
        self.tree_next_id = 1;
        self.tree_nodes.clear();
        self.tree_nodes.push(BranchNode::root());
    }

    fn node_index(&self, id: u32) -> Option<usize> {
        self.tree_nodes.iter().position(|node| node.id == id)
    }

    fn node_visible(&self, id: u32) -> bool {
        let mut current = id;
        for _ in 0..=self.tree_nodes.len() {
            let Some(index) = self.node_index(current) else {
                return false;
            };
            let node = &self.tree_nodes[index];
            if node.cut {
                return false;
            }
            if node.parent == u32::MAX {
                return true;
            }
            current = node.parent;
        }
        false
    }

    fn subtree_size(&self, root: u32) -> usize {
        self.tree_nodes
            .iter()
            .filter(|node| {
                if !self.node_visible(node.id) {
                    return false;
                }
                let mut current = node.id;
                for _ in 0..=self.tree_nodes.len() {
                    if current == root {
                        return true;
                    }
                    let Some(index) = self.node_index(current) else {
                        return false;
                    };
                    let parent = self.tree_nodes[index].parent;
                    if parent == u32::MAX {
                        return false;
                    }
                    current = parent;
                }
                false
            })
            .count()
    }

    fn add_branch(&mut self, parent: &BranchNode, side: i8, lean: i8, attach: u8, step: u32) {
        if self.tree_nodes.len() >= 96 || parent.depth >= 3 {
            return;
        }
        let id = self.tree_next_id;
        self.tree_next_id = self.tree_next_id.saturating_add(1);
        self.tree_nodes.push(BranchNode {
            id,
            parent: parent.id,
            side,
            depth: parent.depth + 1,
            length: 2,
            age: 0,
            lean,
            attach,
            cut: false,
            born_step: step,
        });
    }

    fn grow_structure_step(&mut self, step: u32) {
        for node in &mut self.tree_nodes {
            if !node.cut {
                node.age = node.age.saturating_add(1);
            }
        }

        if step <= 2 {
            self.tree_nodes[0].length = self.tree_nodes[0].length.saturating_add(1).min(18);
            return;
        }
        if step == 3 {
            let root = self.tree_nodes[0].clone();
            let attach = root.length.saturating_sub(2).max(2);
            self.add_branch(&root, -1, -1, attach, step);
            return;
        }
        if step == 4 {
            let root = self.tree_nodes[0].clone();
            let attach = root.length.saturating_sub(1).max(2);
            self.add_branch(&root, 1, 1, attach, step);
            return;
        }

        let mut rng = Rng::new(self.seed ^ (step as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let candidates: Vec<usize> = self
            .tree_nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| !node.cut && node.depth <= 3 && self.node_visible(node.id))
            .map(|(index, _)| index)
            .collect();
        if candidates.is_empty() {
            return;
        }

        let index = candidates[(rng.next() as usize) % candidates.len()];
        let parent = self.tree_nodes[index].clone();
        let child_count = self
            .tree_nodes
            .iter()
            .filter(|node| node.parent == parent.id && !node.cut)
            .count();
        let max_len = match parent.depth {
            0 => 18,
            1 => 11,
            2 => 8,
            _ => 6,
        };

        if parent.length < max_len && (child_count >= 2 || rng.chance(58, 100)) {
            self.tree_nodes[index].length = self.tree_nodes[index].length.saturating_add(1);
            return;
        }

        if parent.depth >= 3 || child_count >= 3 || self.tree_nodes.len() >= 96 {
            if parent.length < max_len {
                self.tree_nodes[index].length = self.tree_nodes[index].length.saturating_add(1);
            }
            return;
        }

        let light_bias = self.photo_bias() + self.light_dir as f32 * 0.22;
        let prune_bias = (self.prune_left.min(6) as f32 - self.prune_right.min(6) as f32) * 0.08;
        let shape_bias = (light_bias + prune_bias).clamp(-1.2, 1.2);
        let random_side = if rng.chance(1, 2) { -1 } else { 1 };
        let side = if parent.side == 0 {
            if shape_bias > 0.18 && rng.chance(68, 100) {
                1
            } else if shape_bias < -0.18 && rng.chance(68, 100) {
                -1
            } else {
                random_side
            }
        } else if rng.chance(72, 100) {
            parent.side
        } else {
            -parent.side
        };
        let light_pull = if shape_bias > 0.22 {
            1
        } else if shape_bias < -0.22 {
            -1
        } else {
            0
        };
        let lean = (side + light_pull).clamp(-2, 2);
        let low_attach = if parent.id == 0 { 3 } else { 1 };
        let high_attach = i32::from(parent.length.max(low_attach + 1));
        let attach = rng.range(i32::from(low_attach), high_attach) as u8;
        self.add_branch(&parent, side, lean, attach, step);
    }

    fn sync_tree_memory(&mut self) {
        if self.tree_nodes.is_empty() {
            self.reset_tree_memory();
        }
        let target = self.growth.floor().clamp(0.0, 100.0) as u32;
        while self.tree_step < target {
            self.tree_step += 1;
            self.grow_structure_step(self.tree_step);
        }
        if let Some(max_id) = self.tree_nodes.iter().map(|node| node.id).max() {
            self.tree_next_id = self.tree_next_id.max(max_id.saturating_add(1));
        }
    }

    fn cut_branch(&mut self, side: &str) -> Option<u32> {
        self.sync_tree_memory();
        let candidates: Vec<u32> = self
            .tree_nodes
            .iter()
            .filter(|node| {
                node.id != 0
                    && !node.cut
                    && self.node_visible(node.id)
                    && match side {
                        "left" => node.side < 0,
                        "right" => node.side > 0,
                        _ => true,
                    }
            })
            .map(|node| node.id)
            .collect();

        let chosen = candidates.into_iter().max_by_key(|id| {
            let node = &self.tree_nodes[self.node_index(*id).expect("candidate must exist")];
            (self.subtree_size(*id), node.depth, node.id)
        })?;
        let index = self.node_index(chosen)?;
        self.tree_nodes[index].cut = true;
        Some(chosen)
    }

    fn apply_legacy_pruning(&mut self) {
        for _ in 0..self.prune_left.min(3) {
            let _ = self.cut_branch("left");
        }
        for _ in 0..self.prune_right.min(3) {
            let _ = self.cut_branch("right");
        }
        for _ in 0..self.prune_top.min(3) {
            let _ = self.cut_branch("top");
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
        self.sync_tree_memory();
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
        let nodes = self
            .tree_nodes
            .iter()
            .map(BranchNode::encode)
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "seed={}\nborn_at={}\nlast_tick={}\nwater={:.4}\nlight={:.4}\nhealth={:.4}\ngrowth={:.4}\nlight_dir={}\nprune_left={}\nprune_right={}\nprune_top={}\nlight_left_hours={:.4}\nlight_center_hours={:.4}\nlight_right_hours={:.4}\ndrought_stress={:.4}\nwet_stress={:.4}\ntree_step={}\ntree_next_id={}\ntree_nodes={}\n",
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
            self.tree_step,
            self.tree_next_id,
            nodes,
        )
    }

    fn parse(s: &str) -> Option<Self> {
        let mut st = State::new();
        let mut saw_tree_nodes = false;
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
                "tree_step" => st.tree_step = v.parse().ok()?,
                "tree_next_id" => st.tree_next_id = v.parse().ok()?,
                "tree_nodes" => {
                    saw_tree_nodes = true;
                    st.tree_nodes = if v.is_empty() {
                        Vec::new()
                    } else {
                        v.split(';').filter_map(BranchNode::parse_record).collect()
                    };
                }
                _ => {}
            }
        }

        if saw_tree_nodes {
            st.tree_nodes.sort_by_key(|node| node.id);
            if st.tree_nodes.is_empty() || st.tree_nodes[0].id != 0 {
                st.tree_nodes.insert(0, BranchNode::root());
            }
            st.sync_tree_memory();
        } else {
            // v0.2.x stored only environmental state. Rebuild one structural
            // baseline from that history once, then persist branch identities.
            st.reset_tree_memory();
            st.sync_tree_memory();
            st.apply_legacy_pruning();
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
                let command = line.trim();
                let response = handle_command(&mut st, command, &mut should_stop);
                // Read-only probes are frequent while the viewer is open. Persist only
                // mutations; elapsed time is still checkpointed by the periodic save.
                if !matches!(command, "ping" | "snapshot") {
                    save_state(&st)?;
                }
                // A client may disconnect before reading the reply. That must never
                // terminate the authoritative daemon.
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
            let _ = st.cut_branch(side);
            let count = match side {
                "left" => {
                    st.prune_left = st.prune_left.saturating_add(1);
                    st.prune_left
                }
                "right" => {
                    st.prune_right = st.prune_right.saturating_add(1);
                    st.prune_right
                }
                _ => {
                    st.prune_top = st.prune_top.saturating_add(1);
                    st.prune_top
                }
            };
            st.health = (st.health - 0.4).max(0.0);
            format!("Pruned {side}: {count}\n")
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

// Visual grammar follows the classic cbonsai vocabulary, but topology now
// comes from persistent branch nodes instead of being regenerated per frame.
fn draw_str(c: &mut Canvas, x: i32, y: i32, text: &str, kind: u8) {
    let width = text.chars().count() as i32;
    let start_x = x - width / 2;
    for (i, ch) in text.chars().enumerate() {
        if ch != ' ' {
            c.set(start_x + i as i32, y, ch, kind);
        }
    }
}

#[allow(dead_code)]
fn is_tree_cell(cell: Cell) -> bool {
    matches!(cell.kind, 1..=5)
}

fn segment_delta(node: &BranchNode, segment: u8, rng: &mut Rng) -> (i32, i32) {
    if node.id == 0 && segment < 4 {
        return (0, -1);
    }

    if node.side == 0 {
        let dx = if node.lean != 0 && rng.chance(30, 100) {
            node.lean.signum() as i32
        } else if rng.chance(24, 100) {
            rng.range(-1, 2)
        } else {
            0
        };
        let dy = if rng.chance(82, 100) { -1 } else { 0 };
        return (dx.clamp(-1, 1), dy);
    }

    let outward = i32::from(node.side.signum());
    let mut dx = if rng.chance(68, 100) { outward } else { 0 };
    if node.lean.signum() == node.side.signum() && rng.chance(18, 100) {
        dx += outward;
    }
    let dy = if rng.chance(44 + u64::from(node.depth) * 8, 100) {
        -1
    } else {
        0
    };
    (dx.clamp(-2, 2), dy)
}

fn segment_glyph(node: &BranchNode, dx: i32, dy: i32) -> &'static str {
    if node.side == 0 {
        if dx < 0 {
            "\\|"
        } else if dx > 0 {
            "|/"
        } else if dy < 0 {
            "/|\\"
        } else {
            "/~"
        }
    } else if node.side < 0 {
        if dy < 0 {
            "\\|"
        } else if dx < 0 {
            "\\_"
        } else {
            "\\"
        }
    } else if dy < 0 {
        "/|"
    } else if dx > 0 {
        "_/"
    } else {
        "/"
    }
}

fn draw_leaf_spray(c: &mut Canvas, rng: &mut Rng, x: i32, y: i32, vigor: f32, lean: i8) {
    let pull = i32::from(lean.signum());
    let pads = (2.0 + vigor * 3.0).round() as i32;
    let density = (72.0 + vigor * 23.0) as u64;

    for _ in 0..pads {
        let light_shift = if pull != 0 && rng.chance(45, 100) {
            pull * 2
        } else {
            0
        };
        let px = x + rng.range(-5, 6) + light_shift;
        let py = y + rng.range(-2, 3);
        let width = rng.range(3, 7);
        let height = rng.range(1, 4);

        for row in 0..height {
            let edge_taper = i32::from(height > 1 && (row == 0 || row == height - 1));
            let row_width = (width - edge_taper).max(2);
            let start_x = px - row_width / 2 + rng.range(-1, 2);
            let row_y = py + row - height / 2;
            if row_y >= c.h - 9 {
                continue;
            }
            for col in 0..row_width {
                if rng.chance(density, 100) {
                    let bright = rng.chance((10.0 + vigor * 18.0) as u64, 100);
                    c.set(start_x + col, row_y, '&', if bright { 3 } else { 2 });
                }
            }
        }
    }
}

fn draw_persistent_tree(c: &mut Canvas, st: &State, base_x: i32, base_y: i32) {
    let stress = ((st.drought_stress + st.wet_stress) / 75.0).clamp(0.0, 0.72);
    let vigor = ((st.health / 100.0) * (1.0 - stress)).clamp(0.18, 1.0);
    let max_id = st.tree_nodes.iter().map(|node| node.id).max().unwrap_or(0) as usize;
    let mut paths: Vec<Vec<(i32, i32)>> = vec![Vec::new(); max_id + 1];

    for node in &st.tree_nodes {
        let node_index = node.id as usize;
        if node_index >= paths.len() {
            continue;
        }

        let start = if node.parent == u32::MAX {
            Some((base_x, base_y))
        } else {
            let parent_index = node.parent as usize;
            let Some(parent_path) = paths.get(parent_index) else {
                continue;
            };
            if parent_path.is_empty() {
                None
            } else {
                let attach = usize::from(node.attach).min(parent_path.len() - 1);
                Some(parent_path[attach])
            }
        };
        let Some((mut x, mut y)) = start else {
            continue;
        };

        if node.cut {
            if node.parent != u32::MAX {
                c.set(x, y, '+', 5);
            }
            continue;
        }

        let mut rng =
            Rng::new(st.seed ^ (u64::from(node.id) + 1).wrapping_mul(0xD1B5_4A32_D192_ED03));
        let mut path = vec![(x, y)];
        for segment in 0..node.length {
            let (dx, dy) = segment_delta(node, segment, &mut rng);
            x = (x + dx).clamp(3, c.w - 4);
            y = (y + dy).clamp(2, c.h - 6);
            if node.depth > 0 && y >= c.h - 8 {
                y = (y - 1).max(2);
            }
            let glyph = segment_glyph(node, dx, dy);
            let kind = if node.depth == 0 || rng.chance(55, 100) {
                1
            } else {
                5
            };
            draw_str(c, x, y, glyph, kind);
            path.push((x, y));
        }
        paths[node_index] = path;

        let has_live_child = st
            .tree_nodes
            .iter()
            .any(|child| child.parent == node.id && !child.cut && st.node_visible(child.id));
        if !has_live_child && y < c.h - 9 {
            let mut leaf_rng =
                Rng::new(st.seed ^ (u64::from(node.id) + 17).wrapping_mul(0x94D0_49BB_1331_11EB));
            draw_leaf_spray(c, &mut leaf_rng, x, y, vigor, node.lean);
        }
    }
}

fn grow_tree(st: &State, w: i32, h: i32) -> Canvas {
    let mut c = Canvas::new(w, h);
    let base_y = h - 5;
    let base_x = w / 2;

    draw_persistent_tree(&mut c, st, base_x, base_y);

    let pot = [
        ("      .-----------------.      ", 7u8),
        (r"       \               /       ", 7u8),
        (r"        \_____________/        ", 7u8),
        ("        (_)         (_)        ", 7u8),
    ];
    for (i, (row, kind)) in pot.iter().enumerate() {
        let sx = base_x - row.chars().count() as i32 / 2;
        for (j, ch) in row.chars().enumerate() {
            if ch == ' ' {
                c.set(sx + j as i32, base_y + i as i32, ' ', 0);
            } else {
                c.set(sx + j as i32, base_y + i as i32, ch, *kind);
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
                let y = (2 + offset + fall * 3).min(base_y - 2);
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
    // The watch view lives for a long time, so terminal geometry cannot be cached.
    // Re-detecting here lets the layout recover after a resize instead of keeping
    // stale dimensions for the lifetime of the process.
    detect_terminal_size()
}

fn detect_terminal_size() -> (usize, usize) {
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

fn compact_scene(st: &State, rows: usize, cols: usize) -> String {
    let lines = [
        format!("{TITLE}bonzai{RESET}"),
        format!("{MUTED}{}{RESET}", mood(st)),
        String::new(),
        format!("{DIM}terminal too small{RESET}"),
        format!("{DIM}resize to at least 36 x 18{RESET}"),
        String::new(),
        format!(
            "{WATER}water{RESET} {:>3.0}%  {SUN}light{RESET} {:>3.0}%",
            st.water, st.light
        ),
        format!("{HEALTH}health{RESET} {:>3.0}%", st.health),
        format!("{DIM}[q] leave  [?] help{RESET}"),
    ];

    let top_pad = rows.saturating_sub(lines.len()) / 2;
    let mut out = String::new();
    out.push_str(&"\n".repeat(top_pad));
    for line in lines {
        out.push_str(&center_line(&line, cols));
        out.push('\n');
    }
    out
}

fn render_scene_for_size(
    st: &State,
    effect: Option<SceneEffect>,
    rows: usize,
    cols: usize,
) -> String {
    if rows < 18 || cols < 36 {
        return compact_scene(st, rows, cols);
    }

    let narrow = cols < 80;
    let split_controls = cols < 68;
    let footer_lines = if narrow { 3 } else { 1 } + if split_controls { 2 } else { 1 };
    let overhead = 2 + footer_lines;
    let h = rows.saturating_sub(overhead).clamp(10, 28) as i32;
    let w = cols.saturating_sub(2).clamp(30, 68) as i32;

    let mut c = grow_tree(st, w, h);
    if let Some(effect) = effect {
        apply_effect(&mut c, effect);
    }

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

    if narrow {
        lines.push(format!(
            "{WATER}water{RESET}  {} {:>3.0}%   {SUN}light{RESET}  {} {:>3.0}% {dir}",
            bar(st.water, 8),
            st.water,
            bar(st.light, 8),
            st.light,
        ));
        lines.push(format!(
            "{HEALTH}health{RESET} {} {:>3.0}%",
            bar(st.health, 10),
            st.health,
        ));
        lines.push(String::new());
    } else {
        lines.push(format!(
            "{WATER}water{RESET} {} {:>3.0}%   {SUN}light{RESET} {} {:>3.0}% {dir}   {HEALTH}health{RESET} {} {:>3.0}%",
            bar(st.water, 10), st.water,
            bar(st.light, 10), st.light,
            bar(st.health, 10), st.health,
        ));
    }

    if split_controls {
        lines.push(format!(
            "{DIM}[w/r] water   [a/s/d] light   [h/?] help{RESET}"
        ));
        lines.push(format!("{DIM}[j/k/l] prune   [q] leave{RESET}"));
    } else {
        lines.push(format!(
            "{DIM}[w/r] water   [a/s/d] light   [j/k/l] prune   [h/?] help   [q] leave{RESET}"
        ));
    }

    let top_pad = rows.saturating_sub(lines.len()) / 2;
    let mut out = String::new();
    out.push_str(&"\n".repeat(top_pad));
    for line in lines {
        out.push_str(&center_line(&line, cols));
        out.push('\n');
    }
    out
}

fn render_scene(st: &State, effect: Option<SceneEffect>) -> String {
    let (rows, cols) = terminal_size();
    render_scene_for_size(st, effect, rows, cols)
}

fn render(st: &State) -> String {
    render_scene(st, None)
}

fn frame_escape(frame: &str) -> String {
    let lines: Vec<&str> = frame.lines().collect();
    let mut out = String::with_capacity(frame.len() + lines.len() * 8 + 16);
    out.push_str("\x1b[H");

    for (index, line) in lines.iter().enumerate() {
        out.push_str("\x1b[2K\r");
        out.push_str(line);
        if index + 1 < lines.len() {
            // Cursor Next Line avoids writing a literal newline on the bottom row,
            // which can scroll the alternate screen and make the whole tree jump.
            out.push_str("\x1b[E");
        }
    }
    out.push_str("\x1b[J");
    out
}

fn paint_frame(frame: &str) -> io::Result<()> {
    let out = frame_escape(frame);
    let mut stdout = io::stdout();
    stdout.write_all(out.as_bytes())?;
    stdout.flush()
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

fn response_percent(response: &str) -> Option<f32> {
    response
        .split_whitespace()
        .rev()
        .find_map(|token| token.strip_suffix('%')?.parse::<f32>().ok())
}

fn response_count(response: &str) -> Option<u32> {
    response
        .split_whitespace()
        .last()
        .and_then(|token| token.trim_end_matches(['.', ',']).parse::<u32>().ok())
}

fn apply_local_command_result(st: &mut State, command: &str, response: &str) {
    let mut parts = command.split_whitespace();
    match parts.next().unwrap_or("") {
        "water" => {
            if let Some(water) = response_percent(response) {
                st.water = water.clamp(0.0, 100.0);
            }
        }
        "light" => {
            if let Some(light) = response_percent(response) {
                st.light = light.clamp(0.0, 100.0);
            }
            st.light_dir = match parts.next().unwrap_or("center") {
                "left" => -1,
                "right" => 1,
                _ => 0,
            };
        }
        "prune" => {
            let side = parts.next().unwrap_or("top");
            let _ = st.cut_branch(side);
            let count = response_count(response);
            match side {
                "left" => st.prune_left = count.unwrap_or_else(|| st.prune_left.saturating_add(1)),
                "right" => {
                    st.prune_right = count.unwrap_or_else(|| st.prune_right.saturating_add(1))
                }
                _ => st.prune_top = count.unwrap_or_else(|| st.prune_top.saturating_add(1)),
            }
            st.health = (st.health - 0.4).max(0.0);
        }
        _ => {}
    }
}

fn animate_water(st: &State) -> io::Result<()> {
    let (rows, cols) = terminal_size();
    for frame in 0..3 {
        paint_frame(&render_scene_for_size(
            st,
            Some(SceneEffect::Water(frame)),
            rows,
            cols,
        ))?;
        thread::sleep(Duration::from_millis(65));
    }
    paint_frame(&render_scene_for_size(st, None, rows, cols))
}

fn animate_light(direction: &str, st: &State) -> io::Result<()> {
    let dir = match direction {
        "left" => -1,
        "right" => 1,
        _ => 0,
    };
    let (rows, cols) = terminal_size();
    for frame in 0..2 {
        paint_frame(&render_scene_for_size(
            st,
            Some(SceneEffect::Light(dir, frame)),
            rows,
            cols,
        ))?;
        thread::sleep(Duration::from_millis(75));
    }
    paint_frame(&render_scene_for_size(st, None, rows, cols))
}

fn animate_prune(side: &str, st: &State) -> io::Result<()> {
    let side = match side {
        "left" => -1,
        "right" => 1,
        _ => 0,
    };
    let (rows, cols) = terminal_size();
    for frame in 0..2 {
        paint_frame(&render_scene_for_size(
            st,
            Some(SceneEffect::Prune(side, frame)),
            rows,
            cols,
        ))?;
        thread::sleep(Duration::from_millis(70));
    }
    paint_frame(&render_scene_for_size(st, None, rows, cols))
}

fn watch_help() -> String {
    let (rows, cols) = terminal_size();
    let mut lines = vec![
        format!("{TITLE}bonzai controls{RESET}"),
        String::new(),
        format!("{WATER}w / r{RESET}   water"),
        format!("{SUN}a / s / d{RESET}   light left / center / right"),
        format!("{HEALTH}j / k / l{RESET}   prune left / top / right"),
        format!("{TITLE}h / ?{RESET}   help"),
        format!("{TITLE}q{RESET}   leave"),
        String::new(),
    ];

    if cols >= 58 {
        lines.push(format!(
            "{MUTED}held keys are rate-limited to protect the tree{RESET}"
        ));
        lines.push(format!(
            "{MUTED}light direction is remembered by future growth{RESET}"
        ));
        lines.push(String::new());
    }
    lines.push(format!("{DIM}press any key to return{RESET}"));

    let mut out = String::new();
    out.push_str(&"\n".repeat(rows.saturating_sub(lines.len()) / 2));
    for line in lines {
        out.push_str(&center_line(&line, cols));
        out.push('\n');
    }
    out
}

fn watch() -> io::Result<()> {
    let _ = Command::new("stty")
        .args(["-echo", "-icanon", "min", "0", "time", "1"])
        .status();

    print!("\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H");
    io::stdout().flush()?;

    let result = (|| {
        let mut last_action = Instant::now() - Duration::from_millis(ACTION_COOLDOWN_MS);
        let mut last_sync = Instant::now();
        let mut st = get_snapshot();
        paint_frame(&render(&st))?;

        loop {
            let mut buf = [0u8; 1];
            if io::stdin().read(&mut buf).unwrap_or(0) > 0 {
                let key = buf[0] as char;
                if key == 'q' {
                    break;
                }

                if matches!(key, 'w' | 'r' | 'a' | 's' | 'd' | 'j' | 'k' | 'l')
                    && last_action.elapsed() < Duration::from_millis(ACTION_COOLDOWN_MS)
                {
                    continue;
                }

                match key {
                    'w' | 'r' => {
                        let response = send("water")?;
                        apply_local_command_result(&mut st, "water", &response);
                        animate_water(&st)?;
                        last_action = Instant::now();
                        last_sync = Instant::now();
                    }
                    'a' | 's' | 'd' => {
                        let direction = match key {
                            'a' => "left",
                            'd' => "right",
                            _ => "center",
                        };
                        let command = format!("light {direction}");
                        let response = send(&command)?;
                        apply_local_command_result(&mut st, &command, &response);
                        animate_light(direction, &st)?;
                        last_action = Instant::now();
                        last_sync = Instant::now();
                    }
                    'j' | 'k' | 'l' => {
                        let side = match key {
                            'j' => "left",
                            'l' => "right",
                            _ => "top",
                        };
                        let command = format!("prune {side}");
                        let response = send(&command)?;
                        apply_local_command_result(&mut st, &command, &response);
                        animate_prune(side, &st)?;
                        last_action = Instant::now();
                        last_sync = Instant::now();
                    }
                    'h' | '?' => {
                        paint_frame(&watch_help())?;
                        loop {
                            if io::stdin().read(&mut buf).unwrap_or(0) > 0 {
                                break;
                            }
                        }
                        paint_frame(&render(&st))?;
                    }
                    _ => {}
                }
            }

            if last_sync.elapsed() >= Duration::from_secs(2) {
                st = get_snapshot();
                paint_frame(&render(&st))?;
                last_sync = Instant::now();
            }
        }
        Ok(())
    })();

    let _ = Command::new("stty").arg("sane").status();
    print!("\x1b[?25h\x1b[?1049l\x1b[0m");
    let _ = io::stdout().flush();
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
        "{SOIL}memory{RESET} L {:.1}h  C {:.1}h  R {:.1}h  ·  total {:.1}h",
        st.light_left_hours,
        st.light_center_hours,
        st.light_right_hours,
        st.total_light_hours()
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
