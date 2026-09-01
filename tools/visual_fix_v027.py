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
s = replace_once(s, 'const VERSION: &str = "0.2.6";', 'const VERSION: &str = "0.2.7";', "version")

s = replace_once(
    s,
    '''        BranchKind::Trunk => {
            let mut dx = if age < 8 {
                rng.range(-2, 3)
            } else {
                rng.range(-1, 2)
            };
            if light_pull != 0 && rng.chance(24, 100) {
                dx += light_pull;
            }
            let dy = if age < 5 {
                if rng.chance(30, 100) {
                    -1
                } else {
                    0
                }
            } else if rng.chance((58.0 + vigor * 26.0) as u64, 100) {
                -1
            } else {
                0
            };
            (dx.clamp(-2, 2), dy)
        }
''',
    '''        BranchKind::Trunk => {
            // Keep a readable stem above the planter before the trunk begins
            // wandering. Without this anchor young trees can look like foliage
            // floating directly over the pot.
            if age <= 4 {
                return (0, -1);
            }

            let mut dx = rng.range(-1, 2);
            if light_pull != 0 && rng.chance(24, 100) {
                dx += light_pull;
            }
            let dy = if rng.chance((62.0 + vigor * 24.0) as u64, 100) {
                -1
            } else {
                0
            };
            (dx.clamp(-2, 2), dy)
        }
''',
    "anchored trunk",
)

s = replace_once(
    s,
    '''            for col in 0..row_width {
                if rng.chance(density, 100) {
                    let bright = rng.chance((10.0 + vigor * 18.0) as u64, 100);
                    c.set(start_x + col, row_y, '&', if bright { 3 } else { 2 });
                }
            }
''',
    '''            // Keep foliage above a small visual buffer around the planter.
            // Branches may descend slightly, but leaves should never appear to
            // grow through the ceramic body.
            let foliage_floor = c.h - 9;
            if row_y >= foliage_floor {
                continue;
            }

            for col in 0..row_width {
                if rng.chance(density, 100) {
                    let bright = rng.chance((10.0 + vigor * 18.0) as u64, 100);
                    c.set(start_x + col, row_y, '&', if bright { 3 } else { 2 });
                }
            }
''',
    "foliage floor",
)

s = replace_once(
    s,
    '''        let glyph = branch_glyph(effective_kind, life, dx, dy);
''',
    '''        // Dying/dead branch tips render as `&`, so they must obey the
        // same lower foliage boundary as leaf sprays.
        if matches!(effective_kind, BranchKind::Dying | BranchKind::Dead) && y >= c.h - 9 {
            continue;
        }

        let glyph = branch_glyph(effective_kind, life, dx, dy);
''',
    "dying branch foliage floor",
)

s = replace_once(
    s,
    '''    draw_cb_branch(
        &mut c,
        &mut rng,
        base_x,
        base_y - 1,
''',
    '''    draw_cb_branch(
        &mut c,
        &mut rng,
        base_x,
        base_y,
''',
    "trunk starts at soil line",
)

s = replace_once(
    s,
    '''    for (i, (row, kind)) in pot.iter().enumerate() {
        let sx = base_x - row.chars().count() as i32 / 2;
        for (j, ch) in row.chars().enumerate() {
            if ch != ' ' {
                c.set(sx + j as i32, base_y + i as i32, ch, *kind);
            }
        }
    }
''',
    '''    for (i, (row, kind)) in pot.iter().enumerate() {
        let sx = base_x - row.chars().count() as i32 / 2;
        for (j, ch) in row.chars().enumerate() {
            // The planter is an opaque foreground object. Clearing spaces as
            // well as drawing its outline prevents branches or leaves from
            // showing through the inside of the pot.
            if ch == ' ' {
                c.set(sx + j as i32, base_y + i as i32, ' ', 0);
            } else {
                c.set(sx + j as i32, base_y + i as i32, ch, *kind);
            }
        }
    }
''',
    "opaque planter",
)

MAIN.write_text(s)

cargo = CARGO.read_text()
cargo = replace_once(cargo, 'version = "0.2.6"', 'version = "0.2.7"', "cargo version")
CARGO.write_text(cargo)

lock = LOCK.read_text()
lock = replace_once(lock, 'version = "0.2.6"', 'version = "0.2.7"', "lock version")
LOCK.write_text(lock)

t = TESTS.read_text()
insert = r'''

        #[test]
        fn trunk_has_a_visible_stem_above_the_planter() {
            let st = stable_state();
            let canvas = grow_tree(&st, 68, 28);
            let base_x = canvas.w / 2;
            let base_y = canvas.h - 5;

            for y in (base_y - 4)..base_y {
                let cell = canvas.get(base_x, y);
                assert!(
                    matches!(cell.kind, 1 | 5),
                    "expected trunk at ({base_x}, {y}), found {:?}",
                    (cell.ch, cell.kind)
                );
                assert_ne!(cell.ch, '&');
            }
        }

        #[test]
        fn foliage_never_reaches_the_planter_buffer() {
            let st = stable_state();
            let canvas = grow_tree(&st, 68, 28);
            let foliage_floor = canvas.h - 9;

            for y in foliage_floor..canvas.h {
                for x in 0..canvas.w {
                    assert!(
                        !matches!(canvas.get(x, y).kind, 2 | 3 | 4),
                        "foliage leaked into planter buffer at ({x}, {y})"
                    );
                }
            }
        }

        #[test]
        fn planter_body_is_opaque_to_tree_cells() {
            let st = stable_state();
            let canvas = grow_tree(&st, 68, 28);
            let base_y = canvas.h - 5;

            for y in base_y..canvas.h - 1 {
                for x in (canvas.w / 2 - 12)..=(canvas.w / 2 + 12) {
                    let cell = canvas.get(x, y);
                    assert!(
                        !matches!(cell.kind, 1 | 2 | 3 | 4 | 5),
                        "tree cell visible through planter at ({x}, {y})"
                    );
                }
            }
        }
'''
needle = "    }\n}"
pos = t.rfind(needle)
if pos < 0:
    raise SystemExit("could not find test module end")
t = t[:pos] + insert + t[pos:]
TESTS.write_text(t)
