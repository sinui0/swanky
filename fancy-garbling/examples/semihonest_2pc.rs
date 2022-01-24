// -*- mode: rust; -*-
//
// This file is part of `twopac`.
// Copyright © 2019 Galois, Inc.
// See LICENSE for licensing information.

use fancy_garbling::{
    circuit::Circuit,
    twopac::semihonest::{Evaluator, Garbler},
    FancyInput,
};
use ocelot::ot::{AlszReceiver as OtReceiver, AlszSender as OtSender};
use scuttlebutt::{unix_channel_pair, AesRng, UnixChannel};
use std::time::SystemTime;

fn circuit(fname: &str, garbler_inputs: Vec<usize>, evaluator_inputs: Vec<usize>) -> Circuit {
    println!("* Circuit: {}", fname);
    Circuit::parse(fname, garbler_inputs, evaluator_inputs).unwrap()
}

fn run_circuit(circ: &mut Circuit, gb_inputs: Vec<u16>, ev_inputs: Vec<u16>) {
    let circ_ = circ.clone();
    let (sender, receiver) = unix_channel_pair();
    let n_gb_inputs = gb_inputs.len();
    let n_ev_inputs = ev_inputs.len();
    let total = SystemTime::now();
    let handle = std::thread::spawn(move || {
        let rng = AesRng::new();
        let start = SystemTime::now();
        let mut gb = Garbler::<UnixChannel, AesRng, OtSender>::new(sender, rng).unwrap();
        println!(
            "Garbler :: Initialization: {} ms",
            start.elapsed().unwrap().as_millis()
        );
        let start = SystemTime::now();
        let xs = gb.encode_many(&gb_inputs, &vec![2; n_gb_inputs]).unwrap();
        let ys = gb.receive_many(&vec![2; n_ev_inputs]).unwrap();
        println!(
            "Garbler :: Encoding inputs: {} ms",
            start.elapsed().unwrap().as_millis()
        );
        let start = SystemTime::now();
        circ_.eval(&mut gb, &xs, &ys).unwrap();
        println!(
            "Garbler :: Circuit garbling: {} ms",
            start.elapsed().unwrap().as_millis()
        );
    });
    let rng = AesRng::new();
    let start = SystemTime::now();
    let mut ev = Evaluator::<UnixChannel, AesRng, OtReceiver>::new(receiver, rng).unwrap();
    println!(
        "Evaluator :: Initialization: {} ms",
        start.elapsed().unwrap().as_millis()
    );
    let start = SystemTime::now();
    let xs = ev.receive_many(&vec![2; n_gb_inputs]).unwrap();
    let ys = ev.encode_many(&ev_inputs, &vec![2; n_ev_inputs]).unwrap();
    println!(
        "Evaluator :: Encoding inputs: {} ms",
        start.elapsed().unwrap().as_millis()
    );
    let start = SystemTime::now();
    circ.eval(&mut ev, &xs, &ys).unwrap();
    println!(
        "Evaluator :: Circuit evaluation: {} ms",
        start.elapsed().unwrap().as_millis()
    );
    handle.join().unwrap();
    println!("Total: {} ms", total.elapsed().unwrap().as_millis());
}

fn main() {
    let mut circ = circuit(
        "circuits/bristol-fashion/aes_128.txt",
        (0..128).collect::<Vec<usize>>(),
        (128..256).collect::<Vec<usize>>(),
    );
    run_circuit(&mut circ, vec![0; 128], vec![0; 128]);
    let mut circ = circuit(
        "circuits/bristol-fashion/sha256.txt",
        (0..512).collect::<Vec<usize>>(),
        (512..768).collect::<Vec<usize>>(),
    );
    run_circuit(&mut circ, vec![0; 512], vec![0; 256]);
    let mut circ = circuit(
        "circuits/bristol-fashion/sha512.txt",
        (0..1024).collect::<Vec<usize>>(),
        (1024..1536).collect::<Vec<usize>>(),
    );
    run_circuit(&mut circ, vec![0; 1024], vec![0; 512]);
}
