from pathlib import Path

main = Path("src/main.rs")
tests = Path("tests/unit.rs")
cargo = Path("Cargo.toml")
lock = Path("Cargo.lock")

s = main.read_text()
s = s.replace('const VERSION: &str = "0.3.0";', 'const VERSION: &str = "0.3.1";', 1)
s = s.replace(
    '''        "water" => {
            if st.water >= 96.0 {
                return format!("Water already high: {:.0}%\\n", st.water);
            }
            st.water = (st.water + 22.0).min(100.0);
''',
    '''        "water" => {
            // Match the rounded percentage shown by the UI. Values below 99.5%
            // are still visibly below 100%, so watering must be allowed to top
            // the reservoir off instead of trapping it around 96-99%.
            if st.water >= 99.5 {
                return format!("Water already high: {:.0}%\\n", st.water);
            }
            st.water = (st.water + 22.0).min(100.0);
''',
    1,
)
if 'if st.water >= 99.5' not in s:
    raise SystemExit('water threshold patch did not apply')
main.write_text(s)

t = tests.read_text()
needle = '''        #[test]
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
'''
replacement = '''        #[test]
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
        fn watering_from_ninety_seven_reaches_one_hundred() {
            let mut st = stable_state();
            st.water = 97.0;
            let mut stop = false;

            let response = handle_command(&mut st, "water", &mut stop);

            assert!((st.water - 100.0).abs() < f32::EPSILON);
            assert_eq!(response, "Water: 100%\\n");
            assert!(!stop);
        }

        #[test]
        fn rounded_full_water_does_not_overwater() {
            let mut st = stable_state();
            st.water = 99.6;
            let mut stop = false;

            let response = handle_command(&mut st, "water", &mut stop);

            assert!((st.water - 99.6).abs() < 0.001);
            assert_eq!(response, "Water already high: 100%\\n");
            assert!(!stop);
        }
'''
if needle not in t:
    raise SystemExit('watering unit test block not found')
tests.write_text(t.replace(needle, replacement, 1))

c = cargo.read_text()
c = c.replace('version = "0.3.0"', 'version = "0.3.1"', 1)
cargo.write_text(c)

l = lock.read_text()
l = l.replace('version = "0.3.0"', 'version = "0.3.1"', 1)
lock.write_text(l)
