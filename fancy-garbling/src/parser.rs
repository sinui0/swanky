// -*- mode: rust; -*-
//
// This file is part of fancy-garbling.
// Copyright © 2019 Galois, Inc.
// See LICENSE for licensing information.

//! Functions for parsing and running a circuit file based on the format given
//! here: <https://homes.esat.kuleuven.be/~nsmart/MPC/>.

use crate::{
    circuit::{Circuit, CircuitRef, Gate},
    errors::CircuitParserError as Error,
};
use regex::{Captures, Regex};
use std::{
    collections::HashSet,
    fs::File,
    io::{BufRead, BufReader},
    str::FromStr,
};

enum GateType {
    AndGate,
    XorGate,
}

fn cap2int(cap: &Captures, idx: usize) -> Result<usize, Error> {
    let s = cap.get(idx).ok_or(Error::ParseIntError)?;
    FromStr::from_str(s.as_str()).map_err(Error::from)
}

fn cap2typ(cap: &Captures, idx: usize) -> Result<GateType, Error> {
    let s = cap.get(idx).ok_or(Error::ParseIntError)?;
    let s = s.as_str();
    match s {
        "AND" => Ok(GateType::AndGate),
        "XOR" => Ok(GateType::XorGate),
        s => Err(Error::ParseGateError(s.to_string())),
    }
}

fn regex2captures<'t>(re: &Regex, line: &'t str) -> Result<Captures<'t>, Error> {
    re.captures(line)
        .ok_or_else(|| Error::ParseLineError(line.to_string()))
}

fn line2vec<'a>(re: &Regex, line: &'a str) -> Result<Vec<&'a str>, Error> {
    let v: Vec<&'a str> = re
        .captures_iter(line)
        .map(|cap| {
            let s = cap.get(1).unwrap().as_str();
            s
        })
        .collect();
    Ok(v)
}

impl Circuit {
    /// Generates a new `Circuit` from file `filename`. The file must follow the
    /// format given here: <https://homes.esat.kuleuven.be/~nsmart/MPC/>,
    /// the old format is not supported: <https://homes.esat.kuleuven.be/~nsmart/MPC/old-circuits.html>,
    /// otherwise a `CircuitParserError` is returned.
    pub fn parse(
        filename: &str,
        garbler_inputs: Vec<usize>,
        evaluator_inputs: Vec<usize>,
    ) -> Result<Self, Error> {
        let f = File::open(filename)?;
        let mut reader = BufReader::new(f);

        let garbler_input_set: HashSet<usize> = garbler_inputs.iter().cloned().collect();
        let evaluator_input_set: HashSet<usize> = evaluator_inputs.iter().cloned().collect();

        let duplicates = garbler_input_set.intersection(&evaluator_input_set);
        if duplicates.count() > 0 {
            return Err(Error::InputError());
        }

        let ngarbler_inputs: usize = garbler_input_set.len();
        let nevaluator_inputs: usize = evaluator_input_set.len();

        // Parse first line: ngates nwires\n
        let mut line = String::new();
        let _ = reader.read_line(&mut line)?;
        let re = Regex::new(r"(\d+)")?;
        let line_1 = line2vec(&re, &line)?;

        // Check that first line has 2 values: ngates, nwires
        if line_1.len() != 2 {
            return Err(Error::ParseLineError(line));
        }

        let ngates: usize = line_1[0].parse()?;
        let nwires: usize = line_1[1].parse()?;

        // Parse second line: ninputs input_0_nwires input_1_nwires...
        let mut line = String::new();
        let _ = reader.read_line(&mut line)?;
        let re = Regex::new(r"(\d+)\s*")?;
        let line_2 = line2vec(&re, &line)?;

        let ninputs: usize = line_2[0].parse()?; // Number of circuit inputs
        let input_nwires: Vec<usize> = line_2[1..]
            .iter()
            .map(|nwires| {
                let nwires: usize = nwires.parse().unwrap();
                nwires
            })
            .collect();

        // Check that nwires is specified for every input
        if input_nwires.len() != ninputs {
            return Err(Error::ParseLineError(line));
        }

        // Parse third line: noutputs output_0_nwires output_1_nwires...
        let mut line = String::new();
        let _ = reader.read_line(&mut line)?;
        let re = Regex::new(r"(\d+)\s*")?;
        let line_3 = line2vec(&re, &line)?;

        let noutputs: usize = line_3[0].parse()?; // Number of circuit outputs
        let output_nwires: Vec<usize> = line_3[1..]
            .iter()
            .map(|nwires| {
                let nwires: usize = nwires.parse().unwrap();
                nwires
            })
            .collect();

        // Check that nwires is specified for every output
        if output_nwires.len() != noutputs {
            return Err(Error::ParseLineError(line));
        }

        let mut circ = Self::new(Some(ngates));

        // Process garbler inputs.
        for i in 0..ngarbler_inputs {
            circ.gates.push(Gate::GarblerInput { id: i });
            circ.garbler_input_refs.push(CircuitRef {
                ix: garbler_inputs[i],
                modulus: 2,
            });
        }

        // Process evaluator inputs.
        for i in 0..nevaluator_inputs {
            circ.gates.push(Gate::EvaluatorInput { id: i });
            circ.evaluator_input_refs.push(CircuitRef {
                ix: evaluator_inputs[i],
                modulus: 2,
            });
        }

        // Create a constant wire for negations.
        circ.gates.push(Gate::Constant { val: 1 });
        let oneref = CircuitRef {
            ix: ngarbler_inputs + nevaluator_inputs,
            modulus: 2,
        };
        circ.const_refs.push(oneref);

        // Process outputs.
        for i in 0..output_nwires[0] {
            circ.output_refs.push(CircuitRef {
                ix: nwires - output_nwires[0] + i,
                modulus: 2,
            });
        }

        let re1 = Regex::new(r"1 1 (\d+) (\d+) INV")?;
        let re2 = Regex::new(r"2 1 (\d+) (\d+) (\d+) ((AND|XOR))")?;

        let mut id = 0;

        // Process gates
        for line in reader.lines() {
            let line = line?;
            match line.chars().next() {
                Some('1') => {
                    let cap = regex2captures(&re1, &line)?;
                    let yref = cap2int(&cap, 1)?;
                    let out = cap2int(&cap, 2)?;
                    let yref = CircuitRef {
                        ix: yref,
                        modulus: 2,
                    };
                    circ.gates.push(Gate::Sub {
                        xref: oneref,
                        yref,
                        out: Some(out),
                    })
                }
                Some('2') => {
                    let cap = regex2captures(&re2, &line)?;
                    let xref = cap2int(&cap, 1)?;
                    let yref = cap2int(&cap, 2)?;
                    let out = cap2int(&cap, 3)?;
                    let typ = cap2typ(&cap, 4)?;
                    let xref = CircuitRef {
                        ix: xref,
                        modulus: 2,
                    };
                    let yref = CircuitRef {
                        ix: yref,
                        modulus: 2,
                    };
                    let gate = match typ {
                        GateType::AndGate => {
                            let gate = Gate::Mul {
                                xref,
                                yref,
                                id,
                                out: Some(out),
                            };
                            id += 1;
                            gate
                        }
                        GateType::XorGate => Gate::Add {
                            xref,
                            yref,
                            out: Some(out),
                        },
                    };
                    circ.gates.push(gate);
                }
                None => continue,
                _ => {
                    return Err(Error::ParseLineError(line.to_string()));
                }
            }
        }
        circ.gate_moduli = vec![2u16; circ.gates.len()];
        Ok(circ)
    }
}

