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
use crate::bolts::HasLen;

pub trait AsMultiBytes {
    fn as_multi_ownd_bytes(&self) -> Vec<OwnedSlice<u8>>;
}

pub trait AsMultiBytesVec {
    fn as_multi_bytes(&self) -> Vec<&[u8]>;
    fn as_multi_bytes_mut(&mut self) -> Vec<&mut Vec<u8>>;
}

pub trait AsMultiInput<I> {
    fn as_multi_input_mut(&mut self) -> &mut Vec<I> where I: Input;
    fn as_multi_input(&self) -> &Vec<I> where I: Input;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MultiInput {
    pub fields: Vec<BytesInput>,
}

impl MultiInput {
    pub fn new(f : Vec<BytesInput>) -> MultiInput {
        MultiInput {
            fields : f
        }
    }
}

impl AsMultiInput<BytesInput> for MultiInput {
    fn as_multi_input_mut(&mut self) -> &mut Vec<BytesInput> {
        &mut self.fields
    }
    fn as_multi_input(&self) -> &Vec<BytesInput> {
        &self.fields
    }
}

//@TODO @FIXME
impl HasBytesVec for MultiInput {
        fn bytes(&self) -> &[u8] {
            return self.fields[0].bytes()
        }

        fn bytes_mut(&mut self) -> &mut Vec<u8> {
            return self.fields[0].bytes_mut()
        }
}

impl HasLen for MultiInput {
        fn len(&self) -> usize {
            self.fields.iter().map(|f| f.len()).sum()
        }

        fn is_empty(&self) -> bool { 
            self.fields.iter().all(|f| f.is_empty())
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

