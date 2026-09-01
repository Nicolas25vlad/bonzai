from pathlib import Path

MAIN = Path("src/main.rs")
TESTS = Path("tests/unit.rs")
CARGO = Path("Cargo.toml")
LOCK = Path("Cargo.lock")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise SystemExit(f"missing expected block: {label}")
    return text.replace(old, new, 1)


def splice(text: str, start: str, end: str, replacement: str, label: str) -> str:
    i = text.find(start)
    if i < 0:
        raise SystemExit(f"missing start marker: {label}")
    j = text.find(end, i)
    if j < 0:
        raise SystemExit(f"missing end marker: {label}")
    return text[:i] + replacement + text[j:]


s = MAIN.read_text()

s = replace_once(s, "    sync::OnceLock,\n", "", "OnceLock import")
s = replace_once(s, 'const VERSION: &str = "0.2.5";', 'const VERSION: &str = "0.2.6";', "version constant")
s = replace_once(
    s,
    "\nstatic TERMINAL_SIZE: OnceLock<(usize, usize)> = OnceLock::new();\n",
    "\n",
    "terminal size cache",
)

old_draw = '''fn draw_str(c: &mut Canvas, x: i32, y: i32, text: &str, kind: u8) {
    for (i, ch) in text.chars().enumerate() {
        if ch != ' ' {
            c.set(x + i as i32, y, ch, kind);
        }
    }
}
'''
new_draw = '''fn draw_str(c: &mut Canvas, x: i32, y: i32, text: &str, kind: u8) {
    let width = text.chars().count() as i32;
    let start_x = x - width / 2;
    for (i, ch) in text.chars().enumerate() {
        if ch != ' ' {
            c.set(start_x + i as i32, y, ch, kind);
        }
    }
}
'''
s = replace_once(s, old_draw, new_draw, "center branch glyphs")

s = replace_once(
    s,
    '("      .-----------------.      ", 2u8),',
    '("      .-----------------.      ", 7u8),',
    "planter rim color",
)

s = replace_once(
    s,
    '''fn terminal_size() -> (usize, usize) {
    *TERMINAL_SIZE.get_or_init(detect_terminal_size)
}
''',
    '''fn terminal_size() -> (usize, usize) {
    // The watch view lives for a long time, so terminal geometry cannot be cached.
    // Re-detecting here lets the layout recover after a resize instead of keeping
    // stale dimensions for the lifetime of the process.
    detect_terminal_size()
}
''',
    "dynamic terminal size",
)

render = r'''fn compact_scene(st: &State, rows: usize, cols: usize) -> String {
    let lines = [
        format!("{TITLE}bonzai{RESET}"),
        format!("{MUTED}{}{RESET}", mood(st)),
        String::new(),
        format!("{DIM}terminal too small for the tree{RESET}"),
        format!("{DIM}resize to at least 36 x 18{RESET}"),
        String::new(),
        format!("{WATER}water{RESET} {:>3.0}%  {SUN}light{RESET} {:>3.0}%  {HEALTH}health{RESET} {:>3.0}%", st.water, st.light, st.health),
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
            bar(st.water, 8), st.water, bar(st.light, 8), st.light,
        ));
        lines.push(format!(
            "{HEALTH}health{RESET} {} {:>3.0}%",
            bar(st.health, 10), st.health,
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
        lines.push(format!("{DIM}[w/r] water   [a/s/d] light   [h/?] help{RESET}"));
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

'''
s = splice(s, "fn render_scene(st: &State, effect: Option<SceneEffect>) -> String {", "fn render(st: &State) -> String {", render, "responsive renderer")

paint = r'''fn frame_escape(frame: &str) -> String {
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

'''
s = splice(s, "fn paint_frame(frame: &str) -> io::Result<()> {", "fn human_age(s: u64) -> String {", paint, "non-scrolling frame painter")

animations = r'''fn animate_water(st: &State) -> io::Result<()> {
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

'''
s = splice(s, "fn animate_water(st: &State) -> io::Result<()> {", "fn watch_help() -> String {", animations, "animation viewport stability")

help_fn = r'''fn watch_help() -> String {
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
        lines.push(format!("{MUTED}held keys are rate-limited to protect the tree{RESET}"));
        lines.push(format!("{MUTED}light direction is remembered by future growth{RESET}"));
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

'''
s = splice(s, "fn watch_help() -> String {", "fn watch() -> io::Result<()> {", help_fn, "responsive help")

MAIN.write_text(s)

cargo = CARGO.read_text()
cargo = replace_once(cargo, 'version = "0.2.5"', 'version = "0.2.6"', "Cargo version")
CARGO.write_text(cargo)

lock = LOCK.read_text()
lock = replace_once(lock, 'version = "0.2.5"', 'version = "0.2.6"', "lock version")
LOCK.write_text(lock)

t = TESTS.read_text()
insert = r'''
        #[test]
        fn branch_glyphs_are_centered_on_the_walker_position() {
            let mut canvas = Canvas::new(9, 3);
            draw_str(&mut canvas, 4, 1, "/|\\", 1);

            assert_eq!(canvas.get(3, 1).ch, '/');
            assert_eq!(canvas.get(4, 1).ch, '|');
            assert_eq!(canvas.get(5, 1).ch, '\\');
            assert_eq!(canvas.get(6, 1).ch, ' ');
        }

        #[test]
        fn responsive_renderer_stays_inside_requested_viewport() {
            let st = stable_state();
            for (rows, cols) in [(24usize, 52usize), (40, 100)] {
                let frame = render_scene_for_size(&st, None, rows, cols);
                let rendered: Vec<&str> = frame.lines().collect();
                assert!(rendered.len() <= rows, "frame exceeded {rows} rows");
                for line in rendered {
                    assert!(
                        visible_width(line) <= cols,
                        "line width {} exceeded {cols} columns",
                        visible_width(line)
                    );
                }
            }
        }

        #[test]
        fn tiny_terminal_falls_back_without_wrapping() {
            let st = stable_state();
            let frame = render_scene_for_size(&st, None, 14, 30);
            let rendered: Vec<&str> = frame.lines().collect();
            assert!(rendered.len() <= 14);
            assert!(rendered.iter().all(|line| visible_width(line) <= 30));
            assert!(frame.contains("terminal too small"));
        }

        #[test]
        fn frame_painter_never_emits_literal_newlines_or_full_clears() {
            let painted = frame_escape("alpha\nbeta\n");
            assert!(!painted.contains('\n'));
            assert!(!painted.contains("\\x1b[2J"));
            assert!(painted.contains("\\x1b[E"));
            assert!(painted.ends_with("\\x1b[J"));
        }
'''
needle = "    }\n}"
pos = t.rfind(needle)
if pos < 0:
    raise SystemExit("could not find unit test module closing braces")
t = t[:pos] + insert + t[pos:]
TESTS.write_text(t)
