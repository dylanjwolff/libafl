use serde::{Serialize, Deserialize};
use rand::{RngCore};
use std::fs::File;
use std::io::{Read, Write};
use std::io;
use walkdir::WalkDir;
use std::path::{PathBuf, Path};
use rmp_serde::{encode, decode};
use uuid::Uuid;
use crate::inputs::{BytesInput, HasBytesVec, HasTargetBytes, Input};
use crate::bolts::fs::write_file_atomic;
use alloc::{string::String, vec::Vec};
use crate::{bolts::ownedref::OwnedSlice, Error};



pub trait AsMultiBytes {
    fn as_multi_ownd_bytes(&self) -> Vec<OwnedSlice<u8>>;
}

pub trait AsMultiBytesVec {
    fn as_multi_bytes(&self) -> Vec<&[u8]>;
    fn as_multi_bytes_mut(&mut self) -> Vec<&mut Vec<u8>>;
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MultiInput {
    pub fields: Vec<BytesInput>,
}

impl MultiInput {
    fn new(f : Vec<BytesInput>) -> MultiInput {
        MultiInput {
            fields : f
        }
    }
}

impl Input for MultiInput {
        fn generate_name(&self, idx: usize) -> String {
            format!("{}{}.test", idx, Uuid::new_v4())
        }

        fn to_file<P>(&self, path: P) -> Result<(), Error> 
                where P: AsRef<Path> {
            let bytes: Vec<u8> = encode::to_vec(&self)
                .expect("Should encode");
            write_file_atomic(path, &bytes)
        }

        fn from_file<P>(path: P) -> Result<Self, Error>
                where P: AsRef<Path>,
        {
            let f : File = File::open(path)?;
            let t : MultiInput = decode::from_read(f)
                .expect("Should decode");
            Ok(t)
        }
}

impl AsMultiBytes for MultiInput {
    fn as_multi_ownd_bytes(&self) -> Vec<OwnedSlice<u8>> {
        self.fields.iter().map(|f| f.target_bytes()).collect()
    }
}

impl AsMultiBytesVec for MultiInput {
    fn as_multi_bytes(&self) -> Vec<&[u8]> {
        self.fields.iter().map(|f| f.bytes()).collect()
    }

    fn as_multi_bytes_mut(&mut self) -> Vec<&mut Vec<u8>> {
        self.fields.iter_mut().map(|f| f.bytes_mut()).collect()
    }
}


enum TestcaseSrc<'a> {
    Files(&'a Path),
    Rand
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

fn split(t : &MultiInput, dests : Vec<&Path>) -> io::Result<()> {
    assert!(t.fields.len() == dests.len());
    for (i, path) in t.fields.iter().zip(dests.iter()) {
        let mut f = File::create(&path)?;
        f.write_all(i.bytes())?;
    }
    Ok(())

}
