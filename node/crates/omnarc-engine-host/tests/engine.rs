//! Drives a real `omnarc-engine` process. Built only with the `engine-tests` feature, and it needs:
//!
//! - `OMNARC_ENGINE`: the engine executable from the engine build, e.g. `build/engine/omnarc-engine`;
//! - `OMNARC_TEST_MODEL`: a small llama-architecture GGUF model, e.g. `stories15M-q8_0.gguf`.
//!
//! ```text
//! OMNARC_ENGINE=... OMNARC_TEST_MODEL=... cargo test -p omnarc-engine-host --features engine-tests
//! ```

use std::path::PathBuf;

use omnarc_engine_host::{Backend, DType, Engine, Error, Input, SamplerParams, Status, Step};

fn paths() -> (PathBuf, PathBuf) {
    let get = |name: &str| {
        PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name} to run the engine tests")))
    };
    (get("OMNARC_ENGINE"), get("OMNARC_TEST_MODEL"))
}

fn tokens_step(request: u64, position: u32, tokens: &[i32]) -> Step<'_> {
    Step { request, position, input: Input::Tokens(tokens), all_logits: false, output: DType::F32 }
}

fn argmax(values: &[f32]) -> usize {
    values.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map_or(0, |(i, _)| i)
}

#[test]
fn split_shards_match_the_whole_model() {
    let (exe, model) = paths();
    let mut engine = Engine::spawn(&exe, 64 << 20).unwrap();
    assert!(engine.abi_version() >= 2);

    let info = engine.model_info(&model).unwrap();
    assert!(info.splittable, "{info:?}");
    let n = info.layer_count;
    let whole = engine.open(&model, 0..n + 1, Backend::Cpu, 256).unwrap();
    let first = engine.open(&model, 0..n / 2, Backend::Cpu, 256).unwrap();
    let rest = engine.open(&model, n / 2..n, Backend::Cpu, 256).unwrap();
    let head = engine.open(&model, n..n + 1, Backend::Cpu, 256).unwrap();
    assert!(whole.takes_tokens() && whole.gives_logits());
    assert!(first.takes_tokens() && !first.gives_logits());
    assert!(!rest.takes_tokens() && !rest.gives_logits());
    assert!(!head.takes_tokens() && head.gives_logits());

    let prompt = engine.tokenize(whole.id, "Once upon a time, there was a little girl named", true).unwrap();
    assert!(prompt.len() > 5);

    // the prompt through the whole model, then through the three shards with the hidden state passed as f32
    let reference = engine.run(&whole, &tokens_step(1, 0, &prompt)).unwrap().to_f32();
    assert_eq!(reference.len(), info.vocab_size as usize);

    let count = u32::try_from(prompt.len()).unwrap();
    let hidden = engine.run(&first, &tokens_step(2, 0, &prompt)).unwrap().data.to_vec();
    let step = |data| Step {
        request: 2,
        position: 0,
        input: Input::Hidden { dtype: DType::F32, count, data },
        all_logits: false,
        output: DType::F32,
    };
    let hidden = engine.run(&rest, &step(&hidden)).unwrap().data.to_vec();
    let split = engine.run(&head, &step(&hidden)).unwrap().to_f32();
    assert_eq!(reference, split, "on the CPU the split run is bit for bit the whole run");

    // greedy generation: the whole model and the shards pick the same tokens; hidden states travel as f16
    let sampler = engine.sampler_open(SamplerParams::default()).unwrap();
    let mut next = i32::try_from(argmax(&reference)).unwrap();
    let mut text = String::new();
    for position in (count..).take(8) {
        let reference = engine.run(&whole, &tokens_step(1, position, &[next])).unwrap().to_f32();

        let hidden = engine
            .run(&first, &Step { output: DType::F16, ..tokens_step(2, position, &[next]) })
            .unwrap()
            .data
            .to_vec();
        let input = Input::Hidden { dtype: DType::F16, count: 1, data: &hidden };
        let step = Step { request: 2, position, input, all_logits: false, output: DType::F16 };
        let hidden = engine.run(&rest, &step).unwrap().data.to_vec();
        let input = Input::Hidden { dtype: DType::F16, count: 1, data: &hidden };
        engine.run(&head, &Step { input, ..step }).unwrap();
        let token = engine.sample(sampler, info.vocab_size).unwrap();

        assert_eq!(usize::try_from(token).unwrap(), argmax(&reference));
        text.push_str(&engine.token_text(whole.id, token).unwrap().0);
        next = token;
    }
    assert_ne!(text.trim(), "");

    for shard in [&whole, &first, &rest, &head] {
        engine.drop_request(shard.id, 1).unwrap();
        engine.drop_request(shard.id, 2).unwrap();
        engine.close(shard.id).unwrap();
    }
    engine.sampler_close(sampler).unwrap();
    engine.quit().unwrap();
}

#[test]
fn errors_are_reported() {
    let (exe, model) = paths();
    let mut engine = Engine::spawn(&exe, 1 << 20).unwrap();
    let n = engine.model_info(&model).unwrap().layer_count;

    let error = engine.open(&model, 0..n + 2, Backend::Cpu, 64).unwrap_err();
    assert!(matches!(error, Error::Engine { status: Status::InvalidArgument, .. }), "{error}");
    let error = engine.model_info(std::path::Path::new("no-such-model.gguf")).unwrap_err();
    assert!(matches!(error, Error::Engine { status: Status::Io, .. }), "{error}");

    // the engine keeps working after errors, and a crash shows up as an I/O error
    engine.model_info(&model).unwrap();
    engine.kill().unwrap();
    assert!(matches!(engine.model_info(&model), Err(Error::Io(_))));
}
