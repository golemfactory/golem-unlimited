
use std::path::PathBuf;
use std::collections::HashMap;
use tokio_process::Child;

type Map<K, V> = HashMap<K, V>;

#[derive(Debug, Hash, PartialOrd, PartialEq)]
pub struct Pid(u64);

pub struct ProcessPool {
    // process pool workdir
    work_dir : PathBuf,
    main_process : Option<(Pid, Child)>,
    exec_processes : Map<Pid, Child>
}




