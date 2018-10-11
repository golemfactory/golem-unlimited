#![allow(dead_code)]
use config::ConfigModule;
use daemonize::Daemonize;
use libc::{dup, flock, kill, LOCK_EX, LOCK_NB, SIGKILL, SIGQUIT, STDERR_FILENO, STDOUT_FILENO};
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::os::raw::c_int;
use std::os::unix::prelude::*;
use std::path::Path;
use std::str;
use std::thread::sleep;
use std::time::Duration;

pub enum Process {
    Running(i32),
    Stopped,
}

pub fn process_status<S>(name: S) -> Result<Process, String>
where
    S: AsRef<str>,
{
    let dir = ConfigModule::new().work_dir().to_path_buf();
    let name: &str = name.as_ref();

    let pid_path = dir.join(format!("{}.pid", name));

    if Path::exists(&pid_path) {
        let file = File::open(&pid_path).map_err(|_| "Cannot read pid file".to_string())?;

        if file_is_locked(&file) {
            let pid = read_pid_file(&pid_path)?;

            return Ok(Process::Running(pid));
        }
    }

    Ok(Process::Stopped)
}

pub fn daemonize_process<S>(name: S) -> Result<bool, String>
where
    S: AsRef<str>,
{
    let dir = ConfigModule::new().work_dir().to_path_buf();
    let name: &str = name.as_ref();

    let pid_path = dir.join(format!("{}.pid", name));

    if let Process::Running(pid) = process_status(name)? {
        println!("There is already running {} process (pid: {})", name, pid);
        return Ok(false);
    }

    let stdout = File::create(dir.join(format!("{}.out", name)))
        .map_err(|_| "Cannot create daemon .out file".to_string())?;
    let stderr = File::create(dir.join(format!("{}.err", name)))
        .map_err(|_| "Cannot create daemon .err file".to_string())?;

    let mut out = stdout_file();

    let daemonize = Daemonize::new()
        .pid_file(&pid_path)
        .chown_pid_file(true)
        .working_directory(&dir)
        .stdout(stdout)
        .stderr(stderr);

    daemonize
        .start()
        .map(|_| {
            let _ = out.write("Daemon started successfully\n".as_ref());
            true
        }).map_err(|e| format!("Daemon creation error: {}", e))
}

pub fn stop_process<S>(name: S) -> Result<(), String>
where
    S: AsRef<str>,
{
    let name: &str = name.as_ref();
    let dir = ConfigModule::new().work_dir().to_path_buf();
    let pid_path = dir.join(format!("{}.pid", name));

    if let Process::Running(pid) = process_status(name)? {
        let file = File::open(pid_path).map_err(|_| "Cannot read pid file".to_string())?;
        println!("Stopping {} process (pid: {})", name, pid);

        print!("Trying to kill the process gracefully... ");
        if !graceful_kill(pid, &file) {
            println!("failed");
            print!("Trying to kill the process forcefully... ");

            if !force_kill(pid, &file) {
                println!("failed");

                return Err(format!("Cannot stop {} process", name));
            } else {
                println!("success");
            }
        } else {
            println!("success");
        }
    } else {
        println!("There is no running instance of {}", name);
    }

    Ok(())
}

fn read_file(mut file: File) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map(|_| buf)
        .map_err(|e| format!(".pid file read error: {:?}", e))
}

fn read_file_with_path<P>(path: P) -> Result<Vec<u8>, String>
where
    P: AsRef<Path>,
{
    File::open(path)
        .map_err(|e| format!(".pid file open error: {:?}", e))
        .and_then(|file| read_file(file))
}

fn read_pid_file(path: &Path) -> Result<i32, String> {
    read_file_with_path(path).and_then(|buf| {
        String::from_utf8_lossy(&buf)
            .parse()
            .map_err(|e| format!("Cannot parse .pid file - {}", e))
    })
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

        if file_is_locked(&lock) {
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
