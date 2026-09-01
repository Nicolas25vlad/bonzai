from pathlib import Path

MAIN = Path("src/main.rs")
TESTS = Path("tests/unit.rs")
CARGO = Path("Cargo.toml")
LOCK = Path("Cargo.lock")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"missing expected block: {label}")
    return text.replace(old, new, 1)


s = MAIN.read_text()
s = replace_once(s, 'const VERSION: &str = "0.2.7";', 'const VERSION: &str = "0.2.8";', "version")

s = replace_once(
    s,
    '''        "prune" => {
            let side = parts.next().unwrap_or("top");
            match side {
                "left" => st.prune_left = st.prune_left.saturating_add(1),
                "right" => st.prune_right = st.prune_right.saturating_add(1),
                _ => st.prune_top = st.prune_top.saturating_add(1),
            }
            st.health = (st.health - 0.4).max(0.0);
            format!("Pruned {side}.\\n")
        }
''',
    '''        "prune" => {
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
''',
    "authoritative prune response",
)

marker = '''fn grow_tree(st: &State, w: i32, h: i32) -> Canvas {
'''
prune_helpers = r'''fn is_tree_cell(cell: Cell) -> bool {
    matches!(cell.kind, 1..=5)
}

fn clear_tree_cell(c: &mut Canvas, x: i32, y: i32) {
    if is_tree_cell(c.get(x, y)) {
        c.set(x, y, ' ', 0);
    }
}

fn apply_prune_shape(c: &mut Canvas, st: &State) {
    let center = c.w / 2;
    let canopy_bottom = (c.h - 9).max(3);

    if st.prune_left > 0 {
        let mut leftmost: Option<i32> = None;
        for y in 2..canopy_bottom {
            for x in 0..(center - 3).max(0) {
                if is_tree_cell(c.get(x, y)) {
                    leftmost = Some(leftmost.map_or(x, |current| current.min(x)));
                }
            }
        }
        if let Some(leftmost) = leftmost {
            let bite = 1 + st.prune_left.min(6) as i32 * 2;
            let cut_x = (leftmost + bite).min(center - 4);
            for y in 2..canopy_bottom {
                for x in 0..=cut_x.max(0) {
                    clear_tree_cell(c, x, y);
                }
            }
        }
    }

    if st.prune_right > 0 {
        let mut rightmost: Option<i32> = None;
        for y in 2..canopy_bottom {
            for x in (center + 4).min(c.w - 1)..c.w {
                if is_tree_cell(c.get(x, y)) {
                    rightmost = Some(rightmost.map_or(x, |current| current.max(x)));
                }
            }
        }
        if let Some(rightmost) = rightmost {
            let bite = 1 + st.prune_right.min(6) as i32 * 2;
            let cut_x = (rightmost - bite).max(center + 4);
            for y in 2..canopy_bottom {
                for x in cut_x.min(c.w - 1)..c.w {
                    clear_tree_cell(c, x, y);
                }
            }
        }
    }

    if st.prune_top > 0 {
        let mut topmost: Option<i32> = None;
        for y in 2..canopy_bottom {
            for x in 0..c.w {
                if is_tree_cell(c.get(x, y)) {
                    topmost = Some(topmost.map_or(y, |current| current.min(y)));
                }
            }
        }
        if let Some(topmost) = topmost {
            let bite = 1 + st.prune_top.min(6) as i32 * 2;
            let cut_y = (topmost + bite).min(canopy_bottom - 2);
            for y in 0..=cut_y.max(0) {
                for x in 0..c.w {
                    clear_tree_cell(c, x, y);
                }
            }
        }
    }
}

'''
s = replace_once(s, marker, prune_helpers + marker, "prune shape helpers")

s = replace_once(
    s,
    '''    draw_cb_branch(
        &mut c,
        &mut rng,
        base_x,
        base_y,
        BranchKind::Trunk,
        life.max(10),
        0,
        vigor,
        tropism,
        st.prune_left,
        st.prune_right,
    );

    // Wide, quiet planter inspired by cbonsai's classic terminal silhouette.
''',
    '''    draw_cb_branch(
        &mut c,
        &mut rng,
        base_x,
        base_y,
        BranchKind::Trunk,
        life.max(10),
        0,
        vigor,
        tropism,
        st.prune_left,
        st.prune_right,
    );

    // Pruning must affect the tree that is already on screen, not only the
    // probability of future branches. The mask clips the current silhouette in
    // a deterministic way while the counters continue to influence growth.
    apply_prune_shape(&mut c, st);

    // Wide, quiet planter inspired by cbonsai's classic terminal silhouette.
''',
    "apply persistent prune silhouette",
)

get_snapshot = '''fn get_snapshot() -> State {
    if let Ok(s) = send("snapshot") {
        State::parse(&s).unwrap_or_else(load_state)
    } else {
        load_state()
    }
}

'''
local_helpers = r'''fn response_percent(response: &str) -> Option<f32> {
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

'''
s = replace_once(s, get_snapshot, get_snapshot + local_helpers, "local command reconciliation")

