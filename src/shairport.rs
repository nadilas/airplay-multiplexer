use anyhow::{anyhow, Result};
use bytes::Bytes;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::broadcast;

use crate::types::TrackMetadata;

pub enum ShairportEvent {
    Started,
    Stopped,
    Metadata(TrackMetadata),
    Error(String),
}

pub struct ShairportManager {
    shairport_path: String,
    receiver_name: String,
    child: Option<Child>,
}

impl ShairportManager {
    pub fn new(shairport_path: &str, receiver_name: &str) -> Self {
        Self {
            shairport_path: shairport_path.to_string(),
            receiver_name: receiver_name.to_string(),
            child: None,
        }
    }

    pub async fn validate_binary(&self) -> Result<()> {
        let output = Command::new(&self.shairport_path)
            .arg("--version")
            .output()
            .await
            .map_err(|e| {
                anyhow!(
                    "shairport-sync binary not found at \"{}\". \
                     Install: sudo apt install shairport-sync (Linux) or \
                     brew install shairport-sync (macOS). Error: {}",
                    self.shairport_path,
                    e
                )
            })?;

        let combined = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);

        if combined.to_lowercase().contains("shairport") || output.status.success() {
            tracing::info!("[shairport] Found: {}", combined.trim());
            Ok(())
        } else {
            Err(anyhow!("shairport-sync not working. Output: {}", combined))
        }
    }

    pub async fn start(
        &mut self,
        audio_tx: broadcast::Sender<Bytes>,
        event_tx: tokio::sync::mpsc::Sender<ShairportEvent>,
    ) -> Result<()> {
        self.validate_binary().await?;
        self.spawn_process(audio_tx, event_tx).await
    }

    async fn spawn_process(
        &mut self,
        audio_tx: broadcast::Sender<Bytes>,
        event_tx: tokio::sync::mpsc::Sender<ShairportEvent>,
    ) -> Result<()> {
        let mut child = Command::new(&self.shairport_path)
            .args([
                "--name",
                &self.receiver_name,
                "--output",
                "stdout",
                "-v",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");

        // Read stdout PCM data and publish to broadcast channel
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut buf = vec![0u8; 4096];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = Bytes::copy_from_slice(&buf[..n]);
                        let _ = audio_tx.send(chunk);
                    }
                    Err(e) => {
                        tracing::error!("[shairport] stdout read error: {}", e);
                        break;
                    }
                }
            }
        });

        // Read stderr for metadata lines
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(meta) = parse_metadata_line(&line) {
                    let _ = event_tx_clone.send(ShairportEvent::Metadata(meta)).await;
                }
            }
        });

        let _ = event_tx.send(ShairportEvent::Started).await;
        self.child = Some(child);
        Ok(())
    }

    pub async fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }
}

pub fn parse_metadata_line(line: &str) -> Option<TrackMetadata> {
    let mut meta = TrackMetadata::default();

    if line.contains("Title:") {
        meta.title = line.split("Title:").nth(1).map(|s| s.trim().to_string());
    } else if line.contains("Artist:") {
        meta.artist = line.split("Artist:").nth(1).map(|s| s.trim().to_string());
    } else if line.contains("Album:") {
        meta.album = line.split("Album:").nth(1).map(|s| s.trim().to_string());
    }

    if meta.title.is_some() || meta.artist.is_some() || meta.album.is_some() {
        Some(meta)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_title() {
        let meta = parse_metadata_line("  Title: My Song").unwrap();
        assert_eq!(meta.title.unwrap(), "My Song");
        assert!(meta.artist.is_none());
        assert!(meta.album.is_none());
    }

    #[test]
    fn test_parse_artist() {
        let meta = parse_metadata_line("  Artist: The Band").unwrap();
        assert!(meta.title.is_none());
        assert_eq!(meta.artist.unwrap(), "The Band");
    }

    #[test]
    fn test_parse_album() {
        let meta = parse_metadata_line("  Album: Greatest Hits").unwrap();
        assert_eq!(meta.album.unwrap(), "Greatest Hits");
    }

    #[test]
    fn test_parse_non_metadata_line() {
        assert!(parse_metadata_line("Playing audio...").is_none());
        assert!(parse_metadata_line("").is_none());
    }
}
