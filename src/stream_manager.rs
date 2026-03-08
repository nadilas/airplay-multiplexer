use bytes::Bytes;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::broadcast;

use crate::types::AudioFormat;

const BROADCAST_CAPACITY: usize = 1024;

pub struct StreamManager {
    audio_format: AudioFormat,
    sender: broadcast::Sender<Bytes>,
    streaming: AtomicBool,
}

impl StreamManager {
    pub fn new(audio_format: AudioFormat) -> Self {
        let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            audio_format,
            sender,
            streaming: AtomicBool::new(false),
        }
    }

    pub fn sender(&self) -> broadcast::Sender<Bytes> {
        self.sender.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.sender.subscribe()
    }

    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    pub fn set_streaming(&self, val: bool) {
        self.streaming.store(val, Ordering::Relaxed);
    }

    pub fn is_streaming(&self) -> bool {
        self.streaming.load(Ordering::Relaxed)
    }

    pub fn audio_format(&self) -> &AudioFormat {
        &self.audio_format
    }

    /// Build a 44-byte RIFF WAV header for streaming (unknown length).
    pub fn create_wav_header(&self) -> [u8; 44] {
        let fmt = &self.audio_format;
        let byte_rate = fmt.byte_rate();
        let block_align = fmt.block_align();

        let mut header = [0u8; 44];

        header[0..4].copy_from_slice(b"RIFF");
        header[4..8].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        header[8..12].copy_from_slice(b"WAVE");
        header[12..16].copy_from_slice(b"fmt ");
        header[16..20].copy_from_slice(&16u32.to_le_bytes());
        header[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
        header[22..24].copy_from_slice(&fmt.channels.to_le_bytes());
        header[24..28].copy_from_slice(&fmt.sample_rate.to_le_bytes());
        header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
        header[32..34].copy_from_slice(&block_align.to_le_bytes());
        header[34..36].copy_from_slice(&fmt.bit_depth.to_le_bytes());
        header[36..40].copy_from_slice(b"data");
        header[40..44].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());

        header
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let sm = StreamManager::new(AudioFormat::default());
        assert!(!sm.is_streaming());
    }

    #[test]
    fn test_set_streaming() {
        let sm = StreamManager::new(AudioFormat::default());
        sm.set_streaming(true);
        assert!(sm.is_streaming());
        sm.set_streaming(false);
        assert!(!sm.is_streaming());
    }

    #[test]
    fn test_subscribe_increments_count() {
        let sm = StreamManager::new(AudioFormat::default());
        let _rx1 = sm.subscribe();
        // broadcast receiver_count includes all live receivers
        assert!(sm.subscriber_count() >= 1);
        let _rx2 = sm.subscribe();
        assert!(sm.subscriber_count() >= 2);
    }

    #[test]
    fn test_wav_header_length() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(header.len(), 44);
    }

    #[test]
    fn test_wav_header_riff_marker() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(&header[0..4], b"RIFF");
    }

    #[test]
    fn test_wav_header_wave_marker() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(&header[8..12], b"WAVE");
    }

    #[test]
    fn test_wav_header_fmt_marker() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(&header[12..16], b"fmt ");
    }

    #[test]
    fn test_wav_header_pcm_format() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(u16::from_le_bytes([header[20], header[21]]), 1);
    }

    #[test]
    fn test_wav_header_channels() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(u16::from_le_bytes([header[22], header[23]]), 2);
    }

    #[test]
    fn test_wav_header_sample_rate() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(
            u32::from_le_bytes([header[24], header[25], header[26], header[27]]),
            44100
        );
    }

    #[test]
    fn test_wav_header_byte_rate() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(
            u32::from_le_bytes([header[28], header[29], header[30], header[31]]),
            176400
        );
    }

    #[test]
    fn test_wav_header_block_align() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(u16::from_le_bytes([header[32], header[33]]), 4);
    }

    #[test]
    fn test_wav_header_bit_depth() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(u16::from_le_bytes([header[34], header[35]]), 16);
    }

    #[test]
    fn test_wav_header_data_marker() {
        let sm = StreamManager::new(AudioFormat::default());
        let header = sm.create_wav_header();
        assert_eq!(&header[36..40], b"data");
    }

    #[tokio::test]
    async fn test_broadcast_distributes_data() {
        let sm = StreamManager::new(AudioFormat::default());
        let mut rx1 = sm.subscribe();
        let mut rx2 = sm.subscribe();
        let tx = sm.sender();

        let data = Bytes::from_static(b"audio chunk");
        tx.send(data.clone()).unwrap();

        assert_eq!(rx1.recv().await.unwrap(), data);
        assert_eq!(rx2.recv().await.unwrap(), data);
    }
}
