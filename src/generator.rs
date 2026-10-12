//! A bounded, provider-independent world designer process.
//!
//! The runner receives one JSON request on stdin and returns one WorldRecipe on
//! stdout. It has no authority to write game saves; only validated recipes commit.
use crate::world::recipe::WorldRecipe;
use crate::worlds::WorldStore;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const MAX_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_ATTEMPTS: u8 = 3;
const RUNNER_TIMEOUT: Duration = Duration::from_secs(90);

pub fn generate(
    store: &WorldStore,
    id: &str,
    prompt: &str,
    program: &Path,
    mut progress: impl FnMut(&str),
) -> Result<PathBuf, String> {
    // Fail before making a model request when the destination cannot be used.
    if store.path(id)?.try_exists().map_err(|e| e.to_string())? {
        return Err(format!("world {id} already exists; choose a new ID"));
    }
    if prompt.trim().is_empty()
        || prompt.len() > 4000
        || prompt
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err("describe the world in 1–4000 bytes without control characters".into());
    }
    design(
        prompt,
        |request| run_program(program, request, RUNNER_TIMEOUT),
        &mut progress,
    )
    .and_then(|recipe| store.create(id, prompt, &recipe))
}

fn design(
    prompt: &str,
    mut runner: impl FnMut(&Value) -> Result<Vec<u8>, String>,
    progress: &mut impl FnMut(&str),
) -> Result<WorldRecipe, String> {
    let mut previous: Option<Value> = None;
    let mut feedback: Option<String> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        progress(&format!(
            "Designing world (attempt {attempt}/{MAX_ATTEMPTS})…"
        ));
        let bytes = runner(&json!({
            "protocol":1,"prompt":prompt,"schema":WorldRecipe::schema(),
            "example":WorldRecipe::example(),"previous_design":previous,"validation_error":feedback,
            "instructions":"Return only a JSON world recipe. 2D side view: x right, y down. Landmark y is relative to the terrain surface at landmark x. Rectangles apply in order; air hollows rooms. Keep x=-7..7 clear. Build accessible entrances and preserve nearby wood and ore. Use only the schema's existing blocks and mechanics."
        }))?;
        if bytes.len() > MAX_OUTPUT_BYTES {
            return Err("generator output exceeds 256 KiB".into());
        }
        let parsed: Result<Value, _> = serde_json::from_slice(&bytes);
        let result = parsed
            .as_ref()
            .map_err(|e| e.to_string())
            .and_then(|value| {
                serde_json::from_value::<WorldRecipe>(value.clone()).map_err(|e| e.to_string())
            })
            .and_then(|recipe| {
                recipe.validate()?;
                Ok(recipe)
            });
        match result {
            Ok(recipe) => {
                progress("Design validated. Saving new world…");
                return Ok(recipe);
            }
            Err(error) => {
                progress(&format!("Design rejected: {error}"));
                previous = parsed.ok();
                feedback = Some(error);
            }
        }
    }
    Err(format!(
        "no valid world after {MAX_ATTEMPTS} attempts: {}; no world was created",
        feedback.unwrap_or_default()
    ))
}

fn capture(
    mut pipe: impl Read + Send + 'static,
    limit: usize,
) -> mpsc::Receiver<Result<Vec<u8>, String>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = pipe
            .by_ref()
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())
            .and_then(|_| {
                if bytes.len() > limit {
                    Err("generator exceeded its output limit".into())
                } else {
                    Ok(bytes)
                }
            });
        let _ = sender.send(result);
    });
    receiver
}

fn run_program(program: &Path, request: &Value, timeout: Duration) -> Result<Vec<u8>, String> {
    run_program_cancellable(
        program,
        request,
        timeout,
        &std::sync::atomic::AtomicBool::new(false),
    )
}

pub(crate) fn run_program_cancellable(
    program: &Path,
    request: &Value,
    timeout: Duration,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>, String> {
    // No shell, command interpolation, or model-provided executable names.
    let mut child = Command::new(program)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start generator {}: {e}", program.display()))?;
    let mut stdin = child.stdin.take().ok_or("generator stdin unavailable")?;
    let output = capture(
        child.stdout.take().ok_or("generator stdout unavailable")?,
        MAX_OUTPUT_BYTES,
    );
    let errors = capture(
        child.stderr.take().ok_or("generator stderr unavailable")?,
        8192,
    );
    let encoded = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    let (written_tx, written_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = written_tx.send(stdin.write_all(&encoded).map_err(|e| e.to_string()));
    });
    let started = Instant::now();
    let mut stdout = None;
    let mut stderr = None;
    let mut written = None;
    let result = (|| {
        loop {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err("request cancelled".into());
            }
            if started.elapsed() >= timeout {
                return Err("generator timed out; no design accepted".into());
            }
            if stdout.is_none()
                && let Ok(result) = output.try_recv()
            {
                stdout = Some(result?);
            }
            if stderr.is_none()
                && let Ok(result) = errors.try_recv()
            {
                stderr = Some(result?);
            }
            if written.is_none()
                && let Ok(result) = written_rx.try_recv()
            {
                written = Some(result);
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())?
                && let (Some(stdout), Some(stderr), Some(written)) = (&stdout, &stderr, &written)
            {
                if !status.success() {
                    return Err(format!(
                        "generator exited with {status}: {}",
                        String::from_utf8_lossy(stderr).trim()
                    ));
                }
                written
                    .as_ref()
                    .map_err(|e| format!("generator did not read its request: {e}"))?;
                return Ok(stdout.clone());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repairs_rejected_design_and_limits_attempts() {
        let mut calls = 0;
        let result = design(
            "a snowy tower",
            |request| {
                calls += 1;
                let mut recipe = WorldRecipe::example();
                if calls == 1 {
                    recipe.landmarks[0].x = 0;
                } else {
                    assert!(
                        request["validation_error"]
                            .as_str()
                            .unwrap()
                            .contains("spawn")
                    );
                    assert_eq!(request["previous_design"]["landmarks"][0]["x"], 0);
                }
                Ok(serde_json::to_vec(&recipe).unwrap())
            },
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(result.landmarks[0].x, 24);
        calls = 0;
        assert!(
            design(
                "snow",
                |_| {
                    calls += 1;
                    Ok(b"not JSON".to_vec())
                },
                &mut |_| {}
            )
            .is_err()
        );
        assert_eq!(calls, 3);
        calls = 0;
        assert!(
            design(
                "snow",
                |_| {
                    calls += 1;
                    Err("provider unavailable".into())
                },
                &mut |_| {}
            )
            .is_err()
        );
        assert_eq!(calls, 1); // Never silently repeat a failed provider request.
    }

    #[cfg(unix)]
    #[test]
    fn runner_obeys_deadline_and_output_budget() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("termcraft-runner-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let program = root.join("runner");
        std::fs::write(&program, b"#!/bin/sh\nexec sleep 10\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let started = Instant::now();
        assert!(
            run_program(&program, &json!({}), Duration::from_millis(70))
                .unwrap_err()
                .contains("timed out")
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        let cancelled = std::sync::atomic::AtomicBool::new(true);
        let started = Instant::now();
        assert!(
            run_program_cancellable(&program, &json!({}), Duration::from_secs(90), &cancelled)
                .unwrap_err()
                .contains("cancelled")
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        std::fs::write(&program, b"#!/bin/sh\nexec head -c 262145 /dev/zero\n").unwrap();
        assert!(
            run_program(&program, &json!({}), Duration::from_secs(2))
                .unwrap_err()
                .contains("output limit")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
