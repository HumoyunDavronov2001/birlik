//! 1,000 scenarios of crashes, restarts, partitions and lost and late messages (PLAN.md §8, SPEC NO-50); see the
//! crate's documentation for what each checks. `OMNARC_SCENARIOS` runs another number, `OMNARC_FIRST_SEED` starts at
//! another seed — to replay a failing one.

#[test]
fn the_agreed_state_survives_every_scenario() {
    let number = |name: &str, default: u64| std::env::var(name).ok().and_then(|n| n.parse().ok()).unwrap_or(default);
    let count = number("OMNARC_SCENARIOS", 1000);
    let first = number("OMNARC_FIRST_SEED", 0);
    let started = std::time::Instant::now();
    let (mut faults, mut committed, mut failures) = (0, 0, Vec::new());
    for seed in first..first + count {
        match omnarc_simulation::run(seed) {
            Ok(outcome) => {
                faults += outcome.faults;
                committed += outcome.committed;
            }
            Err(failure) => failures.push(failure),
        }
    }
    eprintln!(
        "{count} scenarios (seeds {first}..{}): {faults} faults injected, {committed} changes committed, {:.0?}",
        first + count,
        started.elapsed()
    );
    assert!(failures.is_empty(), "{} of {count} scenarios failed:\n{}", failures.len(), failures.join("\n"));
    assert!(committed > 0, "no change was committed: the scenarios proved nothing");
}
