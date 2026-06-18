use std::process::Command;

pub fn set_system_audio_mute(muted: bool) -> Result<(), String> {
    let val = if muted { "1" } else { "0" };
    let output = Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", val])
        .output()
        .map_err(|e| format!("Failed to run pactl: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pactl failed: {}", stderr));
    }
    Ok(())
}

pub fn get_sink_mute_state() -> Result<bool, String> {
    let output = Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output()
        .map_err(|e| format!("Failed to run pactl: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pactl failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim().ends_with("yes"))
}
