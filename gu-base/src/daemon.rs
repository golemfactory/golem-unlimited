#![allow(dead_code)]

use daemonize::Daemonize;
use libc::{
    dup, flock, getpid, kill, LOCK_EX, LOCK_NB, SIGKILL, SIGQUIT, STDERR_FILENO, STDOUT_FILENO,
};
use std::{
    fs::File,
    io::{Read, Write},
    os::{raw::c_int, unix::prelude::*},
    path::{Path, PathBuf},
    str,
    thread::sleep,
    time::Duration,
};

pub enum ProcessStatus {
    Running(i32),
    Stopped,
}

pub struct DaemonProcess {
    name: String,
    status: ProcessStatus,
    work_dir: PathBuf,
    pid_path: PathBuf,
}

impl DaemonProcess {
    pub fn create<S, P>(name: S, work_dir: P) -> Self
    where
        S: AsRef<str>,
        P: AsRef<Path>,
    {
        DaemonProcess {
            name: name.as_ref().into(),
            status: ProcessStatus::Stopped,
            work_dir: work_dir.as_ref().into(),
            pid_path: work_dir.as_ref().join(format!("{}.pid", name.as_ref())),
        }
    }

    pub fn status(&self) -> Result<ProcessStatus, String> {
        if Path::exists(&self.pid_path) {
            let file = open_file(&self.pid_path)?;

            if file_is_locked(&file) {
                return read_pid_file(file).map(|pid| ProcessStatus::Running(pid));
            }
        }

        Ok(ProcessStatus::Stopped)
    }

    pub fn run_normally(&self) -> Result<(), String> {
        if let ProcessStatus::Running(pid) = &self.status()? {
            Err(format!(
                "There is already running {} process (pid: {})",
                &self.name, pid
            ))
        } else {
            write_pid_file(&self.pid_path)
        }
    }

    pub fn daemonize(&self) -> Result<bool, String> {
        if let ProcessStatus::Running(pid) = &self.status()? {
            println!(
                "There is already running {} process (pid: {})",
                &self.name, pid
            );
            return Ok(false);
        }

        let stdout = File::create(&self.work_dir.join(format!("{}.out", &self.name)))
            .map_err(|_| "Cannot create daemon .out file".to_string())?;
        let stderr = File::create(&self.work_dir.join(format!("{}.err", &self.name)))
            .map_err(|_| "Cannot create daemon .err file".to_string())?;

        let mut out = stdout_file();

        let daemonize = Daemonize::new()
            .pid_file(&self.pid_path)
            .chown_pid_file(true)
            .working_directory(&self.work_dir)
            .stdout(stdout)
            .stderr(stderr);

        daemonize
            .start()
            .map(|_| {
                let _ = out.write("Daemon started successfully\n".as_ref());
                true
            }).map_err(|e| format!("Daemon creation error: {}", e))
    }

    pub fn stop(&self) -> Result<(), String> {
        if let ProcessStatus::Running(pid) = &self.status()? {
            let file = open_file(&self.pid_path)?;
            println!("Stopping {} process (pid: {})", &self.name, pid);

            print!("Trying to kill the process gracefully... ");
            if !graceful_kill(pid.clone(), &file) {
                println!("failed");
                print!("Trying to kill the process forcefully... ");

                if !force_kill(pid.clone(), &file) {
                    println!("failed");

                    return Err(format!("Cannot stop {} process", &self.name));
                } else {
                    println!("success");
                }
            } else {
                println!("success");
            }
        } else {
            println!("There is no running instance of {}", &self.name);
        }

        Ok(())
    }
}

fn open_file<P: AsRef<Path>>(path: P) -> Result<File, String> {
    File::open(path).map_err(|e| format!(".pid file open error: {:?}", e))
}

fn read_file(mut file: File) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map(|_| buf)
        .map_err(|e| format!(".pid file read error: {:?}", e))
}

fn read_pid_file(file: File) -> Result<i32, String> {
    read_file(file).and_then(|buf| {
        String::from_utf8_lossy(&buf)
            .parse()
            .map_err(|e| format!("Cannot parse .pid file - {}", e))
    })
}

fn write_pid_file(path: &Path) -> Result<(), String> {
    let buf;
    unsafe {
        buf = getpid().to_string();
    }

    let mut file = File::create(path).map_err(|_| "Cannot open pid file".to_string())?;

    if file_is_locked(&file) {
        Err("Cannot lock newly created pid file".to_string())
    } else {
        file.write(buf.as_bytes())
            .map(|_| {
                Box::leak(Box::new(file));
            }).map_err(|_| "Pid file write error".to_string())
    }
}

fn file_is_locked(file: &File) -> bool {
    let fd = file.as_raw_fd();
    unsafe { flock(fd, LOCK_EX | LOCK_NB) != 0 }
}

unsafe fn fd_to_file(std: c_int) -> File {
    File::from_raw_fd(dup(std))
}

fn stdout_file() -> File {
    unsafe { fd_to_file(STDOUT_FILENO) }
}

fn stderr_file() -> File {
    unsafe { fd_to_file(STDERR_FILENO) }
}

fn send_kill(pid: i32, sig: c_int) {
    unsafe {
        kill(pid, sig);
    }
}

fn kill_with_waiting(pid: i32, lock: &File, sig: c_int) -> bool {
    send_kill(pid, sig);

    for i in 1..(20 * 15) {
        if i % 20 == 0 {
            print!(".");
        }

        sleep(Duration::from_millis(50));

        if !file_is_locked(&lock) {
            return true;
        }
    }

    false
}

fn graceful_kill(pid: i32, file: &File) -> bool {
    kill_with_waiting(pid, file, SIGQUIT)
}

fn force_kill(pid: i32, file: &File) -> bool {
    kill_with_waiting(pid, file, SIGKILL)
}

//#[cfg(test)]
//mod tests {
//    use daemon::daemonize_process;
//    use daemon::file_is_locked;
//    use daemon::stop_process;
//    use std::fs::File;
//    use std::thread::sleep;
//    use std::time::Duration;
//
//    #[test]
//    fn start() {
//        daemonize_process("gu-hub");
//
//        sleep(Duration::from_secs(1234));
//    }
//
//    #[test]
//    fn killy() {
//        let _ = stop_process("gu-hub");
//    }
//}
