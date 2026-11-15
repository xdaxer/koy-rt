//! CLI kodlama aracı (claude, codex, agy veya özel) süreçlerini güvenli şekilde çalıştıran servis.
//! Asla kabuk (cmd/powershell) kullanmaz, argümanları doğrudan argüman listesiyle geçirir ve iptal/zaman aşımı durumunda
//! bütün süreç ağacını sonlandırır (C# `Kill(entireProcessTree: true)`; burada iş nesnesiyle). Hata metni üretmez;
//! `VoiceFailure` kodu döner, metni App seçili dilde yazar.

use std::io::Read;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

use koy_core::cancel::CancellationToken;
use koy_core::diagnostics::{CrashError, CrashLog};
use koy_core::time::TimeSpan;
use koy_core::voice::{
    HarnessError, HarnessExecutableKind, HarnessExecutableResolver, HarnessExecutionRequest, HarnessExecutionResult, IHarnessRunnerService,
    VoiceCommandParser, VoiceFailure,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, SetInformationJobObject, TerminateJobObject,
};
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, WaitForSingleObject};
use windows::Win32::UI::Shell::FOLDERID_Profile;

use crate::search::win_env::known_folder;

#[derive(Default)]
pub struct ProcessHarnessRunnerService;

fn failed(failure: VoiceFailure, detail: impl Into<String>, duration: TimeSpan) -> HarnessExecutionResult {
    HarnessExecutionResult { success: false, exit_code: -1, output: String::new(), error: String::new(), duration, failure, detail: detail.into() }
}

fn resolve_working_directory(specified: &str) -> String {
    if !specified.trim().is_empty() && Path::new(specified).is_dir() {
        return specified.to_string();
    }
    let profile = known_folder(&FOLDERID_Profile);
    if Path::new(&profile).is_dir() {
        return profile;
    }
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_string_lossy().into_owned())).unwrap_or_default()
}

/// Süreç ağacı için iş nesnesi: bırakılınca (ya da sonlandırılınca) ağaçtaki bütün süreçler kapanır.
struct Job(HANDLE);

impl Job {
    fn create() -> Option<Job> {
        // SAFETY: adsız iş nesnesi; sınır bilgisi doğru boyutta.
        unsafe {
            let handle = CreateJobObjectW(None, None).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .ok()?;
            Some(Job(handle))
        }
    }

    fn kill(&self) {
        // SAFETY: iş nesnesi bizim.
        let _ = unsafe { TerminateJobObject(self.0, 1) };
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: tutamaç bir kez kapatılır.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn read_all(mut r: impl Read + Send + 'static) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = r.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

impl IHarnessRunnerService for ProcessHarnessRunnerService {
    fn execute(&self, request: &HarnessExecutionRequest, ct: &CancellationToken) -> Result<HarnessExecutionResult, HarnessError> {
        let resolved = VoiceCommandParser::resolve(
            request.harness_type,
            &request.prompt,
            request.custom_executable.as_deref(),
            request.custom_arguments_template.as_deref(),
            request.claude_permission,
            request.model.as_deref(),
        )?;

        let working_dir = resolve_working_directory(&request.working_directory);
        let timeout = TimeSpan::from_secs(request.timeout_seconds.max(5) as i64);

        let path = std::env::var("PATH").unwrap_or_default();
        let path_dirs: Vec<&str> = path.split(';').filter(|p| !p.is_empty()).collect();
        let executable = HarnessExecutableResolver::resolve(&resolved.executable, &path_dirs, |p| Path::new(p).is_file());
        if executable.kind == HarnessExecutableKind::ScriptNotSupported {
            return Ok(failed(VoiceFailure::ScriptNotSupported, executable.path, TimeSpan::ZERO));
        }

        // Bulunamadıysa ad olduğu gibi bırakılır; Win32 hatası aşağıdaki "bulunamadı" yolunda yakalanır.
        let file = if executable.kind == HarnessExecutableKind::Found { executable.path.clone() } else { resolved.executable.clone() };
        let started = Instant::now();
        let elapsed = || TimeSpan::from_std(started.elapsed());

        let mut child = match Command::new(&file)
            .args(&resolved.arguments)
            .current_dir(&working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW.0)
            .spawn()
        {
            Ok(c) => c,
            Err(e) if e.raw_os_error() == Some(2) => return Ok(failed(VoiceFailure::CommandNotFound, resolved.executable, elapsed())),
            Err(e) => {
                CrashLog::write("ProcessHarnessRunnerService.ExecuteAsync", &CrashError::exception("System.ComponentModel.Win32Exception", &e.to_string(), ""));
                return Ok(HarnessExecutionResult {
                    success: false,
                    exit_code: -1,
                    output: String::new(),
                    error: e.to_string(),
                    duration: elapsed(),
                    failure: VoiceFailure::StartFailed,
                    detail: resolved.executable,
                });
            }
        };

        let job = Job::create();
        if let Some(job) = &job {
            // SAFETY: çocuk süreç tutamacı Child yaşadıkça geçerli.
            let _ = unsafe { AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle())) };
        }
        let out = child.stdout.take().map(read_all);
        let err = child.stderr.take().map(read_all);

        let deadline = started + timeout.to_std();
        let handle = HANDLE(child.as_raw_handle());
        loop {
            // SAFETY: süreç tutamacı geçerli; 100 ms dilimlerle beklenir ki iptal ve zaman aşımı fark edilsin.
            if unsafe { WaitForSingleObject(handle, 100) } == WAIT_OBJECT_0 {
                break;
            }
            let canceled = ct.is_cancellation_requested();
            if canceled || Instant::now() >= deadline {
                match &job {
                    Some(job) => job.kill(),
                    None => {
                        let _ = child.kill();
                    }
                }
                let _ = child.wait();
                if canceled {
                    return Err(HarnessError::Canceled);
                }
                return Ok(failed(VoiceFailure::Timeout, format!("{:.0}", timeout.total_seconds()), elapsed()));
            }
        }

        let status = child.wait().map_err(|e| HarnessError::Unexpected(e.to_string()))?;
        let output = out.and_then(|h| h.join().ok()).unwrap_or_default();
        let error = err.and_then(|h| h.join().ok()).unwrap_or_default();
        let exit_code = status.code().unwrap_or(-1);
        let success = exit_code == 0;
        Ok(HarnessExecutionResult {
            success,
            exit_code,
            output: output.trim().to_string(),
            error: error.trim().to_string(),
            duration: elapsed(),
            failure: if success { VoiceFailure::None } else { VoiceFailure::ExitCode },
            detail: exit_code.to_string(),
        })
    }
}
