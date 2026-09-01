from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"missing expected block: {label}")
    return text.replace(old, new, 1)


def splice(text: str, start: str, end: str, replacement: str, label: str) -> str:
    try:
        a = text.index(start)
        b = text.index(end, a)
    except ValueError as exc:
        raise SystemExit(f"could not locate {label}") from exc
    return text[:a] + replacement + text[b:]


main = Path("src/main.rs")
s = main.read_text()

s = replace_once(s, 'const VERSION: &str = "0.2.4";', 'const VERSION: &str = "0.2.5";', "version")
s = replace_once(
    s,
    'const ACTION_COOLDOWN_MS: u64 = 850;',
    'const ACTION_COOLDOWN_MS: u64 = 420;',
    "action cooldown",
)
s = replace_once(
    s,
    '    process::{Command, Stdio},\n    thread,',
    '    process::{Command, Stdio},\n    sync::OnceLock,\n    thread,',
    "OnceLock import",
)
s = replace_once(
    s,
    'const ACTION_COOLDOWN_MS: u64 = 420;\n',
    'const ACTION_COOLDOWN_MS: u64 = 420;\n\nstatic TERMINAL_SIZE: OnceLock<(usize, usize)> = OnceLock::new();\n',
    "terminal size cache",
)

s = replace_once(
    s,
    '''                let response = handle_command(&mut st, line.trim(), &mut should_stop);
                save_state(&st)?;
                // A health probe or client may disconnect before reading the reply.
                // That must never terminate the authoritative daemon.
                let _ = stream.write_all(response.as_bytes());''',
    '''                let command = line.trim();
                let response = handle_command(&mut st, command, &mut should_stop);
                // Read-only probes are frequent while the viewer is open. Persist only
                // mutations; elapsed time is still checkpointed by the periodic save.
                if !matches!(command, "ping" | "snapshot") {
                    save_state(&st)?;
                }
                // A client may disconnect before reading the reply. That must never
                // terminate the authoritative daemon.
                let _ = stream.write_all(response.as_bytes());''',
    "daemon read-only persistence",
)

leaf_spray = r'''fn draw_leaf_spray(
    c: &mut Canvas,
    rng: &mut Rng,
    x: i32,
    y: i32,
    vigor: f32,
    tropism: f32,
) {
    let pull = if tropism > 0.25 {
        1
    } else if tropism < -0.25 {
        -1
    } else {
        0
    };
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

            for col in 0..row_width {
                if rng.chance(density, 100) {
                    let bright = rng.chance((10.0 + vigor * 18.0) as u64, 100);
                    c.set(start_x + col, row_y, '&', if bright { 3 } else { 2 });
                }
            }
        }
    }
}

'''
s = splice(s, "fn draw_leaf_spray(", "fn draw_cb_branch(", leaf_spray, "leaf spray")

s = replace_once(
    s,
    "fn draw_cb_branch(\n",
    "// Recursive growth carries explicit branch state. The flat signature makes\n// each recursive transition visible and auditable.\n#[allow(clippy::too_many_arguments)]\nfn draw_cb_branch(\n",
    "branch walker lint annotation",
)
s = replace_once(
    s,
    '        let branch_gate = ((12.0 + vigor * 17.0) as u64).min(34);',
    '        let branch_gate = ((16.0 + vigor * 20.0) as u64).min(38);',
    "branch density",
)

old_side = '''            let side = if prefer_right && rng.chance(63, 100) {
                1
            } else if prefer_left && rng.chance(63, 100) {
                -1
            } else if rng.chance(1, 2) {
                -1
            } else {
                1
            };'''
new_side = '''            let preferred_side = if prefer_right {
                Some(1)
            } else if prefer_left {
                Some(-1)
            } else {
                None
            };
            let side = match preferred_side {
                Some(side) if rng.chance(63, 100) => side,
                _ if rng.chance(1, 2) => -1,
                _ => 1,
            };'''
s = replace_once(s, old_side, new_side, "branch side selection")

old_pot = '''    // Classic compact cbonsai-style planter.
    let pot = ["(---./~~~\\\\.---)", " (           ) ", "  (_________)  "];
    for (i, row) in pot.iter().enumerate() {
        let sx = base_x - row.chars().count() as i32 / 2;
        for (j, ch) in row.chars().enumerate() {
            if ch != ' ' {
                let kind = if i == 0 && matches!(ch, '~' | '.' | '/' | '\\\\') {
                    1
                } else if i == 0 && ch == '-' {
                    3
                } else {
                    7
                };
                c.set(sx + j as i32, base_y + i as i32, ch, kind);
            }
        }
    }'''
new_pot = '''    // Wide, quiet planter inspired by cbonsai's classic terminal silhouette.
    let pot = [
        ("      .-----------------.      ", 2u8),
        ("       \\               /       ", 7u8),
        ("        \\_____________/        ", 7u8),
        ("        (_)         (_)        ", 7u8),
    ];
    for (i, (row, kind)) in pot.iter().enumerate() {
        let sx = base_x - row.chars().count() as i32 / 2;
        for (j, ch) in row.chars().enumerate() {
            if ch != ' ' {
                c.set(sx + j as i32, base_y + i as i32, ch, *kind);
            }
        }
    }'''
