from pathlib import Path

main = Path('src/main.rs')
tests = Path('tests/unit.rs')
cargo = Path('Cargo.toml')
lock = Path('Cargo.lock')

s = main.read_text()
s = s.replace('const VERSION: &str = "0.3.1";', 'const VERSION: &str = "0.3.2";', 1)

old_tri = '''fn triangular(x: f32, center: f32, width: f32) -> f32 {
    (1.0 - ((x - center).abs() / width.max(1.0))).clamp(0.0, 1.0)
}
'''
new_score = '''fn availability_score(value: f32, floor: f32, full: f32) -> f32 {
    if full <= floor {
        return f32::from(value >= full);
    }
    ((value - floor) / (full - floor)).clamp(0.0, 1.0)
}
'''
if old_tri not in s:
    raise SystemExit('triangular helper not found')
s = s.replace(old_tri, new_score, 1)

old_health = '''        let water_score = triangular(self.water, 58.0, 55.0);
        let light_score = triangular(self.light, 70.0, 65.0);
        let stress_penalty = ((self.drought_stress + self.wet_stress) / 90.0).min(0.45);
        let comfort = (water_score * 0.58 + light_score * 0.42 - stress_penalty).clamp(0.0, 1.0);
'''
new_health = '''        // Water and light are resources: once there is enough of either,
        // availability stays saturated instead of falling again near 100%.
        // Harm from excess water is modeled separately through wet_stress,
        // which depends on how long the soil remains saturated.
        let water_score = availability_score(self.water, 10.0, 45.0);
        let light_score = availability_score(self.light, 15.0, 60.0);
        let stress_penalty = ((self.drought_stress + self.wet_stress) / 90.0).min(0.45);
        let comfort = (water_score * 0.58 + light_score * 0.42 - stress_penalty).clamp(0.0, 1.0);
'''
if old_health not in s:
    raise SystemExit('health score block not found')
s = s.replace(old_health, new_health, 1)
main.write_text(s)

t = tests.read_text()
old_test = '''        #[test]
        fn triangular_score_peaks_at_center_and_clamps() {
            assert!((triangular(50.0, 50.0, 20.0) - 1.0).abs() < f32::EPSILON);
            assert!((triangular(30.0, 50.0, 20.0) - 0.0).abs() < f32::EPSILON);
            assert!((triangular(70.0, 50.0, 20.0) - 0.0).abs() < f32::EPSILON);
            assert!((triangular(-100.0, 50.0, 20.0) - 0.0).abs() < f32::EPSILON);
        }
'''
new_test = '''        #[test]
        fn availability_score_saturates_instead_of_penalizing_high_values() {
            assert!((availability_score(10.0, 10.0, 45.0) - 0.0).abs() < f32::EPSILON);
            assert!((availability_score(45.0, 10.0, 45.0) - 1.0).abs() < f32::EPSILON);
            assert!((availability_score(99.0, 10.0, 45.0) - 1.0).abs() < f32::EPSILON);
            assert!((availability_score(-100.0, 10.0, 45.0) - 0.0).abs() < f32::EPSILON);
        }

        #[test]
        fn high_water_and_light_without_existing_stress_recover_health() {
            let mut st = stable_state();
            st.health = 80.0;
            st.water = 99.0;
            st.light = 99.0;
            st.drought_stress = 0.0;
            st.wet_stress = 0.0;
            st.last_tick = 1_000;

            st.advance_to(4_600);

            assert!(st.health > 80.0, "health should recover at 99/99 resources: {}", st.health);
        }
'''
if old_test not in t:
    raise SystemExit('triangular test not found')
t = t.replace(old_test, new_test, 1)
tests.write_text(t)

c = cargo.read_text().replace('version = "0.3.1"', 'version = "0.3.2"', 1)
cargo.write_text(c)

l = lock.read_text().replace('version = "0.3.1"', 'version = "0.3.2"', 1)
lock.write_text(l)