s = replace_once(
    s,
    '''                    'w' | 'r' => {
                        let _ = send("water")?;
                        st = get_snapshot();
                        animate_water(&st)?;
                        last_action = Instant::now();
                        last_sync = Instant::now();
                    }
''',
    '''                    'w' | 'r' => {
                        let response = send("water")?;
                        apply_local_command_result(&mut st, "water", &response);
                        animate_water(&st)?;
                        last_action = Instant::now();
                        last_sync = Instant::now();
                    }
''',
    "instant water ui update",
)

s = replace_once(
    s,
    '''                        let _ = send(&format!("light {direction}"))?;
                        st = get_snapshot();
                        animate_light(direction, &st)?;
                        last_action = Instant::now();
                        last_sync = Instant::now();
''',
    '''                        let command = format!("light {direction}");
                        let response = send(&command)?;
                        apply_local_command_result(&mut st, &command, &response);
                        animate_light(direction, &st)?;
                        last_action = Instant::now();
                        last_sync = Instant::now();
''',
    "instant light ui update",
)

s = replace_once(
    s,
    '''                        let _ = send(&format!("prune {side}"))?;
                        st = get_snapshot();
                        animate_prune(side, &st)?;
                        last_action = Instant::now();
                        last_sync = Instant::now();
''',
    '''                        let command = format!("prune {side}");
                        let response = send(&command)?;
                        apply_local_command_result(&mut st, &command, &response);
                        animate_prune(side, &st)?;
                        last_action = Instant::now();
                        last_sync = Instant::now();
''',
    "instant prune ui update",
)

MAIN.write_text(s)

cargo = CARGO.read_text()
cargo = replace_once(cargo, 'version = "0.2.7"', 'version = "0.2.8"', "cargo version")
CARGO.write_text(cargo)

lock = LOCK.read_text()
lock = replace_once(lock, 'version = "0.2.7"', 'version = "0.2.8"', "lock version")
LOCK.write_text(lock)

t = TESTS.read_text()
insert = r'''

        fn tree_cells_in_region(canvas: &Canvas, x0: i32, x1: i32, y0: i32, y1: i32) -> usize {
            let mut total = 0usize;
            for y in y0.max(0)..y1.min(canvas.h) {
                for x in x0.max(0)..x1.min(canvas.w) {
                    if is_tree_cell(canvas.get(x, y)) {
                        total += 1;
                    }
                }
            }
            total
        }

        #[test]
        fn water_response_updates_view_state_immediately() {
            let mut st = stable_state();
            st.water = 41.0;
            apply_local_command_result(&mut st, "water", "Water: 63%\n");
            assert_eq!(st.water, 63.0);

            apply_local_command_result(&mut st, "water", "Water already high: 100%\n");
            assert_eq!(st.water, 100.0);
        }

        #[test]
        fn prune_response_updates_authoritative_counter_locally() {
            let mut st = stable_state();
            st.prune_left = 1;
            apply_local_command_result(&mut st, "prune left", "Pruned left: 4\n");
            assert_eq!(st.prune_left, 4);
        }

        #[test]
        fn left_prune_removes_visible_left_canopy() {
            let mut before = stable_state();
            before.prune_left = 0;
            let plain = grow_tree(&before, 68, 28);
            let center = plain.w / 2;
            let plain_left = tree_cells_in_region(&plain, 0, center - 3, 0, plain.h - 9);

            let mut after = before.clone();
            after.prune_left = 1;
            let pruned = grow_tree(&after, 68, 28);
            let pruned_left = tree_cells_in_region(&pruned, 0, center - 3, 0, pruned.h - 9);
            assert!(pruned_left < plain_left, "left prune did not remove visible canopy");
        }

        #[test]
        fn right_prune_removes_visible_right_canopy() {
            let mut before = stable_state();
            before.prune_right = 0;
            let plain = grow_tree(&before, 68, 28);
            let center = plain.w / 2;
            let plain_right = tree_cells_in_region(&plain, center + 4, plain.w, 0, plain.h - 9);

            let mut after = before.clone();
            after.prune_right = 1;
            let pruned = grow_tree(&after, 68, 28);
            let pruned_right = tree_cells_in_region(&pruned, center + 4, pruned.w, 0, pruned.h - 9);
            assert!(pruned_right < plain_right, "right prune did not remove visible canopy");
        }

        #[test]
        fn top_prune_removes_visible_top_canopy() {
            let mut before = stable_state();
            before.prune_top = 0;
            let plain = grow_tree(&before, 68, 28);
            let plain_top = tree_cells_in_region(&plain, 0, plain.w, 0, 9);

            let mut after = before.clone();
            after.prune_top = 1;
            let pruned = grow_tree(&after, 68, 28);
            let pruned_top = tree_cells_in_region(&pruned, 0, pruned.w, 0, 9);
            assert!(pruned_top < plain_top, "top prune did not remove visible canopy");
        }
'''
needle = "    }\n}"
pos = t.rfind(needle)
if pos < 0:
    raise SystemExit("could not find test module end")
t = t[:pos] + insert + t[pos:]
TESTS.write_text(t)
