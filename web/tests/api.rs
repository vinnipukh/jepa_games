//! The web API on wasm32: Phase 1.5 counting, stars, seeds and error paths.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::wasm_bindgen_test;
use water_sort_web::{
    JsParams as Params, Puzzle, Session, from_code, generate, star_moves, stars, strategies,
    variant, web_config_json,
};

fn uniform_3x3(seed: &str) -> Puzzle {
    generate(
        "uniform",
        &Params::new(3, 3, 1),
        Some(seed.into()),
        None,
        None,
        None,
    )
    .unwrap()
}

fn play(session: &mut Session, actions: &[u16]) {
    let n = u16::try_from(session.n_tubes()).unwrap();
    for &a in actions {
        assert!(session.pour(u8::try_from(a / n).unwrap(), u8::try_from(a % n).unwrap()) > 0);
    }
}

#[wasm_bindgen_test]
fn undo_and_restart_keep_the_counter() {
    let puzzle = uniform_3x3("0");
    let solution = puzzle.solution().unwrap();
    let opt = puzzle.opt_moves().unwrap();
    let mut s = Session::new(&puzzle);
    assert!(!s.undo());
    assert_eq!(s.pour(0, 0), 0, "same tube is illegal");
    assert_eq!(s.moves_counted(), 0, "illegal pours are not counted");
    play(&mut s, &solution[..2]);
    assert!(s.undo());
    assert_eq!(s.moves_counted(), 2, "undo does not decrement");
    s.restart();
    assert!(!s.can_undo());
    assert_eq!(s.cells(), puzzle.cells());
    assert_eq!(s.moves_counted(), 2, "restart keeps the counter");
    assert_eq!(s.stars().unwrap(), None);
    play(&mut s, &solution);
    assert!(s.is_solved());
    assert_eq!(s.moves_counted(), opt + 2);
    assert_eq!(s.stars().unwrap(), Some(stars(opt + 2, opt).unwrap()));
}

#[wasm_bindgen_test]
fn optimal_play_earns_five_stars() {
    let puzzle = uniform_3x3("2a");
    let mut s = Session::new(&puzzle);
    play(&mut s, &puzzle.solution().unwrap());
    assert_eq!(s.stars().unwrap(), Some(5));
}

#[wasm_bindgen_test]
fn star_boundaries() {
    // opt 30: thresholds (3, 8, 15) (core's integer ceil).
    assert_eq!(star_moves(30), vec![30, 33, 38, 45]);
    for (moves, expected) in [
        (30, 5),
        (33, 4),
        (34, 3),
        (38, 3),
        (39, 2),
        (45, 2),
        (46, 1),
    ] {
        assert_eq!(stars(moves, 30).unwrap(), expected, "{moves}");
    }
    assert!(stars(29, 30).is_err());
}

