from pathlib import Path

MAIN = Path("src/main.rs")
TESTS = Path("tests/unit.rs")
CLI = Path("tests/cli.rs")
CARGO = Path("Cargo.toml")
LOCK = Path("Cargo.lock")
README = Path("README.md")

s = MAIN.read_text()
s = s.replace('const VERSION: &str = "0.2.8";', 'const VERSION: &str = "0.3.0";', 1)

state_start = s.index("#[derive(Clone, Debug)]\nstruct State")
state_end = s.index("\nfn triangular", state_start)
state_code = r'''#[derive(Clone, Debug, PartialEq, Eq)]
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

        let mut rng = Rng::new(
            self.seed
                ^ (step as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
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
        let side = if parent.side == 0 {
            if shape_bias > 0.18 && rng.chance(68, 100) {
                1
            } else if shape_bias < -0.18 && rng.chance(68, 100) {
                -1
            } else if rng.chance(1, 2) {
                -1
            } else {
                1
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
'''
s = s[:state_start] + state_code + s[state_end:]

old_prune = '''        "prune" => {
            let side = parts.next().unwrap_or("top");
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
            format!("Pruned {side}: {count}\\n")
        }
'''
new_prune = '''        "prune" => {
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
            format!("Pruned {side}: {count}\\n")
        }
'''
if old_prune not in s:
    raise SystemExit("prune command block not found")
s = s.replace(old_prune, new_prune, 1)

visual_start = s.index("// Visual grammar intentionally follows the classic cbonsai look.")
visual_end = s.index("#[derive(Copy, Clone)]\nenum SceneEffect", visual_start)
visual_code = r'''// Visual grammar follows the classic cbonsai vocabulary, but topology now
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
            let parent_path = paths.get(parent_index)?;
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

        let mut rng = Rng::new(
            st.seed
                ^ (u64::from(node.id) + 1).wrapping_mul(0xD1B5_4A32_D192_ED03),
        );
        let mut path = vec![(x, y)];
        for segment in 0..node.length {
            let (dx, dy) = segment_delta(node, segment, &mut rng);
            x = (x + dx).clamp(3, c.w - 4);
            y = (y + dy).clamp(2, c.h - 6);
            if node.depth > 0 && y >= c.h - 8 {
                y = (y - 1).max(2);
            }
            let glyph = segment_glyph(node, dx, dy);
            let kind = if node.depth == 0 || rng.chance(55, 100) { 1 } else { 5 };
            draw_str(c, x, y, glyph, kind);
            path.push((x, y));
        }
        paths[node_index] = path;

        let has_live_child = st
            .tree_nodes
            .iter()
            .any(|child| child.parent == node.id && !child.cut && st.node_visible(child.id));
        if !has_live_child && y < c.h - 9 {
            let mut leaf_rng = Rng::new(
                st.seed
                    ^ (u64::from(node.id) + 17).wrapping_mul(0x94D0_49BB_1331_11EB),
            );
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

'''
# Rust's Option ? cannot be used inside a function returning (), fix the parent path block before writing.
visual_code = visual_code.replace(
    '            let parent_path = paths.get(parent_index)?;\n            if parent_path.is_empty() {',
    '            let Some(parent_path) = paths.get(parent_index) else {\n                continue;\n            };\n            if parent_path.is_empty() {'
)
s = s[:visual_start] + visual_code + s[visual_end:]

old_local_prune = '''        "prune" => {
            let side = parts.next().unwrap_or("top");
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
'''
new_local_prune = '''        "prune" => {
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
'''
if old_local_prune not in s:
    raise SystemExit("local prune block not found")
s = s.replace(old_local_prune, new_local_prune, 1)
MAIN.write_text(s)

# Unit test fixtures must explicitly sync after overriding growth/environment.
t = TESTS.read_text()
fixture_old = '''            st.drought_stress = 0.0;
            st.wet_stress = 0.0;
            st
'''
fixture_new = '''            st.drought_stress = 0.0;
            st.wet_stress = 0.0;
            st.reset_tree_memory();
            st.sync_tree_memory();
            st
'''
if fixture_old not in t:
    raise SystemExit("stable_state fixture not found")
t = t.replace(fixture_old, fixture_new, 1)

roundtrip_old = '''            assert_eq!(parsed.prune_top, st.prune_top);
        }
'''
roundtrip_new = '''            assert_eq!(parsed.prune_top, st.prune_top);
            assert_eq!(parsed.tree_step, st.tree_step);
            assert_eq!(parsed.tree_nodes, st.tree_nodes);
        }
'''
t = t.replace(roundtrip_old, roundtrip_new, 1)

legacy_old = '''            assert_eq!(parsed.prune_top, 0);
        }
'''
legacy_new = '''            assert_eq!(parsed.prune_top, 0);
            assert!(!parsed.tree_nodes.is_empty());
            assert_eq!(parsed.tree_nodes[0].id, 0);
        }
'''
t = t.replace(legacy_old, legacy_new, 1)

