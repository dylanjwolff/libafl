use serde::{Serialize, Deserialize};
use rand::{RngCore};
use std::fs::File;
use std::io::{Read, Write};
use std::io;
use walkdir::WalkDir;
use std::path::{PathBuf, Path};
use rmp_serde::{encode, decode};
use uuid::Uuid;
use libafl::inputs::{BytesInput, HasBytesVec, Input, MultiInput};
use libafl::Error;
use libafl::bolts::fs::write_file_atomic;

enum TestcaseSrc<'a> {
    Files(&'a Path),
    Rand,
}

fn main() {
    let args : Vec<String> = std::env::args().collect();
    assert!(args.len() >= 2);
    if args[1] == "bundle" {
        assert!(args.len() == 4);
        let raw_input_path = Path::new(&args[2]);
        let bundled_input_path = Path::new(&args[3]);

        let spec = vec![TestcaseSrc::Files(&raw_input_path),
                        TestcaseSrc::Rand];
        let num_tests = 10; // default number of completely random tests to generate if no raw input files

        let testcases = combine_and_or_generate(spec, num_tests);
        let paths = bundle(&testcases, bundled_input_path);
        for path in paths.iter() {
            println!("Created {}", path.to_str().expect("path should be string-able"));
        }
    } else if args[1] == "split" {
        assert!(args.len() == 3);
        println!("Warning: only implemented for splitting into two input files");
        let raw_input_path = Path::new(&args[2]);
        let testcase = Path::new(&args[2]);
        let filename = testcase.file_name().map(|n| n.to_string_lossy()).expect("filename should exist");
        let testcase = MultiInput::from_file(testcase).expect("should be parseable as multi-input");
        split(&testcase, vec![Path::new(&format!("{}.in", filename)), Path::new(&format!("{}.sched", filename))]).expect("main test should work");
    } else {
        panic!("unimplemented!")
    }
}

fn combine_and_or_generate(sources : Vec<TestcaseSrc>, numtests : u32) -> Vec<MultiInput> {
    let ins_from_files : Vec<Vec<BytesInput>> = sources.iter()
        .filter_map(|ts| match ts {
            TestcaseSrc::Files(p) => Some(get_contents(p)),
            _ => None,
        }).collect();

    if ins_from_files.len() > 1 {
        panic!("Not implemented for combinations of multiple classes of input file");
    } else if ins_from_files.len() == 1 {
        let finputs = ins_from_files[0].clone();
        let index = sources.iter().position(|_ts| matches!(TestcaseSrc::Files, _ts))
            .expect("Should be present");
        let v : Vec<MultiInput> = finputs.into_iter().map(|finput| {
            let mut fields = Vec::with_capacity(sources.len());
            fields.resize_with(sources.len(), || generate_field());
            fields[index] = finput;
            MultiInput::new(fields)
        }).collect();
        return v;
    } else {
        let v = (0..numtests)
            .map(|_| MultiInput::new(sources.iter().map(|_| generate_field()).collect()) )
            .collect();
        return v;
    }
}

fn get_contents(p : &Path) -> Vec<BytesInput> {
    WalkDir::new(p).into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_type().is_dir())
        .filter_map(|e| File::open(e.into_path()).ok())
        .map(|mut f| {
            let mut v = Vec::new();
            f.read_to_end(&mut v).expect("all inputs should be readable");
            v
                          })
        .map(|b| BytesInput::new(b))
        .collect()
}


fn generate_field() -> BytesInput {
     let mut rng = rand::thread_rng();
     let mut v = vec![0; 8192];
     rng.fill_bytes(&mut v);
     return BytesInput::new(v);
}

fn bundle(v : &Vec<MultiInput>, dest : &Path) -> Vec<PathBuf> {
    v.iter()
        .map(|t| encode::to_vec(t).expect("should encode"))
        .map(|t| {
            let fname = dest.join(format!("{}.test", Uuid::new_v4().to_string()));
            let mut f = File::create(&fname)
                .expect("file should be able to create");
            f.write_all(&t).expect("file should write");
            fname
        }).collect()
}

fn split(t : &MultiInput, dests : Vec<&Path>) -> io::Result<()> {
    assert!(t.fields.len() == dests.len());
    for (i, path) in t.fields.iter().zip(dests.iter()) {
        let mut f = File::create(&path)?;
        f.write_all(i.bytes())?;
    }
    Ok(())

}