#[wasm_bindgen_test]
fn seeds_are_hex_and_reproducible() {
    let a = uniform_3x3("000000000000002A");
    let b = uniform_3x3("0x2a");
    assert_eq!(a.seed().unwrap(), "000000000000002a");
    assert_eq!(a.puzzle_code(), b.puzzle_code());
    let max = uniform_3x3("ffffffffffffffff");
    assert_eq!(max.seed().unwrap(), "ffffffffffffffff");
    for bad in ["xyz", "12345678901234567", "-1"] {
        assert!(
            generate(
                "uniform",
                &Params::new(3, 3, 1),
                Some(bad.into()),
                None,
                None,
                None
            )
            .is_err(),
            "{bad}"
        );
    }
    // Fresh seeds are recorded, so the puzzle can be reopened.
    let fresh = generate("turan", &Params::new(4, 4, 2), None, None, None, None).unwrap();
    let again = generate(
        "turan",
        &Params::new(4, 4, 2),
        fresh.seed(),
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(fresh.puzzle_code(), again.puzzle_code());
}

#[wasm_bindgen_test]
fn tiers_and_layouts() {
    let p = Params::new(6, 4, 2);
    for tier in ["easy", "medium", "hard"] {
        let z = generate(
            "uniform",
            &p,
            Some("7".into()),
            None,
            Some("distributed".into()),
            Some(tier.into()),
        )
        .unwrap();
        assert_eq!(z.tier().as_deref(), Some(tier));
        assert_eq!(z.layout(), "distributed");
        let cfg = web_config_json(&p, Some("distributed".into()), Some(tier.into())).unwrap();
        assert_eq!(z.config_json().unwrap(), cfg);
    }
    assert!(generate("uniform", &p, None, None, None, Some("impossible".into())).is_err());
    assert!(generate("turan", &p, None, None, None, Some("hard".into())).is_err());
    assert!(generate("turan", &p, None, None, None, Some("any".into())).is_ok());
    assert!(generate("turan", &p, None, Some("pour_walk".into()), None, None).is_err());
    assert_eq!(strategies("standard").unwrap()[0], "reverse_search");
    assert_eq!(
        variant("turan", None, None).unwrap(),
        "reverse_search(max_depth=300,max_states=10000,layout=standard)"
    );
}

#[wasm_bindgen_test]
fn unsupported_params_and_bad_names_are_rejected() {
    assert!(generate("uniform", &Params::new(13, 3, 1), None, None, None, None).is_err());
    assert!(generate("uniform", &Params::new(3, 3, 0), None, None, None, None).is_err());
    assert!(generate("nope", &Params::new(3, 3, 1), None, None, None, None).is_err());
    assert!(
        generate(
            "uniform",
            &Params::new(3, 3, 1),
            None,
            Some("scramble".into()),
            None,
            None
        )
        .is_err()
    );
    assert!(
        generate(
            "uniform",
            &Params::new(3, 3, 1),
            None,
            None,
            Some("diagonal".into()),
            None
        )
        .is_err()
    );
}

#[wasm_bindgen_test]
fn codes_open_and_solve() {
    let z = uniform_3x3("5");
    let opened = from_code(&z.puzzle_code().to_lowercase(), None).unwrap();
    assert_eq!(opened.puzzle_code(), z.puzzle_code());
    assert_eq!(opened.opt_moves(), z.opt_moves());
    assert_eq!(opened.canonical_hash(), z.canonical_hash());
    assert_eq!(opened.generator_id(), None);
    assert!(opened.puzzle_id().starts_with("unknown:"));
    assert!(from_code("not a code", None).is_err());
    // A budget of one state is not enough: the optimum is unknown, play is still possible.
    let starved = from_code(&z.puzzle_code(), Some(1)).unwrap();
    assert_eq!(starved.solver_status(), "timeout");
    assert_eq!(starved.opt_moves(), None);
    assert_eq!(Session::new(&starved).stars().unwrap(), None);
}

#[wasm_bindgen_test]
fn tampered_json_is_rejected() {
    let z = uniform_3x3("9");
    let json = z.to_json().unwrap();
    assert!(Puzzle::from_json(&json).is_ok());
    let hash = z.canonical_hash();
    assert!(Puzzle::from_json(&json.replace(&hash, "0000000000000000")).is_err());
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    v["solution"].as_array_mut().unwrap().pop();
    assert!(Puzzle::from_json(&v.to_string()).is_err());
}

#[wasm_bindgen_test]
fn trajectory_export_records_every_event() {
    let puzzle = uniform_3x3("3");
    let solution = puzzle.solution().unwrap();
    let mut s = Session::new(&puzzle);
    assert_eq!(s.pour(0, 0), 0); // illegal
    assert_eq!(s.pour(9, 0), 0); // out of range: not a recordable action
    play(&mut s, &solution[..1]);
    assert!(s.undo());
    s.restart();
    play(&mut s, &solution);
    assert_eq!(s.n_events(), 4 + solution.len());
    let v: serde_json::Value = serde_json::from_str(&s.export_trajectory().unwrap()).unwrap();
    assert_eq!(v["format"], water_sort_web::TRAJECTORY_FORMAT);
    assert_eq!(v["source"], "human");
    assert_eq!(v["session_id"].as_str().unwrap().len(), 16);
    assert_eq!(v["puzzle"]["puzzle_code"], puzzle.puzzle_code());
    assert_eq!(v["puzzle"]["seed"], "0000000000000003");
    assert_eq!(v["puzzle"]["puzzle_id"], puzzle.puzzle_id());
    assert_eq!(v["moves_counted"], solution.len() + 1);
    assert_eq!(v["solved"], true);
    let rows = v["rows"].as_array().unwrap();
    let events: Vec<&str> = rows.iter().map(|r| r["event"].as_str().unwrap()).collect();
    assert_eq!(&events[..4], ["illegal", "pour", "undo", "restart"]);
    assert!(events[4..].iter().all(|e| *e == "pour"));
    let cells = |r: &serde_json::Value, k: &str| -> Vec<u8> {
        serde_json::from_value(r[k].clone()).unwrap()
    };
    assert_eq!(cells(&rows[0], "state"), puzzle.cells());
    assert_eq!(cells(&rows[0], "state"), cells(&rows[0], "next_state"));
    assert_eq!(rows[0]["illegal"], true);
    assert_eq!(rows[1]["action"], solution[0]);
    assert!(rows[2]["action"].is_null());
    assert_eq!(cells(&rows[2], "next_state"), puzzle.cells());
    for (i, w) in rows.windows(2).enumerate() {
        assert_eq!(cells(&w[0], "next_state"), cells(&w[1], "state"), "row {i}");
        assert_eq!(w[1]["step"], i + 1);
    }
    let last = rows.last().unwrap();
    assert_eq!(
        (last["solved"].clone(), last["done"].clone()),
        (true.into(), true.into())
    );
    assert_eq!(last["moves_counted"], solution.len() + 1);
}