# Replace old mask-based pruning tests at the tail with structural-memory tests.
prune_test_start = t.index("        #[test]\n        fn left_prune_removes_visible_left_canopy()")
module_end = t.rindex("    }\n}")
new_tests = r'''        fn topology_signature(st: &State) -> Vec<(u32, u32, i8, u8, u8, i8, u8, bool, u32)> {
            st.tree_nodes
                .iter()
                .map(|node| {
                    (
                        node.id,
                        node.parent,
                        node.side,
                        node.depth,
                        node.length,
                        node.lean,
                        node.attach,
                        node.cut,
                        node.born_step,
                    )
                })
                .collect()
        }

        #[test]
        fn growth_extends_persistent_structure_without_replacing_existing_ids() {
            let mut st = stable_state();
            let before_ids: Vec<u32> = st.tree_nodes.iter().map(|node| node.id).collect();
            let before_metric: usize = st
                .tree_nodes
                .iter()
                .map(|node| usize::from(node.length))
                .sum::<usize>()
                + st.tree_nodes.len();
            let before_step = st.tree_step;

            st.growth = (st.growth + 5.0).min(100.0);
            st.sync_tree_memory();

            assert!(st.tree_step > before_step);
            assert!(before_ids.iter().all(|id| st.node_index(*id).is_some()));
            let after_metric: usize = st
                .tree_nodes
                .iter()
                .map(|node| usize::from(node.length))
                .sum::<usize>()
                + st.tree_nodes.len();
            assert!(after_metric > before_metric);
        }

        #[test]
        fn care_changes_without_growth_do_not_rewrite_branch_topology() {
            let mut st = stable_state();
            let before = topology_signature(&st);
            st.water = 25.0;
            st.light = 91.0;
            st.light_dir = -1;
            st.sync_tree_memory();
            assert_eq!(topology_signature(&st), before);
        }

        #[test]
        fn prune_marks_a_real_left_branch_cut_and_persists_it() {
            let mut st = stable_state();
            let mut stop = false;
            handle_command(&mut st, "prune left", &mut stop);

            assert!(st
                .tree_nodes
                .iter()
                .any(|node| node.side < 0 && node.cut));
            let parsed = State::parse(&st.serialize()).expect("pruned tree should parse");
            assert_eq!(parsed.tree_nodes, st.tree_nodes);
        }

        #[test]
        fn structural_renderer_is_stable_when_only_water_changes() {
            let st = stable_state();
            let mut wetter = st.clone();
            wetter.water = (wetter.water + 20.0).min(100.0);

            let a = topology_signature(&st);
            let b = topology_signature(&wetter);
            assert_eq!(a, b);
        }

        #[test]
        fn legacy_state_is_migrated_to_structural_memory_once() {
            let raw = "seed=99\nborn_at=10\nlast_tick=10\nwater=70\nlight=70\nhealth=90\ngrowth=32\nprune_left=1\n";
            let first = State::parse(raw).expect("legacy state should migrate");
            assert!(first.tree_step >= 32);
            assert!(first.tree_nodes.len() > 2);
            assert!(first.tree_nodes.iter().any(|node| node.cut));

            let second = State::parse(&first.serialize()).expect("new state should parse");
            assert_eq!(first.tree_nodes, second.tree_nodes);
            assert_eq!(first.tree_step, second.tree_step);
        }
'''
t = t[:prune_test_start] + new_tests + t[module_end:]
TESTS.write_text(t)

cli = CLI.read_text()
cli = cli.replace(
    '        "wet_stress",\n',
    '        "wet_stress",\n        "tree_step",\n        "tree_next_id",\n        "tree_nodes",\n',
    1,
)
CLI.write_text(cli)

for path in (CARGO, LOCK):
    text = path.read_text()
    text = text.replace('version = "0.2.8"', 'version = "0.3.0"', 1)
    path.write_text(text)

readme = README.read_text()
readme = readme.replace(
    '| **Procedural** | The visible tree is reconstructed deterministically from its persistent state. |',
    '| **Procedural** | Branches have persistent identities. New growth extends existing tips instead of rebuilding the tree from scratch. |',
)
readme = readme.replace(
    '              persistent state\n                     │\n                     ▼\n              branch growth',
    '              persistent state\n                     │\n                     ▼\n            persistent branch graph\n                     │\n                     ▼\n              branch growth',
)
readme = readme.replace(
    'Keep the light on one side and new growth gradually favors it. Leave the tree in low light and shoots stretch more. Drought and repeated overwatering reduce vigor. Pruning suppresses future growth in the affected region.\n\nThe current environment is a condition. **The shape of the tree is a history.**',
    'Keep the light on one side and new branches gradually favor it. Leave the tree in low light and growth slows and stretches. Drought and repeated overwatering reduce vigor. Pruning cuts a real stored branch; later growth can emerge from the surviving structure.\n\nEach branch stores its parent, direction, attachment point, length, age and cut state. Existing branches keep their identity as the tree grows. **The current environment is a condition. The shape of the tree is a history.**',
)
README.write_text(readme)
