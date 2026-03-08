import { PassThrough, Readable } from 'stream';
import { AudioFormat } from './types';

export class StreamManager {
  private source: Readable | null = null;
  private subscribers = new Map<string, PassThrough>();
  private audioFormat: AudioFormat;

  constructor(audioFormat: AudioFormat) {
    this.audioFormat = audioFormat;
  }

  setSource(source: Readable): void {
    this.cleanup();
    this.source = source;

    source.on('data', (chunk: Buffer) => {
      for (const [id, passthrough] of this.subscribers) {
        if (!passthrough.destroyed) {
          // Don't use backpressure — if a subscriber is slow, skip data
          // rather than blocking other subscribers
          if (!passthrough.write(chunk)) {
            // Subscriber buffer is full, data will be dropped
          }
        }
      }
    });

    source.on('end', () => {
      console.log('[stream-manager] Source stream ended');
      this.endAllSubscribers();
    });

    source.on('error', (err) => {
      console.error(`[stream-manager] Source stream error: ${err.message}`);
      this.endAllSubscribers();
    });
  }

  subscribe(id: string): PassThrough {
    // Clean up existing subscription if any
    this.unsubscribe(id);

    const passthrough = new PassThrough({
      highWaterMark: this.audioFormat.sampleRate * (this.audioFormat.bitDepth / 8) * this.audioFormat.channels,
    });

    this.subscribers.set(id, passthrough);
    console.log(`[stream-manager] Subscriber added: ${id} (total: ${this.subscribers.size})`);
    return passthrough;
  }

  unsubscribe(id: string): void {
    const existing = this.subscribers.get(id);
    if (existing) {
      if (!existing.destroyed) {
        existing.end();
      }
      this.subscribers.delete(id);
      console.log(`[stream-manager] Subscriber removed: ${id} (total: ${this.subscribers.size})`);
    }
  }

  createWavHeader(): Buffer {
    const { sampleRate, bitDepth, channels } = this.audioFormat;
    const byteRate = sampleRate * channels * (bitDepth / 8);
    const blockAlign = channels * (bitDepth / 8);

    const header = Buffer.alloc(44);
    header.write('RIFF', 0);
    header.writeUInt32LE(0xffffffff, 4); // file size - unknown for streaming
    header.write('WAVE', 8);
    header.write('fmt ', 12);
    header.writeUInt32LE(16, 16); // fmt chunk size
    header.writeUInt16LE(1, 20); // PCM format
    header.writeUInt16LE(channels, 22);
    header.writeUInt32LE(sampleRate, 24);
    header.writeUInt32LE(byteRate, 28);
    header.writeUInt16LE(blockAlign, 32);
    header.writeUInt16LE(bitDepth, 34);
    header.write('data', 36);
    header.writeUInt32LE(0xffffffff, 40); // data size - unknown for streaming

    return header;
  }

  createWavStream(subscriberId: string): PassThrough {
    const pcmStream = this.subscribe(subscriberId);
    const wavStream = new PassThrough();

    // Write WAV header first
    wavStream.write(this.createWavHeader());

    // Then pipe PCM data through
    pcmStream.pipe(wavStream, { end: true });

    return wavStream;
  }

  private endAllSubscribers(): void {
    for (const [id, passthrough] of this.subscribers) {
      if (!passthrough.destroyed) {
        passthrough.end();
      }
    }
  }

  cleanup(): void {
    this.endAllSubscribers();
    this.subscribers.clear();
    this.source = null;
  }

  get subscriberCount(): number {
    return this.subscribers.size;
  }

  get isStreaming(): boolean {
    return this.source !== null;
  }
}
