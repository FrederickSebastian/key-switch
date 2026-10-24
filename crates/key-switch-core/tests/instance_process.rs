use key_switch_core::{InstanceLock, StartupError};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    time::Duration,
};

#[test]
fn lock_child() {
    let Some(directory) = std::env::var_os("KEY_SWITCH_LOCK_TEST_DIR") else {
        return;
    };
    if let Some(gate) = std::env::var_os("KEY_SWITCH_LOCK_TEST_GATE") {
        let gate = std::path::PathBuf::from(gate);
        for _ in 0..500 {
            if gate.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(gate.exists());
    }
    match InstanceLock::acquire(std::path::Path::new(&directory)) {
        Ok(_lock) => {
            println!("LOCK_ACQUIRED");
            let mode = std::env::var("KEY_SWITCH_LOCK_TEST_MODE").unwrap_or_default();
            if mode == "abort" {
                std::process::abort();
            }
            if mode == "hold" {
                std::thread::sleep(Duration::from_secs(30));
            }
        }
        Err(StartupError::AlreadyRunning) => println!("LOCK_BUSY"),
        Err(e) => panic!("unexpected lock error: {e:?}"),
    }
}
fn child(directory: &std::path::Path, mode: &str) -> Command {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", "lock_child", "--nocapture"])
        .env("KEY_SWITCH_LOCK_TEST_DIR", directory)
        .env("KEY_SWITCH_LOCK_TEST_MODE", mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    cmd
}
#[test]
fn independent_processes_release_on_exit_kill_and_crash() {
    let directory = tempfile::tempdir().unwrap();
    {
        let mut owner = child(directory.path(), "hold").spawn().unwrap();
        let mut output = BufReader::new(owner.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            assert!(output.read_line(&mut line).unwrap() > 0);
            if line.contains("LOCK_ACQUIRED") {
                break;
            }
            line.clear();
        }
        let loser = child(directory.path(), "exit").output().unwrap();
        assert!(loser.status.success());
        assert!(String::from_utf8_lossy(&loser.stdout).contains("LOCK_BUSY"));
        owner.kill().unwrap();
        owner.wait().unwrap();
        let next = child(directory.path(), "exit").output().unwrap();
        assert!(next.status.success());
        assert!(String::from_utf8_lossy(&next.stdout).contains("LOCK_ACQUIRED"));
    }
    assert!(!child(directory.path(), "abort")
        .output()
        .unwrap()
        .status
        .success());
    assert!(InstanceLock::acquire(directory.path()).is_ok());
}

#[test]
fn simultaneous_startup_has_exactly_one_owner() {
    let directory = tempfile::tempdir().unwrap();
    let gate = directory.path().join("start");
    let mut first = child(directory.path(), "hold")
        .env("KEY_SWITCH_LOCK_TEST_GATE", &gate)
        .spawn()
        .unwrap();
    let mut second = child(directory.path(), "hold")
        .env("KEY_SWITCH_LOCK_TEST_GATE", &gate)
        .spawn()
        .unwrap();
    std::fs::write(&gate, "start").unwrap();
    let mut outputs = vec![];
    for process in [&mut first, &mut second] {
        let mut reader = BufReader::new(process.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line.contains("LOCK_ACQUIRED") || line.contains("LOCK_BUSY") {
                outputs.push(line);
                break;
            }
            line.clear();
        }
    }
    let _ = first.kill();
    let _ = second.kill();
    first.wait().unwrap();
    second.wait().unwrap();
    assert_eq!(
        outputs
            .iter()
            .filter(|s| s.contains("LOCK_ACQUIRED"))
            .count(),
        1
    );
    assert_eq!(
        outputs.iter().filter(|s| s.contains("LOCK_BUSY")).count(),
        1
    );
    assert!(InstanceLock::acquire(directory.path()).is_ok());
}
