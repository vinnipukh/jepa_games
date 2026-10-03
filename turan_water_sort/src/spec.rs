//! Strategy specs: the text form of a [`TuranStrategy`], as the bindings (Python, web) and the
//! generator's own `variant()` string write it.

use water_sort_core::Layout;

use crate::TuranStrategy;

/// A strategy spec that cannot be parsed. The message names the problem.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct StrategySpecError(pub String);

/// Parses a Turan strategy: a name (`reverse_search`, `scramble`, `pour_walk`, `constrained`;
/// `-` may replace `_`), optionally followed by `(key=value,...)` as in the generator's
/// `variant()` string, e.g. `"scramble(steps=40)"` or
/// `"reverse_search(max_depth=300,max_states=10000,layout=standard)"`. Missing arguments take
/// their defaults; a `layout` argument must match `layout`.
///
/// # Errors
///
/// Unknown names or arguments, malformed values, or a `layout` argument other than `layout`.
pub fn parse_strategy(spec: &str, layout: Layout) -> Result<TuranStrategy, StrategySpecError> {
    let err = |msg: String| StrategySpecError(msg);
    let spec = spec.trim();
    let (name, args) = match spec.split_once('(') {
        Some((name, rest)) => {
            let args = rest
                .strip_suffix(')')
                .ok_or_else(|| err(format!("strategy {spec:?}: missing ')'")))?;
            (name.trim(), args)
        }
        None => (spec, ""),
    };
    let mut kv: Vec<(&str, &str)> = Vec::new();
    for part in args.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (k, v) = part
            .split_once('=')
            .ok_or_else(|| err(format!("strategy {spec:?}: expected key=value")))?;
        kv.push((k.trim(), v.trim()));
    }
    let mut take = |key: &str, default: u32| -> Result<u32, StrategySpecError> {
        match kv.iter().position(|(k, _)| *k == key) {
            Some(i) => {
                let (_, v) = kv.remove(i);
                v.parse()
                    .map_err(|_| err(format!("strategy {spec:?}: bad value for {key}")))
            }
            None => Ok(default),
        }
    };
    let strategy = match name.replace('-', "_").as_str() {
        "reverse_search" | "search" => TuranStrategy::ReverseSearch {
            max_depth: take("max_depth", TuranStrategy::DEFAULT_SEARCH_DEPTH)?,
            max_states: take("max_states", TuranStrategy::DEFAULT_SEARCH_STATES)?,
        },
        "scramble" => TuranStrategy::Scramble {
            steps: take("steps", TuranStrategy::DEFAULT_STEPS)?,
            max_extra_steps: take("max_extra_steps", TuranStrategy::DEFAULT_MAX_EXTRA_STEPS)?,
        },
        "pour_walk" | "walk" => TuranStrategy::PourWalk {
            steps: take("steps", TuranStrategy::DEFAULT_WALK_STEPS)?,
        },
        "constrained" => TuranStrategy::Constrained,
        other => {
            return Err(err(format!(
                "unknown strategy {other:?} (expected reverse_search, scramble, pour_walk or \
                 constrained)"
            )));
        }
    };
    for (k, v) in kv {
        if k == "layout" {
            let given: Layout = v
                .parse()
                .map_err(|e: water_sort_core::UnknownLayout| err(e.to_string()))?;
            if given != layout {
                return Err(err(format!(
                    "strategy {spec:?} names layout {given}, but the generator uses {layout}"
                )));
            }
        } else {
            return Err(err(format!("strategy {spec:?}: unknown argument {k:?}")));
        }
    }
    Ok(strategy)
}

#[cfg(test)]
mod tests {
    use water_sort_core::Generator;

    use super::*;
    use crate::Turan;

    #[test]
    fn variant_strings_read_back() {
        let strategies = [
            TuranStrategy::default(),
            TuranStrategy::scramble(17),
            TuranStrategy::DEFAULT_POUR_WALK,
            TuranStrategy::Constrained,
        ];
        for layout in Layout::ALL {
            for s in strategies {
                let variant = Turan::new(s, layout).variant();
                assert_eq!(parse_strategy(&variant, layout), Ok(s), "{variant}");
            }
        }
    }

    #[test]
    fn names_and_defaults() {
        let l = Layout::Standard;
        assert_eq!(
            parse_strategy("scramble", l),
            Ok(TuranStrategy::scramble(40))
        );
        assert_eq!(
            parse_strategy(" pour-walk(steps=9) ", l),
            Ok(TuranStrategy::PourWalk { steps: 9 })
        );
        assert_eq!(parse_strategy("search", l), Ok(TuranStrategy::default()));
    }

    #[test]
    fn errors() {
        let l = Layout::Standard;
        for bad in [
            "nope",
            "scramble(steps=40",
            "scramble(steps)",
            "scramble(steps=x)",
            "scramble(foo=1)",
            "constrained(layout=distributed)",
            "constrained(layout=diagonal)",
        ] {
            assert!(parse_strategy(bad, l).is_err(), "{bad}");
        }
    }
}
