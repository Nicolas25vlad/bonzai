mod app {
    #![allow(dead_code)]

    include!("../src/main.rs");

    #[cfg(test)]
    mod unit {
        use super::*;

        fn stable_state() -> State {
            let mut st = State::new();
            let now = now_secs();
            st.seed = 0x000B_05A1;
            st.born_at = now.saturating_sub(1_000);
            st.last_tick = now;
            st.water = 72.0;
            st.light = 68.0;
            st.health = 94.0;
            st.growth = 38.0;
            st.light_dir = 0;
            st.prune_left = 0;
            st.prune_right = 0;
            st.prune_top = 0;
            st.light_left_hours = 2.0;
            st.light_center_hours = 5.0;
            st.light_right_hours = 8.0;
            st.drought_stress = 0.0;
            st.wet_stress = 0.0;
            st
        }

        #[test]
        fn triangular_score_peaks_at_center_and_clamps() {
            assert!((triangular(50.0, 50.0, 20.0) - 1.0).abs() < f32::EPSILON);
            assert!((triangular(30.0, 50.0, 20.0) - 0.0).abs() < f32::EPSILON);
            assert!((triangular(70.0, 50.0, 20.0) - 0.0).abs() < f32::EPSILON);
            assert!((triangular(-100.0, 50.0, 20.0) - 0.0).abs() < f32::EPSILON);
        }

        #[test]
        fn state_round_trip_preserves_persistent_fields() {
            let st = stable_state();
            let parsed = State::parse(&st.serialize()).expect("serialized state should parse");

            assert_eq!(parsed.seed, st.seed);
            assert_eq!(parsed.born_at, st.born_at);
            assert_eq!(parsed.last_tick, st.last_tick);
            assert!((parsed.water - st.water).abs() < 0.001);
            assert!((parsed.light - st.light).abs() < 0.001);
            assert!((parsed.health - st.health).abs() < 0.001);
            assert!((parsed.growth - st.growth).abs() < 0.001);
            assert_eq!(parsed.light_dir, st.light_dir);
            assert_eq!(parsed.prune_left, st.prune_left);
            assert_eq!(parsed.prune_right, st.prune_right);
            assert_eq!(parsed.prune_top, st.prune_top);
        }

        #[test]
        fn old_state_files_receive_defaults_for_new_fields() {
            let parsed = State::parse("seed=42\nborn_at=100\nlast_tick=100\nwater=50\n")
                .expect("legacy state should parse");

            assert_eq!(parsed.seed, 42);
            assert_eq!(parsed.born_at, 100);
            assert_eq!(parsed.last_tick, 100);
            assert!((parsed.water - 50.0).abs() < 0.001);
            assert!((parsed.health - 100.0).abs() < 0.001);
            assert_eq!(parsed.prune_left, 0);
            assert_eq!(parsed.prune_right, 0);
            assert_eq!(parsed.prune_top, 0);
        }

        #[test]
        fn one_hour_of_simulation_keeps_core_values_bounded() {
            let mut st = stable_state();
            st.last_tick = 1_000;
            st.water = 72.0;
            st.light = 68.0;
            st.advance_to(4_600);

            assert!(st.water < 72.0);
            assert!(st.light < 68.0);
            assert!((0.0..=100.0).contains(&st.water));
            assert!((0.0..=100.0).contains(&st.light));
            assert!((0.0..=100.0).contains(&st.health));
            assert!((0.0..=100.0).contains(&st.growth));
        }

        #[test]
        fn photo_bias_points_toward_accumulated_light() {
            let mut st = stable_state();
            st.light_left_hours = 1.0;
            st.light_center_hours = 1.0;
            st.light_right_hours = 8.0;
            assert!(st.photo_bias() > 0.0);

            st.light_left_hours = 8.0;
            st.light_right_hours = 1.0;
            assert!(st.photo_bias() < 0.0);
        }

        #[test]
        fn watering_is_bounded_at_one_hundred_percent() {
            let mut st = stable_state();
            st.water = 90.0;
            let mut stop = false;

            handle_command(&mut st, "water", &mut stop);
            assert!((st.water - 100.0).abs() < f32::EPSILON);

            handle_command(&mut st, "water", &mut stop);
            assert!((st.water - 100.0).abs() < f32::EPSILON);
            assert!(!stop);
        }

        #[test]
        fn light_and_pruning_commands_change_only_expected_directional_state() {
            let mut st = stable_state();
            let mut stop = false;

            handle_command(&mut st, "light right", &mut stop);
            assert_eq!(st.light_dir, 1);

            handle_command(&mut st, "prune left", &mut stop);
            assert_eq!(st.prune_left, 1);
            assert_eq!(st.prune_right, 0);
            assert_eq!(st.prune_top, 0);
        }

        #[test]
        fn renderer_is_deterministic_for_the_same_state() {
            let st = stable_state();
            let a = grow_tree(&st, 58, 25);
            let b = grow_tree(&st, 58, 25);

            assert_eq!(a.w, b.w);
            assert_eq!(a.h, b.h);
            for y in 0..a.h {
                for x in 0..a.w {
                    let left = a.get(x, y);
                    let right = b.get(x, y);
                    assert_eq!(left.ch, right.ch, "glyph changed at ({x}, {y})");
                    assert_eq!(left.kind, right.kind, "style changed at ({x}, {y})");
                }
            }
        }

        #[test]
        fn progress_bar_clamps_out_of_range_values() {
            assert_eq!(bar(-10.0, 5), "─────");
            assert_eq!(bar(0.0, 5), "─────");
            assert_eq!(bar(100.0, 5), "━━━━━");
            assert_eq!(bar(150.0, 5), "━━━━━");
        }

        #[test]
        fn ansi_sequences_do_not_count_toward_visible_width() {
            let title = format!("{TITLE}bonzai{RESET}");
            assert_eq!(visible_width(&title), 6);

            let centered = center_line(&title, 10);
            assert!(centered.starts_with("  "));
            assert_eq!(visible_width(&centered), 8);
        }

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
            assert!(!painted.contains("\x1b[2J"));
            assert!(painted.contains("\x1b[E"));
            assert!(painted.ends_with("\x1b[J"));
        }

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
                        !matches!(canvas.get(x, y).kind, 2..=4),
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
                        !matches!(cell.kind, 1..=5),
                        "tree cell visible through planter at ({x}, {y})"
                    );
                }
            }
        }

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
            assert!(
                pruned_left < plain_left,
                "left prune did not remove visible canopy"
            );
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
            assert!(
                pruned_right < plain_right,
                "right prune did not remove visible canopy"
            );
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
            assert!(
                pruned_top < plain_top,
                "top prune did not remove visible canopy"
            );
        }
    }
}