#[cfg(test)]
mod tests {
    use crate::circuit::Circuit;

    #[test]
    fn test_adder64() {
        let garbler_inputs: Vec<usize> = (0..64).collect();
        let evaluator_inputs: Vec<usize> = (64..128).collect();
        let circ = Circuit::parse(
            "circuits/bristol-fashion/adder64.txt",
            garbler_inputs,
            evaluator_inputs,
        )
        .unwrap();

        let a = vec![0u16; 64];
        let b = vec![0u16; 64];
        let output = circ.eval_plain(&a, &b).unwrap();
        assert_eq!(
            output.iter().map(|i| i.to_string()).collect::<String>(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );

        let mut a = vec![0u16; 64];
        a[63] = 1;
        a.reverse();
        let b = vec![0u16; 64];
        let mut output = circ.eval_plain(&a, &b).unwrap();
        output.reverse();
        assert_eq!(
            output.iter().map(|i| i.to_string()).collect::<String>(),
            "0000000000000000000000000000000000000000000000000000000000000001"
        );

        let a = vec![0u16; 64];
        let mut b = vec![0u16; 64];
        b[63] = 1;
        b.reverse();
        let mut output = circ.eval_plain(&a, &b).unwrap();
        output.reverse();
        assert_eq!(
            output.iter().map(|i| i.to_string()).collect::<String>(),
            "0000000000000000000000000000000000000000000000000000000000000001"
        );

        let mut a = vec![0u16; 64];
        a[63] = 1;
        a.reverse();
        let mut b = vec![0u16; 64];
        b[63] = 1;
        b.reverse();
        let mut output = circ.eval_plain(&a, &b).unwrap();
        output.reverse();
        assert_eq!(
            output.iter().map(|i| i.to_string()).collect::<String>(),
            "0000000000000000000000000000000000000000000000000000000000000010"
        );

        let a = vec![1u16; 64];
        let mut b = vec![0u16; 64];
        b[63] = 1;
        b.reverse();
        let mut output = circ.eval_plain(&a, &b).unwrap();
        output.reverse();
        assert_eq!(
            output.iter().map(|i| i.to_string()).collect::<String>(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn test_aes_reverse() {
        let garbler_inputs: Vec<usize> = (0..128).collect();
        let evaluator_inputs: Vec<usize> = (128..256).collect();
        let circ = Circuit::parse(
            "circuits/bristol-fashion/aes_128_reverse.txt",
            garbler_inputs,
            evaluator_inputs,
        )
        .unwrap();

        let mut key = vec![0u16; 128];
        let mut pt = vec![0u16; 128];
        let mut output = circ.eval_plain(&key, &pt).unwrap();
        output.reverse();
        assert_eq!(output.iter().map(|i| i.to_string()).collect::<String>(),
                   "01100110111010010100101111010100111011111000101000101100001110111000100001001100111110100101100111001010001101000010101100101110");

        key = vec![1u16; 128];
        pt = vec![0u16; 128];
        output = circ.eval_plain(&key, &pt).unwrap();
        output.reverse();
        assert_eq!(output.iter().map(|i| i.to_string()).collect::<String>(),
                   "10100001111101100010010110001100100001110111110101011111110011011000100101100100010010000100010100111000101111111100100100101100");

        key = vec![0u16; 128];
        key[7] = 1;
        key.reverse();
        pt = vec![0u16; 128];
        output = circ.eval_plain(&key, &pt).unwrap();
        output.reverse();
        assert_eq!(output.iter().map(|i| i.to_string()).collect::<String>(),
                   "11011100000011101101100001011101111110010110000100011010101110110111001001001001110011011101000101101000110001010100011001111110");

        key = vec![0u16; 128];
        for i in 0..8 {
            key[127 - i] = 1;
        }
        key.reverse();
        pt = vec![0u16; 128];
        output = circ.eval_plain(&key, &pt).unwrap();
        output.reverse();
        assert_eq!(output.iter().map(|i| i.to_string()).collect::<String>(),
                   "11010101110010011000110001001000001001010101111101111000110011000100011111100001010010011110010101011100111111000011111111111101");

        key = vec![0u16; 128];
        for i in 0..8 {
            key[127 - i] = 1;
        }
        key.reverse();
        pt = vec![0u16; 128];
        pt.splice(
            ..64,
            [
                0, 1, 1, 0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 1, 0, 0, 0, 1, 1, 0, 1, 0, 0, 0, 0, 1, 1, 0,
                0, 1, 0, 1, 0, 1, 1, 1, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 0, 1, 1, 1, 0, 1, 0, 1,
                0, 1, 1, 0, 1, 1, 0, 1,
            ],
        );
        pt.reverse();
        output = circ.eval_plain(&key, &pt).unwrap();
        output.reverse();
        assert_eq!(output.iter().map(|i| i.to_string()).collect::<String>(),
                    "10001010010011010111111100000011011000110101001101101001101011100001101111001110101010001010101010000000100010000001000101010111");

        key = vec![0u16; 128];

        for i in 0..8 {
            key[i] = 1;
        }

        key.reverse();
        pt = vec![0u16; 128];
        output = circ.eval_plain(&key, &pt).unwrap();
        output.reverse();
        assert_eq!(output.iter().map(|i| i.to_string()).collect::<String>(),
                    "10110001110101110101100000100101011010110010100011111101100001010000101011010100100101000100001000001000110011110001000101010101");
    }

    #[test]
    fn test_aes() {
        let garbler_inputs: Vec<usize> = (0..128).collect();
        let evaluator_inputs: Vec<usize> = (128..256).collect();
        let circ = Circuit::parse(
            "circuits/bristol-fashion/aes_128.txt",
            garbler_inputs,
            evaluator_inputs,
        )
        .unwrap();

        let mut key = vec![0u16; 128];

        for i in 0..8 {
            key[i] = 1;
        }

        let pt = vec![0u16; 128];
        let output = circ.eval_plain(&pt, &key).unwrap();
        assert_eq!(output.iter().map(|i| i.to_string()).collect::<String>(),
                    "10110001110101110101100000100101011010110010100011111101100001010000101011010100100101000100001000001000110011110001000101010101");
    }

    // #[test]
    // fn test_gc_eval() {
    //     let garbler_inputs: Vec<usize> = (0..127).collect();
    //     let evaluator_inputs: Vec<usize> = (128..256).collect();
    //     let circ = Circuit::parse(
    //         "circuits/bristol-fashion/aes_128.txt",
    //         garbler_inputs,
    //         evaluator_inputs,
    //     )
    //     .unwrap();
    //     let (en, gc) = garble(&mut circ).unwrap();
    //     let gb = en.encode_garbler_inputs(&vec![0u16; 128]);
    //     let ev = en.encode_evaluator_inputs(&vec![0u16; 128]);
    //     gc.eval(&mut circ, &gb, &ev).unwrap();
    // }
}
