// -*- mode: rust; -*-
//
// This file is part of fancy-garbling.
// Copyright © 2019 Galois, Inc.
// See LICENSE for licensing information.

//! Benchmark code of garbling / evaluating using Nigel's circuits.

use criterion::{criterion_group, criterion_main, Criterion};
use fancy_garbling::{circuit::Circuit, classic::garble};
use std::time::Duration;

fn circuit(fname: &str, garber_inputs: Vec<usize>, evaluator_inputs: Vec<usize>) -> Circuit {
    let circ = Circuit::parse(fname, garber_inputs, evaluator_inputs).unwrap();
    // println!("{}", fname);
    // circ.print_info().unwrap();
    circ
}

fn bench_garble_aes(c: &mut Criterion) {
    let circ = circuit(
        "circuits/bristol-fashion/aes_128.txt",
        (0..128).collect::<Vec<usize>>(),
        (128..256).collect::<Vec<usize>>(),
    );
    c.bench_function("garble::aes", move |bench| {
        bench.iter(|| garble(&circ));
    });
}

fn bench_eval_aes(c: &mut Criterion) {
    let circ = circuit(
        "circuits/bristol-fashion/aes_128.txt",
        (0..128).collect::<Vec<usize>>(),
        (128..256).collect::<Vec<usize>>(),
    );
    let (en, gc) = garble(&circ).unwrap();
    let gb = en.encode_garbler_inputs(&vec![0u16; 128]);
    let ev = en.encode_evaluator_inputs(&vec![0u16; 128]);
    c.bench_function("eval::aes", move |bench| {
        bench.iter(|| gc.eval(&circ, &gb, &ev));
    });
}

fn bench_garble_sha_256(c: &mut Criterion) {
    let circ = circuit(
        "circuits/bristol-fashion/sha256.txt",
        (0..512).collect::<Vec<usize>>(),
        (512..768).collect::<Vec<usize>>(),
    );
    c.bench_function("garble::sha-256", move |bench| {
        bench.iter(|| garble(&circ));
    });
}

fn bench_eval_sha_256(c: &mut Criterion) {
    let circ = circuit(
        "circuits/bristol-fashion/sha256.txt",
        (0..512).collect::<Vec<usize>>(),
        (512..768).collect::<Vec<usize>>(),
    );
    let (en, gc) = garble(&circ).unwrap();
    let gb = en.encode_garbler_inputs(&vec![0u16; 512]);
    let ev = en.encode_evaluator_inputs(&vec![0u16; 256]);
    c.bench_function("eval::sha-256", move |bench| {
        bench.iter(|| gc.eval(&circ, &gb, &ev));
    });
}

criterion_group! {
    name = parsing;
    config = Criterion::default().warm_up_time(Duration::from_millis(100));
    targets = bench_garble_aes, bench_eval_aes, bench_garble_sha_256, bench_eval_sha_256
}

criterion_main!(parsing);
