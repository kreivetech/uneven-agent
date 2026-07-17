use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;
use tokio::io::{AsyncBufReadExt, BufReader};
use regex::Regex;
use crate::UnevenError;
use crate::domain::parsed_error::ErrorType;
use super::ErrorEvent;
use crate::infrastructure::watcher::error_parser::ErrorParser;
use crate::infrastructure::watcher::terminal::TerminalMonitor;

#[derive(Debug, Clone)]
pub enum ProcessWatcherEvent {
    Start { command: String },
    Output { channel: String, data: String },
    Error { errors: Vec<ErrorEvent> },
    Exit { code: Option<i32> },
    Restart { attempt: u32 },
    Stopped { reason: String },
}

pub struct ProcessWatcher {
    command: String,
    cwd: String,
    auto_restart: bool,
    max_restarts: u32,
    is_running: Arc<AtomicBool>,
    event_tx: UnboundedSender<ProcessWatcherEvent>,
    terminal_monitor: Arc<TerminalMonitor>,
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl ProcessWatcher {
    pub fn new(
        command: String,
        cwd: String,
        auto_restart: bool,
        max_restarts: u32,
        log_path: &str,
        event_tx: UnboundedSender<ProcessWatcherEvent>,
    ) -> Self {
        Self {
            command,
            cwd,
            auto_restart,
            max_restarts,
            is_running: Arc::new(AtomicBool::new(false)),
            event_tx,
            terminal_monitor: Arc::new(TerminalMonitor::new(log_path)),
            cancel_tx: None,
        }
    }

    pub fn start(&mut self) -> Result<(), UnevenError> {
        if self.is_running.swap(true, Ordering::SeqCst) {
            return Err(UnevenError::WatcherError("Watcher already running".into()));
        }

        let command = self.command.clone();
        let cwd = self.cwd.clone();
        let auto_restart = self.auto_restart;
        let max_restarts = self.max_restarts;
        let is_running = self.is_running.clone();
        let event_tx = self.event_tx.clone();
        let terminal_monitor = self.terminal_monitor.clone();

        let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();
        self.cancel_tx = Some(cancel_tx);

        tokio::spawn(async move {
            let mut restart_count = 0;
            let stable_duration = Duration::from_secs(30);
            let save_regex = Regex::new(r"\bSAVE(D)?\b").unwrap();
            let dossier_regex = Regex::new("(?i)X-Uneven-Dossier[:'\" ]+([A-Za-z0-9+/=]+)").unwrap();

            let parts = parse_command(&command);
            if parts.is_empty() {
                let _ = event_tx.send(ProcessWatcherEvent::Error {
                    errors: vec![ErrorEvent {
                        file: "SYSTEM".to_string(),
                        line: 0,
                        column: 0,
                        message: "Empty command specified".to_string(),
                        stack_trace: String::new(),
                        error_type: Some(ErrorType::Unknown),
                        code: Some("SYSTEM".to_string()),
                    }],
                });
                is_running.store(false, Ordering::SeqCst);
                return;
            }

            let binary = &parts[0];
            let args = &parts[1..];

            loop {
                if !is_running.load(Ordering::SeqCst) {
                    break;
                }

                let mut cmd = Command::new(binary);
                cmd.args(args);
                cmd.current_dir(&cwd);
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());

                #[cfg(unix)]
                unsafe {
                    cmd.pre_exec(|| {
                        libc::setpgid(0, 0);
                        Ok(())
                    });
                }

                let mut child = match cmd.spawn() {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = event_tx.send(ProcessWatcherEvent::Error {
                            errors: vec![ErrorEvent {
                                file: "SYSTEM".to_string(),
                                line: 0,
                                column: 0,
                                message: format!("Failed to spawn command '{}': {}", command, e),
                                stack_trace: String::new(),
                                error_type: Some(ErrorType::Unknown),
                                code: Some("SYSTEM".to_string()),
                            }],
                        });
                        break;
                    }
                };

                let pid = child.id();
                let _ = event_tx.send(ProcessWatcherEvent::Start {
                    command: command.clone(),
                });

                // Stability timer
                let is_stable = Arc::new(AtomicBool::new(false));
                let is_stable_clone = is_stable.clone();
                let stability_task = tokio::spawn(async move {
                    sleep(stable_duration).await;
                    is_stable_clone.store(true, Ordering::SeqCst);
                });

                // Process output
                let stdout = child.stdout.take().expect("Failed to open stdout");
                let stderr = child.stderr.take().expect("Failed to open stderr");

                let event_tx_stdout = event_tx.clone();
                let event_tx_stderr = event_tx.clone();
                let terminal_monitor_clone = terminal_monitor.clone();

                let (io_cancel_tx, _io_cancel_rx) = tokio::sync::broadcast::channel::<()>(1);
                let mut io_cancel_rx_stdout = io_cancel_tx.subscribe();
                let mut io_cancel_rx_stderr = io_cancel_tx.subscribe();

                // Read stdout
                let save_regex_clone = save_regex.clone();
                let dossier_regex_clone = dossier_regex.clone();
                let stdout_task = tokio::spawn(async move {
                    let mut reader = BufReader::new(stdout).lines();
                    let mut history: Vec<String> = Vec::new();

                    loop {
                        tokio::select! {
                            line_res = reader.next_line() => {
                                match line_res {
                                    Ok(Some(line)) => {
                                        if history.len() >= 20 {
                                            history.remove(0);
                                        }
                                        history.push(line.clone());

                                        let _ = event_tx_stdout.send(ProcessWatcherEvent::Output {
                                            channel: "stdout".to_string(),
                                            data: format!("{}\n", line),
                                        });

                                        // Dossier Pentest check
                                        if let Some(cap) = dossier_regex_clone.captures(&line) {
                                            let _payload = &cap[1];
                                            terminal_monitor_clone.process_output(&line);

                                            let _ = event_tx_stdout.send(ProcessWatcherEvent::Error {
                                                errors: vec![ErrorEvent {
                                                    file: "SYSTEM".to_string(),
                                                    line: 0,
                                                    column: 0,
                                                    message: "URGENT: Incoming pentest probe detected. Attacker dossier bypassing VPN. Check logs.".to_string(),
                                                    stack_trace: line.clone(),
                                                    error_type: Some(ErrorType::Unknown),
                                                    code: Some("ATTACKER_DOSSIER".to_string()),
                                                }],
                                            });
                                        }

                                        // SAVE trigger check
                                        if save_regex_clone.is_match(&line) {
                                            let _ = event_tx_stdout.send(ProcessWatcherEvent::Error {
                                                errors: vec![ErrorEvent {
                                                    file: "unknown".to_string(),
                                                    line: 0,
                                                    column: 0,
                                                    message: "AutoFix triggered by SAVE notification in terminal.".to_string(),
                                                    stack_trace: line.clone(),
                                                    error_type: Some(ErrorType::ProactiveScan),
                                                    code: Some("SCAN".to_string()),
                                                }],
                                            });
                                        }

                                        // Regular compiler error check
                                        if let Some(mut err) = ErrorParser::parse_error(&line) {
                                            err.stack_trace = history.join("\n");
                                            let _ = event_tx_stdout.send(ProcessWatcherEvent::Error {
                                                errors: vec![err],
                                            });
                                        }
                                    }
                                    _ => break,
                                }
                            }
                            _ = io_cancel_rx_stdout.recv() => {
                                break;
                            }
                        }
                    }
                });

                // Read stderr
                let stderr_task = tokio::spawn(async move {
                    let mut reader = BufReader::new(stderr).lines();
                    let mut history: Vec<String> = Vec::new();

                    loop {
                        tokio::select! {
                            line_res = reader.next_line() => {
                                match line_res {
                                    Ok(Some(line)) => {
                                        if history.len() >= 20 {
                                            history.remove(0);
                                        }
                                        history.push(line.clone());

                                        let _ = event_tx_stderr.send(ProcessWatcherEvent::Output {
                                            channel: "stderr".to_string(),
                                            data: format!("{}\n", line),
                                        });

                                        if let Some(mut err) = ErrorParser::parse_error(&line) {
                                            err.stack_trace = history.join("\n");
                                            let _ = event_tx_stderr.send(ProcessWatcherEvent::Error {
                                                errors: vec![err],
                                            });
                                        }
                                    }
                                    _ => break,
                                }
                            }
                            _ = io_cancel_rx_stderr.recv() => {
                                break;
                            }
                        }
                    }
                });

                // Wait for process exit or manual stop
                let exit_status = tokio::select! {
                    res = child.wait() => {
                        stability_task.abort();
                        let _ = io_cancel_tx.send(());
                        let _ = stdout_task.await;
                        let _ = stderr_task.await;
                        res.ok()
                    }
                    _ = &mut cancel_rx => {
                        stability_task.abort();
                        let _ = io_cancel_tx.send(());
                        
                        // Kill process group on Unix or child on Windows
                        if let Some(p) = pid {
                            #[cfg(unix)]
                            {
                                let pgid = -(p as i32);
                                unsafe {
                                    libc::kill(pgid, libc::SIGTERM);
                                }
                                sleep(Duration::from_millis(500)).await;
                                unsafe {
                                    libc::kill(pgid, libc::SIGKILL);
                                }
                            }
                            #[cfg(not(unix))]
                            {
                                let _ = child.kill().await;
                            }
                        }
                        
                        let _ = stdout_task.await;
                        let _ = stderr_task.await;
                        is_running.store(false, Ordering::SeqCst);
                        break;
                    }
                };

                let exit_code = exit_status.and_then(|s| s.code());
                let _ = event_tx.send(ProcessWatcherEvent::Exit { code: exit_code });

                if !is_running.load(Ordering::SeqCst) {
                    break;
                }

                // Check stability
                if is_stable.load(Ordering::SeqCst) {
                    restart_count = 0;
                }

                // Decide restart
                if auto_restart && exit_code.unwrap_or(0) != 0 {
                    if restart_count < max_restarts {
                        restart_count += 1;
                        let _ = event_tx.send(ProcessWatcherEvent::Restart {
                            attempt: restart_count,
                        });
                        sleep(Duration::from_secs(2)).await;
                    } else {
                        let _ = event_tx.send(ProcessWatcherEvent::Stopped {
                            reason: "max-restarts".to_string(),
                        });
                        is_running.store(false, Ordering::SeqCst);
                        break;
                    }
                } else {
                    is_running.store(false, Ordering::SeqCst);
                    break;
                }
            }
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        if self.is_running.swap(false, Ordering::SeqCst) {
            if let Some(cancel) = self.cancel_tx.take() {
                let _ = cancel.send(());
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}

// Split command string respecting quotes
fn parse_command(cmd: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_double_quote = false;
    let mut in_single_quote = false;

    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
        } else if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
        } else if c.is_whitespace() && !in_double_quote && !in_single_quote {
            if !current.is_empty() {
                parts.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
        i += 1;
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

use tokio::time::sleep;
use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_simple() {
        let res = parse_command("npm run dev");
        assert_eq!(res, vec!["npm", "run", "dev"]);
    }

    #[test]
    fn test_parse_command_quotes() {
        let res = parse_command("npm run \"start server\" 'dev mode'");
        assert_eq!(res, vec!["npm", "run", "start server", "dev mode"]);
    }
}
