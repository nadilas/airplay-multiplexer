import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import { PassThrough, Readable } from 'stream';
import { StreamManager } from '../stream-manager';
import { AudioFormat } from '../types';

const defaultFormat: AudioFormat = {
  sampleRate: 44100,
  bitDepth: 16,
  channels: 2,
};

describe('StreamManager', () => {
  let manager: StreamManager;

  beforeEach(() => {
    manager = new StreamManager(defaultFormat);
  });

  afterEach(() => {
    manager.cleanup();
  });

  it('starts with no subscribers and not streaming', () => {
    assert.strictEqual(manager.subscriberCount, 0);
    assert.strictEqual(manager.isStreaming, false);
  });

  it('subscribe adds a subscriber', () => {
    const stream = manager.subscribe('device-1');
    assert.ok(stream instanceof PassThrough);
    assert.strictEqual(manager.subscriberCount, 1);
  });

  it('subscribe replaces an existing subscription for the same id', () => {
    const first = manager.subscribe('device-1');
    const second = manager.subscribe('device-1');
    assert.strictEqual(manager.subscriberCount, 1);
    // First stream should be ended
    assert.ok(first.destroyed || first.writableEnded);
  });

  it('unsubscribe removes a subscriber', () => {
    manager.subscribe('device-1');
    manager.subscribe('device-2');
    assert.strictEqual(manager.subscriberCount, 2);

    manager.unsubscribe('device-1');
    assert.strictEqual(manager.subscriberCount, 1);
  });

  it('unsubscribe is a no-op for unknown id', () => {
    manager.unsubscribe('nonexistent');
    assert.strictEqual(manager.subscriberCount, 0);
  });

  it('setSource distributes data to all subscribers', (t, done) => {
    const source = new PassThrough();
    manager.setSource(source);
    assert.strictEqual(manager.isStreaming, true);

    const sub1 = manager.subscribe('device-1');
    const sub2 = manager.subscribe('device-2');

    const chunks1: Buffer[] = [];
    const chunks2: Buffer[] = [];

    sub1.on('data', (chunk) => chunks1.push(chunk));
    sub2.on('data', (chunk) => chunks2.push(chunk));

    const testData = Buffer.from('hello audio data');
    source.write(testData);

    // Allow event loop to process
    setTimeout(() => {
      assert.strictEqual(chunks1.length, 1);
      assert.strictEqual(chunks2.length, 1);
      assert.deepStrictEqual(chunks1[0], testData);
      assert.deepStrictEqual(chunks2[0], testData);
      done();
    }, 50);
  });

  it('setSource ends subscribers when source ends', (t, done) => {
    const source = new PassThrough();
    manager.setSource(source);

    const sub = manager.subscribe('device-1');

    let ended = false;
    sub.on('end', () => {
      ended = true;
    });

    // Need to consume data for 'end' to fire
    sub.resume();

    source.end();

    setTimeout(() => {
      assert.strictEqual(ended, true);
      done();
    }, 50);
  });

  it('cleanup clears all subscribers and source', () => {
    const source = new PassThrough();
    manager.setSource(source);
    manager.subscribe('device-1');
    manager.subscribe('device-2');

    manager.cleanup();

    assert.strictEqual(manager.subscriberCount, 0);
    assert.strictEqual(manager.isStreaming, false);
  });

  it('createWavHeader returns a valid 44-byte WAV header', () => {
    const header = manager.createWavHeader();

    assert.strictEqual(header.length, 44);
    // RIFF marker
    assert.strictEqual(header.toString('ascii', 0, 4), 'RIFF');
    // WAVE marker
    assert.strictEqual(header.toString('ascii', 8, 12), 'WAVE');
    // fmt marker
    assert.strictEqual(header.toString('ascii', 12, 16), 'fmt ');
    // PCM format (1)
    assert.strictEqual(header.readUInt16LE(20), 1);
    // Channels
    assert.strictEqual(header.readUInt16LE(22), defaultFormat.channels);
    // Sample rate
    assert.strictEqual(header.readUInt32LE(24), defaultFormat.sampleRate);
    // Byte rate = sampleRate * channels * (bitDepth / 8)
    const expectedByteRate = defaultFormat.sampleRate * defaultFormat.channels * (defaultFormat.bitDepth / 8);
    assert.strictEqual(header.readUInt32LE(28), expectedByteRate);
    // Block align = channels * (bitDepth / 8)
    const expectedBlockAlign = defaultFormat.channels * (defaultFormat.bitDepth / 8);
    assert.strictEqual(header.readUInt16LE(32), expectedBlockAlign);
    // Bit depth
    assert.strictEqual(header.readUInt16LE(34), defaultFormat.bitDepth);
    // data marker
    assert.strictEqual(header.toString('ascii', 36, 40), 'data');
  });

  it('createWavStream returns a stream with WAV header followed by PCM data', (t, done) => {
    const source = new PassThrough();
    manager.setSource(source);

    const wavStream = manager.createWavStream('wav-subscriber');
    const chunks: Buffer[] = [];

    wavStream.on('data', (chunk) => chunks.push(chunk));

    const pcmData = Buffer.alloc(1024, 0x42);
    source.write(pcmData);

    setTimeout(() => {
      const combined = Buffer.concat(chunks);
      // Should start with WAV header
      assert.strictEqual(combined.toString('ascii', 0, 4), 'RIFF');
      // The PCM data should follow after the 44-byte header
      const afterHeader = combined.slice(44);
      assert.ok(afterHeader.length > 0);
      done();
    }, 100);
  });

  it('handles multiple setSource calls by cleaning up previous source', () => {
    const source1 = new PassThrough();
    const source2 = new PassThrough();

    manager.setSource(source1);
    manager.subscribe('device-1');
    assert.strictEqual(manager.subscriberCount, 1);

    // Setting a new source should clean up
    manager.setSource(source2);
    assert.strictEqual(manager.isStreaming, true);
    // Subscribers are cleared on new setSource
    assert.strictEqual(manager.subscriberCount, 0);
  });
});