s = replace_once(s, old_pot, new_pot, "planter")

s = replace_once(
    s,
    'fn terminal_size() -> (usize, usize) {\n',
    '''fn terminal_size() -> (usize, usize) {
    *TERMINAL_SIZE.get_or_init(detect_terminal_size)
}

fn detect_terminal_size() -> (usize, usize) {
''',
    "terminal size detector",
)

s = replace_once(
    s,
    '    let w = 58;\n    let h = 25;',
    '    let w = 68;\n    let h = 28;',
    "viewer canvas size",
)
s = replace_once(
    s,
    '    out.push_str("\\x1b[?25l\\x1b[2J\\x1b[H");\n',
    '',
    "render clear sequence",
)

render_marker = '''fn render(st: &State) -> String {
    render_scene(st, None)
}
'''
paint_frame = '''fn render(st: &State) -> String {
    render_scene(st, None)
}

fn paint_frame(frame: &str) -> io::Result<()> {
    let mut out = String::with_capacity(frame.len() + 256);
    out.push_str("\\x1b[H");
    for line in frame.lines() {
        out.push_str(line);
        out.push_str("\\x1b[K\\n");
    }
    out.push_str("\\x1b[J");

    let mut stdout = io::stdout();
    stdout.write_all(out.as_bytes())?;
    stdout.flush()
}
'''
s = replace_once(s, render_marker, paint_frame, "frame painter")

animations = r'''fn animate_water(st: &State) -> io::Result<()> {
    for frame in 0..3 {
        paint_frame(&render_scene(st, Some(SceneEffect::Water(frame))))?;
        thread::sleep(Duration::from_millis(65));
    }
    paint_frame(&render(st))
}

fn animate_light(direction: &str, st: &State) -> io::Result<()> {
    let dir = match direction {
        "left" => -1,
        "right" => 1,
        _ => 0,
    };
    for frame in 0..2 {
        paint_frame(&render_scene(st, Some(SceneEffect::Light(dir, frame))))?;
        thread::sleep(Duration::from_millis(75));
    }
    paint_frame(&render(st))
}

fn animate_prune(side: &str, st: &State) -> io::Result<()> {
    let side = match side {
        "left" => -1,
        "right" => 1,
        _ => 0,
    };
    for frame in 0..2 {
        paint_frame(&render_scene(st, Some(SceneEffect::Prune(side, frame))))?;
        thread::sleep(Duration::from_millis(70));
    }
    paint_frame(&render(st))
}

'''
s = splice(s, "fn animate_water(", "fn watch_help(", animations, "inline animations")

s = replace_once(
    s,
    '    out.push_str("\\x1b[2J\\x1b[H");\n',
    '',
    "help screen clear",
)

watch = r'''fn watch() -> io::Result<()> {
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
                        let _ = send("water")?;
                        st = get_snapshot();
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
                        let _ = send(&format!("light {direction}"))?;
                        st = get_snapshot();
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
                        let _ = send(&format!("prune {side}"))?;
                        st = get_snapshot();
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

'''
s = splice(s, "fn drain_input()", "fn start_daemon()", watch, "watch loop")

old_memory = '''    println!(
        "{SOIL}memory{RESET} L {:.1}h  C {:.1}h  R {:.1}h",
        st.light_left_hours, st.light_center_hours, st.light_right_hours
    );'''
new_memory = '''    println!(
        "{SOIL}memory{RESET} L {:.1}h  C {:.1}h  R {:.1}h  ·  total {:.1}h",
        st.light_left_hours,
        st.light_center_hours,
        st.light_right_hours,
        st.total_light_hours()
    );'''
s = replace_once(s, old_memory, new_memory, "status light memory")

s = replace_once(
    s,
    '''{TITLE}INFO{RESET}\\
  help, -h, --help         Show this help\\
  version, -V, --version   Print the version\\''',
    '''{TITLE}INFO{RESET}\\
  update                   Download, build and install the latest version\\
  help, -h, --help         Show this help\\
  version, -V, --version   Print the version\\''',
    "usage update command",
)

s = s.replace('    print!("\\x1b[?25h\\x1b[0m\\n");', '    println!("\\x1b[?25h\\x1b[0m");', 1)

main.write_text(s)

cargo = Path("Cargo.toml")
cargo_text = cargo.read_text()
cargo_text = replace_once(cargo_text, 'version = "0.2.4"', 'version = "0.2.5"', "Cargo version")
cargo.write_text(cargo_text)

lock = Path("Cargo.lock")
lock_text = lock.read_text()
lock_text = replace_once(lock_text, 'version = "0.2.4"', 'version = "0.2.5"', "lock version")
lock.write_text(lock_text)
